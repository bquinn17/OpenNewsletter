# Auth flow

Authentication uses Amazon Cognito with two federated IdPs, Google and Facebook. (No Apple IdP: Sign In with Apple requires a paid, $99/yr Apple Developer membership, which the owner declined.) Self-signup is disabled in the user pool; users can only enter the system by redeeming an invite code. Existing users can join additional groups by redeeming additional invites.

This document covers Cognito configuration, the OAuth round-trip, invite redemption (both first-time and additional), the bootstrap admin script, and the JWT verification path on the backend. It's for anyone touching sign-in, invite redemption, or the tenant-isolation checks that depend on them. [`infrastructure.md` §4](infrastructure.md#4-authstack) is the source of truth for the exact Cognito resource configuration; this page covers the flows and the reasoning behind them.

---

## 1. Cognito user pool configuration recap

(Detailed in [`infrastructure.md` §4](infrastructure.md#4-authstack). Key points reiterated here.)

- **Self sign-up: DISABLED.** All signups happen via federation; the `PreSignUp` trigger auto-confirms them.
- **Custom attributes**: none (the invite code travels in the OIDC `state` parameter, §3).
- **Triggers**:
  - `PreSignUp` — pure pass-through; auto-confirm only, no invite logic (§3)
  - No `PostConfirmation` trigger (§5)
- **App client**: Authorization Code with PKCE, public client (no secret), Cognito disabled-as-IdP for end users.
- **Bootstrap admin client**: separate app client `admin-bootstrap` that allows USER_PASSWORD_AUTH. **Dev only** — prod has no password auth; `scripts/bootstrap_admin.py` doesn't create a Cognito user there at all; see §9.

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
- The user IS in Cognito but completely inert. We accept the small downside of having user-pool entries that never finished onboarding. A cleanup job is designed in [`archival.md` §6](archival.md#6-inactive-account-cleanup-separate-but-related) but deferred, so in v1 these entries simply accumulate.

### 3.3 PreSignUp logic

```rust
// returns auto-confirm true so federated users skip email confirm
fn handle(event: PreSignUp) -> PreSignUp {
    event.response.auto_confirm_user = true;
    event.response.auto_verify_email = false;
    event
}
```

`auto_verify_email` is always false (changed 2026-10-08, review fix). It was once true for every federated signup, which set `email_verified=true` for Facebook users as well and defeated the Google-only rule for bound invites (§4.2). `email_verified` now comes only from the Google IdP's attribute mapping. Nothing had been deployed when this changed, so no user pool holds users created under the old behaviour.

The SPA preserves the invite code across the OAuth round-trip by passing `{ invite, returnTo }` as `oidc-client-ts`'s `signinRedirect({ state })` data. The library keeps that data client-side, keyed by the opaque OIDC `state` nonce, so nothing user-controlled travels through Cognito ([`frontend.md` §6](frontend.md#6-auth-in-the-spa)). A `sessionStorage` stash is the fallback. After token exchange, `/auth/callback` sends the user to `/join?code=…`, which posts the code to `/invites/redeem`.

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

# 2. Bound-email check (D5)
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

**Bound-email check, step 2** ([D5](../decisions.md#d5)). `jwt.email_verified` comes from Cognito's `email_verified` attribute, which only Google populates via the IdP attribute mapping ([`infrastructure.md` §4.2](infrastructure.md#42-identity-providers-federated)); Facebook doesn't supply it. So a `boundEmail` invite can only be redeemed by signing in with Google using that exact (lowercased) address — a Facebook sign-in, or a Google sign-in with a different or unverified email, gets `INVITE_EMAIL_MISMATCH`. This is the mechanism the prod bootstrap admin uses (§9). The check actually runs after step 5's "already a member" short-circuit rather than before it (an idempotent retry by an existing member still returns 200, and nothing is consumed on that path, so no access is granted). API Gateway's HTTP API JWT authorizer hands claims to the Lambda as strings, so the handler treats `email_verified` as true only for JSON `true` or the string `"true"` (case-insensitive); a missing claim is false.

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

There is none: the `User` row is created lazily by `/invites/redeem` (§4), so no Cognito-side trigger is needed.

---

## 6. JWT verification on the backend

API Gateway's HTTP API JWT authorizer verifies the bearer token — issuer, `aud`/`client_id`, JWKS signature, `exp`/`nbf` — before a request ever reaches a Lambda. In `dev` the audience list is `[frontend_client_id, bootstrap_client_id]`; in `prod` it's `[frontend_client_id]` only, since `admin-bootstrap` doesn't exist there ([`infrastructure.md` §4.3](infrastructure.md#43-app-clients)/§6.1). There is no in-Lambda JWT verifier and no local API Gateway proxy — handlers only ever run behind this authorizer ([`testing-and-ci.md` §2.1](../development/testing-and-ci.md#21-unit-in-crate)).

Validated claims land in `event.requestContext.authorizer.jwt.claims`; handlers read them (`sub`, `email`, `email_verified`, `name`, `cognito:username`) via `extract_claims` and trust them without re-verifying the signature. The bearer must be the **ID token** — `email`/`email_verified`/`name` aren't on the access token, and `/invites/redeem` (§4) needs them.

Every protected handler then resolves `claims.sub` → our internal `userId` via the `CognitoSubLookup` row ([`data-model.md` §2.1a](data-model.md#21a-cognito-sub-lookup), `resolve_user_id` in `backend/crates/persistence/src/auth.rs`), cached for ≤60s per warm container alongside the membership cache (§11) to amortize the lookup. A cache miss that also misses DynamoDB means the JWT is valid but the account has never redeemed an invite — `/config` renders that as empty memberships; every other handler returns `UNAUTHENTICATED`.

---

## 7. Token lifetime and refresh

- Access token: 60 min
- ID token: 60 min
- Refresh token: 30 days

The frontend renews in `auth/renewSession.ts`, shortly before expiry (`accessTokenExpiring`), on a reload with an expired ID token, and after a 401. Cognito has no `prompt=none` iframe flow, so federated sessions use the **refresh-token grant** against `/oauth2/token`. Bootstrap-login sessions (§9.2) hold a refresh token for the `admin-bootstrap` client, so they renew with Cognito `InitiateAuth` `REFRESH_TOKEN_AUTH` against that client. If renewal fails (refresh token expired or revoked), the stored user is dropped and protected routes bounce to login.

**Tokens live in `localStorage`** ([D1](../decisions.md#d1)). `oidc-client-ts`'s `userStore` — the ID/access/refresh tokens — persists in `localStorage` rather than `sessionStorage`, so an installed PWA stays signed in across separate launches for the full 30-day refresh-token lifetime instead of losing its session the moment the tab/app closes. `inviteStash` (§4.5, §2) is unaffected and stays in `sessionStorage`. See §8 (logout) and §12 (threat model) for the consequences.

---

## 8. Logout

`logout()` in `auth/AuthProvider.tsx` ([D1](../decisions.md#d1)):
1. Removes the stored user from `localStorage` (was `sessionStorage`; see §7).
2. Clears React Query cache.
3. Clears Zustand persisted stores.
4. **Revokes the refresh token** via Cognito `POST /oauth2/revoke` (best effort — a network failure here doesn't block logout; the client-side token removal in step 1 already ends the session locally).
5. Redirects to Cognito's `/logout` endpoint with `client_id` and `logout_uri=https://opennewsletter.example.com/`. Cognito's OIDC discovery document has no `end_session_endpoint`, so the SPA builds this URL from `VITE_COGNITO_HOSTED_DOMAIN` itself.

Cognito invalidates the SSO session. The revoke call means a token captured before logout (e.g. by XSS, §12) can't be refreshed past this point even though it was read from `localStorage` rather than `sessionStorage`. The user lands back at the SPA logged out.

---

## 9. Bootstrap admin & first group

There must be a way to create the very first group + first admin without an invite code (chicken-and-egg). This is solved with a one-shot script invoked by the operator. Dev and prod follow different flows ([D5](../decisions.md#d5)): dev uses a password-bootstrap flow (§9.1); prod has no password auth at all — it bootstraps via an email-bound admin invite redeemed through Google (§9.1a).

### 9.1 `scripts/bootstrap_admin.py` — dev

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
   - Put the group's first `voting` Newsletter ([`lifecycle.md` §4.2](lifecycle.md#42-default-schedule-per-group))

   IDs are UUID5-derived from the Cognito sub, so re-running the script is idempotent.
7. Prints the group ID and a sign-in URL.

### 9.1a `scripts/bootstrap_admin.py` — prod ([D5](../decisions.md#d5))

Inputs: `--env prod`, `--group-name`, `--timezone`; the admin's email comes from `config.bootstrap_admin_email` (`BOOTSTRAP_ADMIN_EMAIL`, [`infrastructure.md` §2](infrastructure.md#2-configpy)), not a CLI flag, since it must match what the operator later signs in with.

Flow:
1. Reads CDK outputs the same way as dev.
2. Creates **no Cognito user**. Issues a `TransactWriteItems` against DynamoDB with:
   - Put `Group` (META) with the chosen settings + **`memberCount = 0`** (there's no member yet — redemption bumps it, same as any other invite join)
   - Put the group's first `voting` Newsletter ([`lifecycle.md` §4.2](lifecycle.md#42-default-schedule-per-group))
   - Put an `Invite` ([`data-model.md` §2.4](data-model.md#24-invite)) with `bound_email = BOOTSTRAP_ADMIN_EMAIL.lower()`, `role_on_redeem = admin`, `ttlDays = 30`
3. Prints the `/join?code=…` URL for that invite.
4. The operator opens the URL and signs in with **Google**, using the bootstrap admin email. The normal redeem path (§4) creates the `User` row and `GroupMembership` exactly as it would for any other invite — the bound-email check (§4.2 step 2) passes because it's the same email, and Google supplies `email_verified` ([`infrastructure.md` §4.2](infrastructure.md#42-identity-providers-federated)). No special-cased bootstrap code runs on the redeem side at all; §9.1a's only job is minting the right invite.

A bound invite must be redeemed via Google — Facebook doesn't supply `email_verified` (§4.2). If the operator's primary account is Facebook-only, they need a Google account with the bootstrap email for this one step.

### 9.2 Sign-in for the bootstrap admin

**Dev**: the bootstrap admin uses a separate **`admin-bootstrap`** Cognito app client that allows `ALLOW_USER_PASSWORD_AUTH` (dev-only, [`infrastructure.md` §4.3](infrastructure.md#43-app-clients)). The frontend has a hidden route `/admin/bootstrap-login` (only enabled in dev or via a feature flag set in `env.ts`) that posts username+password directly to Cognito and stores the resulting tokens.

**Prod**: there is no bootstrap-specific sign-in path. The operator signs in through the normal Google OAuth flow like any other user (§2); redeeming the bound invite (§9.1a step 4) is what grants admin membership. The old password-auth-then-federate-later approach (and its deferred `admin-link-provider-for-user` idea) no longer applies in prod.

### 9.3 Subsequent admin actions

Once the bootstrap admin exists (dev: the password account; prod: the Google-federated account created at redemption), they:
1. Sign in.
2. Open `/g/:g/admin` → Invites tab → "Generate invite" with role=admin → send the URL to another founding member if desired.
3. **Dev only**: redeem a fresh admin invite on a federated account so that account becomes the canonical admin, then optionally demote or remove the password-bootstrap account.

---

## 10. Identity provider setup: design-relevant facts

Outside of CDK, a human operator registers Google and Facebook as IdPs, generates VAPID keys, and sets `BOOTSTRAP_ADMIN_EMAIL` for prod (§9.1a). The step-by-step procedure, exact commands and redirect URIs are in [`operator-setup.md`](../operations/operator-setup.md); this section keeps only the facts that shape the design:

- **Redirect URI.** Both IdP apps (Google, Facebook) must list `https://{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com/oauth2/idpresponse` as an allowed redirect — the Cognito hosted UI's callback, not the SPA's own `/auth/callback`.
- **Attribute mapping and why.** `email -> email`, `name -> name` for both IdPs; Google additionally maps `email_verified -> email_verified`. Facebook doesn't supply `email_verified`, which is why a `boundEmail` invite can only be redeemed via Google (§4.2, §9.1a).
- **One combined secret, not one per credential** ([D7](../decisions.md#d7)). The Google/Facebook OAuth secrets and VAPID keys land as JSON keys inside the **one** per-env Secrets Manager secret `opennewsletter/{env}` (`app_secret_arn`, [`infrastructure.md` §2](infrastructure.md#2-configpy)/§7.1/§12), merged with a single `jq` recipe rather than created as separate secrets. `BOOTSTRAP_ADMIN_EMAIL` is a plain config value, not a secret — it's a repo variable in CI and an `.env.local` entry locally (`infra/opennewsletter/config.py`).

---

## 11. Tenant authorization helper

`require_membership(repo, user_id, group_id, require_admin)` in `backend/crates/persistence/src/auth.rs` is the tenant-isolation gate every protected handler calls before touching group data. It checks a `DashMap` (`MEMBERSHIP_CACHE`, `once_cell::sync::Lazy`, keyed `(UserId, GroupId)`) fresh for `MEMBERSHIP_CACHE_TTL_SECONDS` (60s, `shared::config`); on a miss it loads the membership from DynamoDB, caches it, and returns `ApiError::forbidden` if the caller isn't a member — or isn't an admin, when `require_admin` is set. The same module's `resolve_user_id`/`require_user_id` do the `sub` → `userId` lookup from §6 with the same cache pattern, and `invalidate_membership`/`cache_user_id` let a handler evict or seed an entry when membership or identity changes mid-request. Both caches live for the duration of the Lambda's warm container.

---

## 12. Threat model considerations

- **Forged invites**: invite codes are 16-char Crockford base32 (≈ 80 bits of entropy). Brute-force trying random codes is infeasible. There is no per-user rate limit on `POST /invites/redeem`: HTTP APIs can't throttle per caller, so only the stage-wide 25 rps / 50 burst limit ([`infrastructure.md` §6.1](infrastructure.md#61-http-api-apigatewayv2)) applies. The code entropy is what protects it.
- **Reusing invites**: prevented by `TransactWriteItems` condition on `status=pending`.
- **Privilege escalation**: only admins can call admin routes; admins are scoped to one group at a time. The auth helper enforces this on every handler.
- **JWT confusion**: API Gateway validates audience and issuer. Backend trusts `sub` only.
- **Token theft via XSS** ([D1](../decisions.md#d1)): tokens live in `localStorage` rather than `sessionStorage` (§7), so an installed PWA stays signed in across launches. XSS could read tokens from either storage equally, so that part of the exposure is unchanged; the difference is lifetime — a stolen refresh token out of `localStorage` stays valid for up to 30 days unless revoked, versus disappearing when the tab closed under a `sessionStorage` scheme. CSP (restricting script/connect/img sources, [`frontend.md` §12.1](frontend.md#121-content-security-policy)) remains the primary mitigation against the XSS that would be needed to read it in the first place. Logout's refresh-token revocation (§8) is the secondary mitigation: it closes the window for a token stolen before logout. Shared-device exposure (a draft mirror or cached media images surviving logout) is accepted as-is — a pre-existing risk this doesn't materially worsen ([D9](../decisions.md#d9)).
- **Leaked presigned media URL**: a presigned S3 GET URL is scoped to one object and expires after `MEDIA_URL_TTL_SECONDS` (1 hour); a URL can also die earlier if the signing Lambda's own role credentials expire first. A leaked URL grants read access to that one image or avatar for at most an hour, never to the group's other images. See [`media.md` §4.2](media.md#42-url-lifetime-and-signing-window)/§6.

---

## 13. Sequence summary

See the diagram in §2 — it's the full sequence; this section is intentionally left as a pointer rather than a second copy.
