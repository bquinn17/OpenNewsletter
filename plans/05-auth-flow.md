# 05 — Auth Flow

Authentication uses Amazon Cognito with three federated IdPs (Google, Apple, Facebook). Self-signup is disabled in the user pool; users can only enter the system by redeeming an invite code. Existing users can join additional groups by redeeming additional invites.

This document covers Cognito configuration, the OAuth round-trip, invite redemption (both first-time and additional), the bootstrap admin script, and the JWT verification path on the backend.

---

## 1. Cognito user pool configuration recap

(Detailed in `01-infrastructure-cdk.md` §4. Key points reiterated here.)

- **Self sign-up: DISABLED.** All signups happen via federation, gated by the `PreSignUp` Lambda trigger.
- **Custom attribute**: `custom:pendingInvite` (mutable string).
- **Triggers**:
  - `PreSignUp` — invite validation + reservation
  - `PostConfirmation` — User + GroupMembership creation
- **App client**: Authorization Code with PKCE, public client (no secret), Cognito disabled-as-IdP for end users.
- **Bootstrap admin client**: separate app client `admin-bootstrap` that allows USER_PASSWORD_AUTH for the bootstrapped admin (only used by `scripts/bootstrap_admin.py`).

---

## 2. End-to-end first-signup flow

Goal: a person with an invite URL becomes a Cognito user AND is added to a group atomically.

```
┌─────────┐                     ┌──────────┐                   ┌─────────┐                ┌──────────┐
│ Browser │                     │ GH Pages │                   │ Cognito │                │  Lambda  │
│         │                     │   SPA    │                   │  Hosted │                │ Triggers │
│         │                     │          │                   │   UI    │                │          │
└────┬────┘                     └─────┬────┘                   └────┬────┘                └────┬─────┘
     │                                │                             │                          │
     │  1. visits /join?code=XYZ      │                             │                          │
     │ ──────────────────────────────►│                             │                          │
     │                                │                             │                          │
     │                                │  2. stash code in sessionStorage                       │
     │                                │     redirect to Cognito hosted UI                      │
     │                                │     (clientMetadata.invite=XYZ via state param echo)   │
     │ ◄──────────────────────────────│                             │                          │
     │                                                              │                          │
     │  3. picks IdP (Google), signs in there, returns to Cognito                              │
     │ ────────────────────────────────────────────────────────────►│                          │
     │                                                              │                          │
     │                                                              │  4. PreSignUp trigger    │
     │                                                              │ ────────────────────────►│
     │                                                              │                          │
     │                                                              │  5. validate invite,     │
     │                                                              │     reserve atomically   │
     │                                                              │     (DDB Update with     │
     │                                                              │      condition)          │
     │                                                              │ ◄────────────────────────│
     │                                                              │                          │
     │                                                              │  6. PostConfirmation     │
     │                                                              │ ────────────────────────►│
     │                                                              │                          │
     │                                                              │  7. consume reservation, │
     │                                                              │     create User +        │
     │                                                              │     GroupMembership      │
     │                                                              │     (TransactWriteItems) │
     │                                                              │ ◄────────────────────────│
     │                                                              │                          │
     │  8. redirected back to /auth/callback?code=...               │                          │
     │ ◄────────────────────────────────────────────────────────────│                          │
     │                                │                             │                          │
     │  9. SPA exchanges code for tokens                            │                          │
     │     (PKCE; clientMetadata not needed here)                   │                          │
     │ ──────────────────────────────►│ ───────────────────────────►│                          │
     │                                │                             │                          │
     │ 10. tokens; SPA loads /config and renders Home               │                          │
```

---

## 3. PreSignUp Lambda

Crate: `lambda-invites` (it has both the API endpoints and the Cognito triggers — they share the persistence module). Distinct binary entry points.

### 3.1 Inputs

The Cognito event includes:
- `request.userAttributes.email`
- `clientMetadata` map — populated from the OAuth `clientMetadata` parameter the SPA passes during the authorize call

The SPA passes `clientMetadata.invite` by appending `&clientMetadata=...` and `&state=...`. The hosted-UI flow propagates these through the OAuth dance via the `clientMetadata` parameter on `oidc-client-ts`'s `signinRedirect({ extraQueryParams: { ... } })` — but actually Cognito's hosted UI does NOT forward `clientMetadata` from the front channel. The robust approach:

**Use the `state` parameter.** The SPA Base64-encodes `{ invite: "XYZ", returnTo: "/" }` into `state` before redirect. The Cognito callback returns this `state` to the SPA. After token exchange, the SPA discovers the user is *new* (no User row in DynamoDB → `/config` returns `pendingInvite: true`) and posts the invite to `POST /invites/redeem` to finalize.

This means the actual flow is:

> The `PreSignUp` trigger does NOT see the invite. It only checks that an account *can* be created (allows OAuth-flagged users through). The **post-token** `/invites/redeem` call from the SPA does the linking, and its handler does both invite consumption AND group membership creation.

This simplifies the system materially. Revised flow in §4 below.

### 3.2 Why this is fine

- Without an invite redemption, the user has zero memberships → `/config` returns empty groups → SPA redirects to `/join`. They cannot reach any group's data because every protected route checks membership.
- The user IS in Cognito but completely inert. We accept the small downside of having user-pool entries that never finished onboarding (cleaned up via a periodic job — see `10-archival.md` §6).

### 3.3 PreSignUp logic (final)

```rust
// returns auto-confirm true so federated users skip email confirm
fn handle(event: PreSignUp) -> PreSignUp {
    event.response.auto_confirm_user = true;
    event.response.auto_verify_email = matches!(event.trigger_source.as_str(), "PreSignUp_ExternalProvider");
    event
}
```

Pure pass-through. Invite validation moved to `POST /invites/redeem`.

---

## 4. Invite redemption (`POST /invites/redeem`)

The single, canonical place where invites are consumed.

### 4.1 Inputs

- `Authorization: Bearer {idToken or accessToken}` — caller is authenticated
- Body: `{ "code": "ABCD-EFGH-JKMN-PQRS" }`

### 4.2 Algorithm

```
caller_id = jwt.sub
code = body.code

# 1. Look up invite
invite = GetItem(pk=INVITE#{code}, sk=META)
if invite is None: return 400 INVITE_INVALID
if invite.status == "consumed": return 409 INVITE_CONSUMED
if invite.status == "revoked": return 410 INVITE_INVALID
if now() > invite.expiresAt: return 410 INVITE_EXPIRED

# 2. Look up group + check member cap
group = GetItem(pk=GROUP#{invite.groupId}, sk=META)
if group.memberCount >= group.memberSoftCap: return 409 MEMBER_CAP_REACHED

# 3. Look up caller's existing User row (it may not exist yet)
user = GetItem(pk=USER#{caller_id}, sk=PROFILE)

# 4. Determine if a User row needs to be created
items_to_write = []
if user is None:
    items_to_write.append(Put(User row, derived from JWT email/name))

# 5. Atomic transaction (TransactWriteItems)
items_to_write += [
    Update(invite,
        Condition: status = "pending" AND now < expiresAt,
        Set: status = "consumed", consumedBy = caller_id, consumedAt = now()),
    Put(GroupMembership(userId=caller_id, groupId=invite.groupId, role=invite.roleOnRedeem),
        Condition: attribute_not_exists(pk)),  # idempotent on retry, but a duplicate join is a programmer error
    Update(group, ADD memberCount 1,
        Condition: memberCount < memberSoftCap),
]

# 6. If the membership Put fails because the row already exists, the user is already a member.
#    Detect this case before transaction and short-circuit with 200.

# Return success
```

### 4.3 First-signup vs additional-group

The same algorithm covers both cases:
- First signup: `User` row is missing → created.
- Additional group: `User` row exists; only the `Invite` and `GroupMembership` (and group counter) change.

### 4.4 Idempotency

Re-POSTing the same code with the same caller after success: invite is `consumed`, so we return 409 `INVITE_CONSUMED`. To make the SPA's life simpler, the handler ALSO checks whether `consumedBy == caller_id`; if yes, it returns 200 with the membership info (this happens if the user double-submits or the network retries).

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
- Validates `aud` (or `client_id` for access tokens) = the frontend client ID
- Validates signature against JWKS
- Validates `exp` and `nbf`

When the token is valid, the JWT claims are placed in `event.requestContext.authorizer.jwt.claims`. The Rust handlers parse this via:

```rust
#[derive(Deserialize)]
struct AuthClaims {
    sub: String,                // user id
    email: Option<String>,
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

Defense-in-depth for local development: the Rust shared crate has a `JwtVerifier` that fetches JWKS and validates signatures. This is gated behind the `ENV=local` flag (used by `scripts/run_local.sh`, which proxies HTTP requests to Lambdas without API Gateway).

---

## 7. Token lifetime and refresh

- Access token: 60 min
- ID token: 60 min
- Refresh token: 30 days

Frontend uses `oidc-client-ts` `automaticSilentRenew: true`. Silent renew uses the `prompt=none` Cognito flow. If silent renew fails (refresh token expired or revoked), the user is bounced to login.

---

## 8. Logout

`logout()` in `auth/login.ts`:
1. Clears tokens from sessionStorage.
2. Clears React Query cache.
3. Clears Zustand persisted stores.
4. Redirects to Cognito's `/logout` endpoint with `client_id` and `logout_uri=https://opennewsletter.example.com/`.

Cognito invalidates the SSO session. The user lands back at the SPA logged out.

---

## 9. Bootstrap admin & first group

There must be a way to create the very first group + first admin without an invite code (chicken-and-egg). This is solved with a one-shot script invoked by the operator.

### 9.1 `scripts/bootstrap_admin.py`

Inputs (CLI flags or env):
- `--env dev|prod`
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
   - Put `User` (PROFILE)
   - Put `Group` (META) with the chosen settings + `memberCount=1`
   - Put `GroupMembership` (role=admin)
7. Prints the group ID and a sign-in URL.

### 9.2 Sign-in for the bootstrap admin

The bootstrap admin uses a separate **`admin-bootstrap`** Cognito app client that allows `ALLOW_USER_PASSWORD_AUTH`. The frontend has a hidden route `/admin/bootstrap-login` (only enabled in dev or via a feature flag set in `env.ts`) that posts username+password directly to Cognito and stores the resulting tokens.

In production, after bootstrap, the admin should immediately federate their account by signing in via Google/Apple/Facebook with the same email — Cognito will treat that as a separate user. We address this by also creating a federated-link via `admin-link-provider-for-user` if the operator passes `--federate-google` etc. (deferred — for v1 the bootstrap admin can simply use password auth).

### 9.3 Subsequent admin actions

Once the bootstrap admin exists, they:
1. Sign in.
2. Open `/g/:g/admin` → Invites tab → "Generate invite" with role=admin → send the URL to themselves (or another founding member).
3. Redeem on a federated account → that account becomes the canonical admin.
4. (Optional) Demote the bootstrap account to `member` or remove it.

---

## 10. Identity provider setup checklist (operator runbook)

Outside of CDK, a human operator must:

- [ ] **Google**: Create OAuth 2.0 client in Google Cloud Console. Authorized redirect URI: `https://{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`. Authorized JS origin: `https://opennewsletter.example.com`. Save Client ID + Client Secret to AWS Secrets Manager.
- [ ] **Apple**: Create Services ID in Apple Developer. Configure Sign In With Apple. Generate a private key. Domain & redirect URL: same Cognito callback. Save team ID, key ID, private key to Secrets Manager.
- [ ] **Facebook**: Create app in Meta for Developers. Add Facebook Login. Valid OAuth redirect URI: same Cognito callback. Save App ID + Secret to Secrets Manager.
- [ ] **VAPID keys**: `python scripts/generate_vapid_keys.py` → store JSON in `opennewsletter/vapid/{env}` Secrets Manager secret.
- [ ] **CloudFront signing keypair**: generate RSA 2048; upload public to CloudFront (CDK does this from a public-key file in `infra/keys/`); store private key in Secrets Manager `opennewsletter/cdn-signing/{env}`.

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

- **Forged invites**: invite codes are 16-char Crockford base32 (≈ 80 bits of entropy). Brute-force trying random codes is infeasible. Rate-limit `POST /invites/redeem` to 10/min/user via API Gateway throttling.
- **Reusing invites**: prevented by `TransactWriteItems` condition on `status=pending`.
- **Privilege escalation**: only admins can call admin routes; admins are scoped to one group at a time. The auth helper enforces this on every handler.
- **JWT confusion**: API Gateway validates audience and issuer. Backend trusts `sub` only.
- **Token theft via XSS**: tokens live in `sessionStorage`; CSP set to default-src 'self', script-src 'self', img-src 'self' https://cdn.opennewsletter.example.com data:, etc. (See `04-frontend-architecture.md` build step — set in `index.html` and verified in tests.)
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
