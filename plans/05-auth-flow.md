# 05 — Auth Flow

Authentication uses Amazon Cognito with two federated IdPs (Google, Facebook). (No Apple IdP — dropped, see `PROGRESS.md` B5 decision; Apple Developer membership is $99/yr.) Self-signup is disabled in the user pool; users can only enter the system by redeeming an invite code. Existing users can join additional groups by redeeming additional invites.

This document covers Cognito configuration, the OAuth round-trip, invite redemption (both first-time and additional), the bootstrap admin script, and the JWT verification path on the backend.

---

## 1. Cognito user pool configuration recap

(Detailed in `01-infrastructure-cdk.md` §4. Key points reiterated here.)

- **Self sign-up: DISABLED.** All signups happen via federation; the `PreSignUp` trigger auto-confirms them.
- **Custom attributes**: none (the invite code travels in the OIDC `state` parameter, §3).
- **Triggers**:
  - `PreSignUp` — pure pass-through; auto-confirm only, no invite logic (§3)
  - No `PostConfirmation` trigger (§5)
- **App client**: Authorization Code with PKCE, public client (no secret), Cognito disabled-as-IdP for end users.
- **Bootstrap admin client**: separate app client `admin-bootstrap` that allows USER_PASSWORD_AUTH. **Dev only** (decided 2026-10-07; lands in M13.5, D5) — in prod, `scripts/bootstrap_admin.py` no longer creates a Cognito user or uses password auth at all; see §9.

---

## 2. End-to-end first-signup flow

Goal: a person with an invite URL becomes a Cognito user AND is added to a group atomically.

```
┌─────────┐                     ┌──────────┐                   ┌─────────┐                ┌──────────┐
│ Browser │                     │ GH Pages │                   │ Cognito │                │  Lambda  │
│         │                     │   SPA    │                   │  Hosted │                │          │
│         │                     │          │                   │   UI    │                │          │
└────┬────┘                     └─────┬────┘                   └────┬────┘                └────┬─────┘
     │                                │                             │                          │
     │  1. visits /join?code=XYZ      │                             │                          │
     │ ──────────────────────────────►│                             │                          │
     │                                │                             │                          │
     │                                │  2. stash code in sessionStorage; also attach          │
     │                                │     { invite, returnTo } as oidc-client-ts state data  │
     │                                │     (§3.3); redirect to Cognito hosted UI              │
     │ ◄──────────────────────────────│                             │                          │
     │                                                              │                          │
     │  3. picks IdP (Google), signs in there, returns to Cognito                              │
     │ ────────────────────────────────────────────────────────────►│                          │
     │                                                              │                          │
     │                                                              │  4. PreSignUp trigger    │
     │                                                              │     (pass-through:       │
     │                                                              │      auto-confirm only)  │
     │                                                              │ ────────────────────────►│
     │                                                              │ ◄────────────────────────│
     │                                                              │                          │
     │  5. redirected back to /auth/callback?code=...&state=...     │                          │
     │ ◄────────────────────────────────────────────────────────────│                          │
     │                                │                             │                          │
     │  6. SPA exchanges code for tokens (PKCE)                     │                          │
     │ ──────────────────────────────►│ ───────────────────────────►│                          │
     │                                │                             │                          │
     │  7. SPA reads invite code back out of the state data (or the │                          │
     │     sessionStorage stash), goes to /join?code=…, which calls │                          │
     │     POST /invites/redeem                                     │                          │
     │     → lambda-invites creates User (if new) + GroupMembership │                          │
     │       + consumes the invite in one TransactWriteItems (§4)   │                          │
     │ ──────────────────────────────►│ ────────────────────────────────────────────────────► │
     │                                │                             │                          │
     │  8. SPA refreshes /config, sees the new group, renders Home  │                          │
```

---

## 3. PreSignUp Lambda

Crate: `lambda-invites` (it has both the API endpoints and the Cognito triggers — they share the persistence module). Distinct binary entry points.

### 3.1 Role

`PreSignUp` is a **pure pass-through** that auto-confirms federated accounts. It does NOT see or consume the invite code — Cognito's hosted UI does not reliably forward `clientMetadata` from the front channel, so we don't depend on it.

Invite consumption happens later, after the SPA holds a real JWT, via `POST /invites/redeem` (§4). That handler does both invite consumption AND group membership creation in a single transaction, and creates the User row lazily on first redemption.

### 3.2 Why this is fine

- Without an invite redemption, the user has zero memberships → `/config` returns empty groups → SPA redirects to `/join`. They cannot reach any group's data because every protected route checks membership.
- The user IS in Cognito but completely inert. We accept the small downside of having user-pool entries that never finished onboarding. A cleanup job is designed in `10-archival.md` §6 but deferred, so in v1 these entries simply accumulate.

### 3.3 PreSignUp logic

```rust
// returns auto-confirm true so federated users skip email confirm
fn handle(event: PreSignUp) -> PreSignUp {
    event.response.auto_confirm_user = true;
    event.response.auto_verify_email = matches!(event.trigger_source.as_str(), "PreSignUp_ExternalProvider");
    event
}
```

The SPA preserves the invite code across the OAuth round-trip by passing `{ invite, returnTo }` as `oidc-client-ts`'s `signinRedirect({ state })` data. The library keeps that data client-side, keyed by the opaque OIDC `state` nonce, so nothing user-controlled travels through Cognito (decided M7; `04` §6). A `sessionStorage` stash is the fallback. After token exchange, `/auth/callback` sends the user to `/join?code=…`, which posts the code to `/invites/redeem`.

---

## 4. Invite redemption (`POST /invites/redeem`)

The single, canonical place where invites are consumed.

### 4.1 Inputs

- `Authorization: Bearer {idToken}` — caller is authenticated. It must be the **ID token**: the algorithm below reads `jwt.email`, `jwt.name`, and (for bound invites) `jwt.email_verified`, none of which the access token carries. A request that reaches this handler without an `email` claim is rejected with `VALIDATION_FAILED`.
- Body: `{ "code": "ABCD-EFGH-JKMN-PQRS" }`

### 4.2 Algorithm

```
sub = jwt.sub
code = body.code

# 1. Look up invite
invite = GetItem(pk=INVITE#{code}, sk=META)
if invite is None: return 400 INVITE_INVALID
if invite.status == "consumed": return 409 INVITE_CONSUMED
if invite.status == "revoked": return 400 INVITE_INVALID
if now() > invite.expiresAt: return 410 INVITE_EXPIRED

# 2. Bound-email check (decided 2026-10-07; lands in M13.5, D5)
if invite.boundEmail is not None:
    if jwt.email is None or jwt.email.lower() != invite.boundEmail or not jwt.email_verified:
        return 403 INVITE_EMAIL_MISMATCH

# 3. Look up group + check member cap
group = GetItem(pk=GROUP#{invite.groupId}, sk=META)
if group.memberCount >= group.memberSoftCap: return 409 MEMBER_CAP_REACHED

# 4. Resolve sub -> userId (or mint a new one)
lookup = GetItem(pk=COGNITO_SUB#{sub}, sk=USER_ID)
if lookup is None:
    user_id = uuidv7()                          # first-ever redemption for this Cognito account
    is_new_user = True
else:
    user_id = lookup.userId
    is_new_user = False

# 5. Already a member? Short-circuit (idempotent retries land here).
if not is_new_user:
    existing = GetItem(pk=USER#{user_id}, sk=GROUP#{invite.groupId})
    if existing is not None:
        return 200 { groupId, groupName, role: existing.role }

# 6. Atomic transaction (TransactWriteItems)
items_to_write = []
if is_new_user:
    items_to_write += [
        Put(CognitoSubLookup(sub=sub, userId=user_id),
            Condition: attribute_not_exists(pk)),
        Put(User(userId=user_id, cognitoSub=sub, email=jwt.email, displayName=jwt.name or jwt.email,
                  avatarColor=derive_color(user_id))),
    ]
items_to_write += [
    Update(invite,
        Condition: status = "pending" AND now < expiresAt,
        Set: status = "consumed", consumedBy = user_id, consumedAt = now()),
    Put(GroupMembership(userId=user_id, groupId=invite.groupId, role=invite.roleOnRedeem),
        Condition: attribute_not_exists(pk)),
    Update(group, ADD memberCount 1,
        Condition: memberCount < memberSoftCap),
]
write_transact(items_to_write)

return 200 { groupId, groupName, role: invite.roleOnRedeem }
```

**Bound-email check, step 2** (decided 2026-10-07; lands in M13.5, D5). `jwt.email_verified` comes from Cognito's `email_verified` attribute, which only Google populates via the IdP attribute mapping (`01` §4.2); Facebook doesn't supply it. So a `boundEmail` invite can only be redeemed by signing in with Google using that exact (lowercased) address — a Facebook sign-in, or a Google sign-in with a different or unverified email, gets `INVITE_EMAIL_MISMATCH`. This is the mechanism the prod bootstrap admin uses (§9).

### 4.3 First-signup vs additional-group

The same algorithm covers both cases:
- First signup: `User` row is missing → created.
- Additional group: `User` row exists; only the `Invite` and `GroupMembership` (and group counter) change.

### 4.4 Idempotency

Re-POSTing the same code with the same caller after success: invite is `consumed`, so we return 409 `INVITE_CONSUMED`. To make the SPA's life simpler, the handler ALSO checks whether `consumedBy == user_id`; if yes, it returns 200 with the membership info (this happens if the user double-submits or the network retries).

### 4.5 SPA wrapper

The `JoinPage` calls `POST /invites/redeem` and on success:
- Adds the new group to the React Query cache.
- Sets `currentGroup = newGroupId`.
- Navigates to `/g/{newGroupId}/upcoming`.

If the user reached `/join?code=...` while logged out: the SPA stashes the code in `sessionStorage`, redirects to login, and on `/auth/callback` checks for the stash and finishes the redemption automatically.

---

## 5. PostConfirmation trigger

Vestigial. Since the User row is created lazily by `/invites/redeem`, the only thing PostConfirmation needs to do is record `lastLoginAt` heuristics. We don't need it for v1.

**Decision: do not implement a PostConfirmation trigger.** Less surface area, fewer Lambdas, simpler IAM.

---

## 6. JWT verification on the backend

API Gateway's HTTP API JWT authorizer does the heavy lifting:
- Validates issuer = `https://cognito-idp.us-east-1.amazonaws.com/{userPoolId}`
- Validates `aud` (or `client_id` for access tokens). In `dev`, the audience list is `[frontend_client_id, bootstrap_client_id]`. In `prod`, it's `[frontend_client_id]` only — `admin-bootstrap` doesn't exist there (decided 2026-10-07; lands in M13.5, D5; `01` §4.3/§6.1).
- Validates signature against JWKS
- Validates `exp` and `nbf`

When the token is valid, the JWT claims are placed in `event.requestContext.authorizer.jwt.claims`. The Rust handlers parse this via:

```rust
#[derive(Deserialize)]
struct AuthClaims {
    sub: String,                // user id
    email: Option<String>,
    email_verified: Option<bool>,   // read by POST /invites/redeem for bound invites (D5)
    name: Option<String>,
    #[serde(rename = "cognito:username")]
    cognito_username: Option<String>,
}

fn extract_claims(req: &Request) -> Result<AuthClaims, ApiError> {
    let claims_value = req.request_context()
        .authorizer
        .as_ref()
        .and_then(|a| a.jwt.as_ref())
        .map(|j| j.claims.clone())
        .ok_or(ApiError::Unauthenticated)?;
    serde_json::from_value(serde_json::Value::Object(claims_value.into_iter().collect()))
        .map_err(|_| ApiError::Unauthenticated)
}
```

The handler does NOT re-verify the signature — API Gateway already did. It trusts the claims.

After extracting claims, every protected handler resolves `claims.sub` → our internal `userId` via the `CognitoSubLookup` row (`02-data-model-dynamodb.md` §2.1a). This lookup is cached per cold-start for ≤60s alongside the membership cache so the cost is amortized:

```rust
pub async fn resolve_user_id(repo: &Repo, sub: &str) -> Result<UserId, ApiError> {
    if let Some(cached) = SUB_LOOKUP_CACHE.get(sub) {
        if cached.cached_at.elapsed() < Duration::from_secs(60) {
            return Ok(cached.user_id.clone());
        }
    }
    let user_id = repo.lookup_user_id_by_sub(sub).await?
        .ok_or(ApiError::Unauthenticated)?;   // happens on first request before invite redemption
    SUB_LOOKUP_CACHE.insert(sub.to_string(), Cached { user_id: user_id.clone(), cached_at: Instant::now() });
    Ok(user_id)
}
```

A `None` from the lookup means the JWT is valid but the account has never redeemed an invite — `/config` handles this case specially (returns empty memberships); other handlers return `UNAUTHENTICATED`.

There is no in-Lambda JWT verifier and no local API Gateway proxy (corrected M13, `11` §2.1). Handlers only ever run behind the API Gateway authorizer.

---

## 7. Token lifetime and refresh

- Access token: 60 min
- ID token: 60 min
- Refresh token: 30 days

The frontend renews in `auth/renewSession.ts` (decided in M7), shortly before expiry (`accessTokenExpiring`), on a reload with an expired ID token, and after a 401. Cognito has no `prompt=none` iframe flow, so federated sessions use the **refresh-token grant** against `/oauth2/token`. Bootstrap-login sessions (§9.2) hold a refresh token for the `admin-bootstrap` client, so they renew with Cognito `InitiateAuth` `REFRESH_TOKEN_AUTH` against that client. If renewal fails (refresh token expired or revoked), the stored user is dropped and protected routes bounce to login.

**Storage moves to `localStorage` (decided 2026-10-07; lands in M13.5, D1).** `oidc-client-ts`'s `userStore` — the ID/access/refresh tokens — now persists in `localStorage` instead of `sessionStorage`, so an installed PWA stays signed in across separate launches for the full 30-day refresh-token lifetime, instead of losing its session the moment the tab/app closes. `inviteStash` (§4.5, §13) is unaffected and stays in `sessionStorage`. See §8 (logout) and §12 (threat model) for the consequences.

---

## 8. Logout

`logout()` in `auth/AuthProvider.tsx` (decided 2026-10-07; lands in M13.5, D1 — step order and step 4 are new):
1. Removes the stored user from `localStorage` (was `sessionStorage`; see §7).
2. Clears React Query cache.
3. Clears Zustand persisted stores.
4. **Revokes the refresh token** via Cognito `POST /oauth2/revoke` (best effort — a network failure here doesn't block logout; the client-side token removal in step 1 already ends the session locally).
5. Redirects to Cognito's `/logout` endpoint with `client_id` and `logout_uri=https://opennewsletter.example.com/`. Cognito's OIDC discovery document has no `end_session_endpoint`, so the SPA builds this URL from `VITE_COGNITO_HOSTED_DOMAIN` itself.

Cognito invalidates the SSO session. The revoke call means a token captured before logout (e.g. by XSS, §12) can't be refreshed past this point even though it was read from `localStorage` rather than `sessionStorage`. The user lands back at the SPA logged out.

---

## 9. Bootstrap admin & first group

There must be a way to create the very first group + first admin without an invite code (chicken-and-egg). This is solved with a one-shot script invoked by the operator. **Dev and prod now follow different flows** (decided 2026-10-07; lands in M13.5, D5): dev keeps the original password-bootstrap flow (§9.1); prod no longer creates a Cognito user at all and instead bootstraps via an email-bound admin invite (§9.1a).

### 9.1 `scripts/bootstrap_admin.py` — dev (unchanged)

Inputs (CLI flags or env):
- `--env dev`
- `--admin-email`
- `--admin-display-name`
- `--group-name`
- `--timezone` (defaults to `America/New_York`)

Flow:
1. Reads CDK outputs (`user_pool_id`, `table_name`) from `cdk.out/{env}-outputs.json` or via `aws cloudformation describe-stacks`.
2. Calls `cognito-idp admin-create-user` with email + a temporary password and `MessageAction=SUPPRESS`. Marks email verified.
3. Calls `cognito-idp admin-set-user-password` to set a permanent password (asks the operator for it interactively, or accepts `--password`).
4. Calls `cognito-idp admin-add-user-to-group` to add the user to a Cognito-side `admins` group (informational; the real role lives in DynamoDB).
5. Reads the new user's `sub`.
6. Issues a `TransactWriteItems` against DynamoDB with:
   - Put `CognitoSubLookup` (`COGNITO_SUB#{sub}` → `userId`)
   - Put `User` (PROFILE)
   - Put `Group` (META) with the chosen settings + `memberCount=1`
   - Put `GroupMembership` (role=admin)
   - Put the group's first `voting` Newsletter (`06` §4.2; added M5)

   IDs are UUID5-derived from the Cognito sub, so re-running the script is idempotent.
7. Prints the group ID and a sign-in URL.

### 9.1a `scripts/bootstrap_admin.py` — prod (decided 2026-10-07; lands in M13.5, D5)

Inputs: `--env prod`, `--group-name`, `--timezone`; the admin's email comes from `config.bootstrap_admin_email` (`BOOTSTRAP_ADMIN_EMAIL`, `01` §2), not a CLI flag, since it must match what the operator later signs in with.

Flow:
1. Reads CDK outputs the same way as dev.
2. Creates **no Cognito user**. Issues a `TransactWriteItems` against DynamoDB with:
   - Put `Group` (META) with the chosen settings + **`memberCount = 0`** (there's no member yet — redemption bumps it, same as any other invite join)
   - Put the group's first `voting` Newsletter (`06` §4.2)
   - Put an `Invite` (`02` §2.4) with `bound_email = BOOTSTRAP_ADMIN_EMAIL.lower()`, `role_on_redeem = admin`, `ttlDays = 30`
3. Prints the `/join?code=…` URL for that invite.
4. The operator opens the URL and signs in with **Google**, using the bootstrap admin email. The normal redeem path (§4) creates the `User` row and `GroupMembership` exactly as it would for any other invite — the bound-email check (§4.2 step 2) passes because it's the same email, and Google supplies `email_verified` (`01` §4.2). No special-cased bootstrap code runs on the redeem side at all; §9.1a's only job is minting the right invite.

A bound invite must be redeemed via Google — Facebook doesn't supply `email_verified` (§4.2). If the operator's primary account is Facebook-only, they need a Google account with the bootstrap email for this one step.

### 9.2 Sign-in for the bootstrap admin

**Dev**: the bootstrap admin uses a separate **`admin-bootstrap`** Cognito app client that allows `ALLOW_USER_PASSWORD_AUTH` (dev-only, `01` §4.3). The frontend has a hidden route `/admin/bootstrap-login` (only enabled in dev or via a feature flag set in `env.ts`) that posts username+password directly to Cognito and stores the resulting tokens.

**Prod**: there is no bootstrap-specific sign-in path. The operator signs in through the normal Google OAuth flow like any other user (§2); redeeming the bound invite (§9.1a step 4) is what grants admin membership. The old password-auth-then-federate-later approach (and its deferred `admin-link-provider-for-user` idea) no longer applies in prod.

### 9.3 Subsequent admin actions

Once the bootstrap admin exists (dev: the password account; prod: the Google-federated account created at redemption), they:
1. Sign in.
2. Open `/g/:g/admin` → Invites tab → "Generate invite" with role=admin → send the URL to another founding member if desired.
3. **Dev only**: redeem a fresh admin invite on a federated account so that account becomes the canonical admin, then optionally demote or remove the password-bootstrap account.

---

## 10. Identity provider setup checklist (operator runbook)

Outside of CDK, a human operator must:

All of the following now land as JSON keys inside **one** per-env Secrets Manager secret, `opennewsletter/{env}` (`app_secret_arn`, `01` §2/§7.1/§12), rather than one secret each (decided 2026-10-07; lands in M13.5, D7):

- [ ] **Google**: Create OAuth 2.0 client in Google Cloud Console. Authorized redirect URI: `https://{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`. Authorized JS origin: `https://opennewsletter.example.com`. Save Client ID + Client Secret as `google_client_id`/`google_client_secret` in `opennewsletter/{env}`.
- [ ] **Facebook**: Create app in Meta for Developers. Add Facebook Login. Valid OAuth redirect URI: same Cognito callback. Save App ID + Secret as `facebook_app_id`/`facebook_app_secret` in `opennewsletter/{env}`.
- [ ] **VAPID keys**: `python scripts/generate_vapid_keys.py` → merge the JSON output's `publicKey`/`privateKey`/`subject` into `opennewsletter/{env}` as `vapid_public_key`/`vapid_private_key`/`vapid_subject`.
- [ ] **CloudFront signing keypair**: generate RSA 2048; upload public to CloudFront (CDK does this from a public-key file in `infra/keys/`); store the private key as `cdn_signing_private_key` in `opennewsletter/{env}`.
- [ ] **Bootstrap admin email (prod only)**: set `BOOTSTRAP_ADMIN_EMAIL` for `scripts/bootstrap_admin.py`'s prod mode (§9.1a).

This checklist is reproduced in `12-build-order.md` Milestone 1.

---

## 11. Tenant authorization helper

A single Rust function used by every protected handler:

```rust
pub async fn require_membership(
    repo: &Repo,
    user_id: &str,
    group_id: &str,
    require_admin: bool,
) -> Result<GroupMembership, ApiError> {
    if let Some(cached) = MEMBERSHIP_CACHE.get(&(user_id.to_string(), group_id.to_string())) {
        if cached.cached_at.elapsed() < Duration::from_secs(60) {
            return enforce(cached.membership.clone(), require_admin);
        }
    }
    let m = repo.get_membership(user_id, group_id).await?
        .ok_or(ApiError::Forbidden)?;
    MEMBERSHIP_CACHE.insert((user_id.into(), group_id.into()), Cached { membership: m.clone(), cached_at: Instant::now() });
    enforce(m, require_admin)
}

fn enforce(m: GroupMembership, require_admin: bool) -> Result<GroupMembership, ApiError> {
    if require_admin && m.role != Role::Admin { return Err(ApiError::Forbidden); }
    Ok(m)
}
```

`MEMBERSHIP_CACHE` is a `once_cell::sync::Lazy<DashMap<...>>` that lives for the duration of the Lambda's warm container.

---

## 12. Threat model considerations

- **Forged invites**: invite codes are 16-char Crockford base32 (≈ 80 bits of entropy). Brute-force trying random codes is infeasible. There is no per-user rate limit on `POST /invites/redeem`: HTTP APIs can't throttle per caller, so only the stage-wide 25 rps / 50 burst limit (`01` §6.1) applies. The code entropy is what protects it.
- **Reusing invites**: prevented by `TransactWriteItems` condition on `status=pending`.
- **Privilege escalation**: only admins can call admin routes; admins are scoped to one group at a time. The auth helper enforces this on every handler.
- **JWT confusion**: API Gateway validates audience and issuer. Backend trusts `sub` only.
- **Token theft via XSS** (updated 2026-10-07; lands in M13.5, D1): tokens now live in `localStorage` rather than `sessionStorage` (§7), so an installed PWA stays signed in across launches. XSS could read tokens from either storage equally — that part of the exposure is unchanged. The difference is lifetime: a stolen refresh token out of `localStorage` stays valid for up to 30 days unless revoked, versus disappearing when the tab closed under the old `sessionStorage` scheme. CSP (restricting script/connect/img sources) remains the primary mitigation against the XSS that would be needed to read it in the first place — the full policy is specified in `04-frontend-architecture.md` §12.1, shipped as a `<meta>` tag in `index.html` (GitHub Pages can't set headers) and verified by a Playwright test in Milestone 13. Logout's refresh-token revocation (§8) is the secondary mitigation: it closes the window for a token stolen before logout. Shared-device exposure (a draft mirror or cached CDN images surviving logout on a shared device) is accepted as-is — a pre-existing risk this change doesn't materially worsen (owner decision D9).
- **Replay of CDN cookies**: cookies expire in 1 hour and are scoped to `/img/{groupId}/*`. A leaked cookie grants read access to that group's images for ≤1 hour. Acceptable.

---

## 13. Sequence summary (text version of §2 diagram)

1. User clicks invite URL → SPA loads `/join?code=XYZ`
2. SPA stashes code, redirects to Cognito hosted UI
3. User completes OAuth at IdP, redirects to Cognito, then back to `/auth/callback`
4. SPA exchanges code for tokens
5. SPA pulls stashed invite code, calls `POST /invites/redeem`
6. Backend atomically creates User (if needed) + GroupMembership + consumes invite
7. SPA refreshes `/config`, sees the new group, navigates user in
