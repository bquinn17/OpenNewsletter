# Infrastructure

This document specifies every AWS resource OpenNewsletter provisions, organized into the CDK stacks under `infra/opennewsletter/`. It's for anyone writing or reviewing CDK code, or tracing a deployed resource back to where it's defined. The CDK code itself is the final authority on what's actually deployed; this page documents the design and the reasoning behind it. [`api.md` §13](api.md#13-per-route-authrole-summary-table) is the authoritative route ⇄ Lambda ⇄ role table that `api_stack.py` is built from (§6.2).

Region: **us-east-1**. Account: single personal AWS account for both `dev` and `prod`, distinguished by stack suffix (`-dev`, `-prod`). See [`overview.md` §7](../overview.md#7-environments) for the rationale.

---

## 1. Stack inventory

| Stack | Purpose | Depends on |
|---|---|---|
| `DataStack` | DynamoDB table + GSIs | — |
| `AuthStack` | Cognito user pool, IdPs, hosted UI domain | — |
| `MediaPersistentStack` | S3 buckets (originals + processed), read by presigned URL (§5.2) | `DataStack` |
| `MediaPipelineStack` | `lambda-image-process` and its S3 event subscription | `DataStack`, `MediaPersistentStack` |
| `ApiStack` | HTTP API, request Lambdas, routes, JWT authorizer | `DataStack`, `AuthStack`, `MediaPersistentStack`, `NotificationsStack` |
| `NotificationsStack` | EventBridge schedules, tick Lambdas, VAPID secret | `DataStack` |
| `FrontendStack` | ACM cert (`us-east-1`) + Route53 records (if Route53) | — |
| `MonitoringStack` | CloudWatch dashboards + alarms | All others |
| `CiStack` | Account-wide, unsuffixed (one per account, not per env): GitHub OIDC provider + deploy/diff roles ([`testing-and-ci.md` §6](../development/testing-and-ci.md#6-github-actions-workflows)), and the account's one AWS Budget (§9.2a) | — |

CDK app entry point: `infra/app.py`. Uses `--context env=<dev|prod>` to select environment-specific config from `infra/opennewsletter/config.py`.

**Dependency direction**: `ApiStack` depends on `NotificationsStack`, not the reverse — the dev-only `POST /admin/dev/tick/cycle` route (§6, §11a) wires directly to `NotificationsStack`'s `cycle-tick` Lambda, so `ApiStack` receives it as a constructor parameter. `app.py` instantiates `NotificationsStack` before `ApiStack` accordingly. `lambda-cycle-tick` is the one that invokes `lambda-push` (notify-tick sends reminders in-process, §7.3); that grant is on an ARN built with `Stack.format_arn` inside `NotificationsStack`, rather than on the live `ApiStack` construct, so no CDK-level cycle is created (§7.2).

---

## 2. `config.py`

```python
from dataclasses import dataclass

@dataclass(frozen=True)
class EnvConfig:
    env: str                          # "dev" | "prod"
    domain: str                       # ROOT_DOMAIN; defaults to "{env}.opennewsletter.example.com". Dev never attaches it: raw AWS endpoints (docs/development/environments.md §2)
    api_domain: str                   # API_DOMAIN; defaults to "api-{env}.opennewsletter.example.com"
    hosted_zone_id: str | None        # if Route53, else None (DNS managed externally)
    hosted_zone_name: str | None
    cognito_domain_prefix: str        # globally unique; e.g. "opennewsletter-prod"
    app_secret_arn: str               # ARN of the one per-env Secrets Manager secret (§7.1, §12)
    log_retention_days: int           # 30 in dev, 90 in prod
    alarm_email: str | None
    bootstrap_admin_email: str | None # prod only; target of the email-bound admin invite scripts/bootstrap_admin.py creates (docs/design/auth.md §9)
    github_repo: str                  # "owner/repo" slug CiStack's GitHub OIDC trust conditions match
```

Provider OAuth credentials and VAPID keys are pre-populated in AWS Secrets Manager out-of-band (not in CDK), as JSON keys inside the one per-env secret `opennewsletter/{env}` referenced by `app_secret_arn` (§7.1, §12). `config.py` only stores the ARN.

---

## 3. `DataStack`

### 3.1 DynamoDB table `OpenNewsletter-{env}`

- **Billing**: `PAY_PER_REQUEST`
- **Partition key**: `pk` (String)
- **Sort key**: `sk` (String)
- **Time-to-live attribute**: `ttl` (Number, epoch seconds) — used for invite expirations and ephemeral entities
- **Streams**: **disabled in v1**. The only planned consumer is the archival Lambda ([`archival.md`](archival.md)), which is deferred. Re-enable as `NEW_AND_OLD_IMAGES` when archival lands.
- **Point-in-time recovery**: enabled in `prod`, disabled in `dev`
- **Removal policy**: `RETAIN` in `prod`, `DESTROY` in `dev`
- **Encryption**: AWS-owned key (free; the AWS-managed `aws/dynamodb` KMS key bills per-key)

### 3.2 Global Secondary Indexes

- **GSI1** — generic inversion index
  - PK: `gsi1pk` (String)
  - SK: `gsi1sk` (String)
  - Projection: `ALL`
  - Used to find: members of a group, invites by group, newsletters by status, push subs by user, etc.
- **GSI2** — by-status / by-time index
  - PK: `gsi2pk` (String)
  - SK: `gsi2sk` (String)
  - Projection: `ALL`
  - Used to find: newsletters across all groups by status, scheduled-soon items

See [`data-model.md`](data-model.md) for exact key compositions.

### 3.3 Outputs (for cross-stack reference)

- `table_name`
- `table_arn`

---

## 4. `AuthStack`

### 4.1 Cognito User Pool

- **Name**: `OpenNewsletter-{env}`
- **Sign-in aliases**: email
- **Self-signup**: **disabled** (signup goes through invite redemption — see [`auth.md`](auth.md))
- **Password policy**: 12 chars min, require lowercase, uppercase, digit, symbol (used only for the bootstrap admin path; OAuth users don't see this)
- **MFA**: optional, TOTP only
- **Account recovery**: email
- **Standard attributes**: email (required), name (optional)
- **Custom attributes**: none. (The invite code survives the OAuth round-trip inside the OIDC `state` parameter managed by the SPA — see [`auth.md` §3](auth.md#3-presignup-lambda) — so no Cognito-side storage is needed.)
- **Lambda triggers**:
  - `PreSignUp` Lambda (Rust, in `lambda-invites` crate) — **pure pass-through** that auto-confirms federated accounts (`auto_confirm_user = true`). It does NOT see, validate, or consume invite codes. Invite consumption happens after login via `POST /invites/redeem` (§6.5 below and [`auth.md` §4](auth.md#4-invite-redemption-post-invitesredeem)), which also lazily creates the `User` row on first redemption.
  - There is **no `PostConfirmation` trigger** ([`auth.md` §5](auth.md#5-postconfirmation-trigger)).

### 4.2 Identity Providers (federated)

- **Google** — `UserPoolIdentityProviderGoogle`. Reads `google_client_id`/`google_client_secret` via CFN dynamic references against the app secret (`app_secret_arn`), e.g. `{{resolve:secretsmanager:<app_secret_arn>:SecretString:google_client_id}}`.
- **Facebook** — `UserPoolIdentityProviderFacebook`. Same pattern against `facebook_app_id`/`facebook_app_secret`.

No Apple IdP: Sign In with Apple requires a paid ($99/yr) Apple Developer membership, which the owner declined, so only Google and Facebook are configured.

For both: attribute mapping `email -> email`, `name -> name`. Google additionally maps `email_verified -> email_verified`, needed so `POST /invites/redeem` can check a bound invite's email-verified claim ([`auth.md` §4.2](auth.md#42-algorithm)/§9). Facebook doesn't supply `email_verified`, so an email-bound invite can only be redeemed via Google.

### 4.3 App Clients

Two app clients on the same user pool:

**`frontend`** — the public client every end user goes through.
- **Generate secret**: NO (public client)
- **OAuth flows**: Authorization Code with PKCE
- **OAuth scopes**: `openid`, `email`, `profile`
- **Callback URLs**: `https://{config.domain}/auth/callback`, plus `http://localhost:5173/auth/callback` in `dev`
- **Logout URLs**: `https://{config.domain}/`, plus `http://localhost:5173/` in `dev`
- **Supported IdPs**: Google, Facebook (Cognito itself disabled for end users)
- **Token validity**: ID 60min, Access 60min, Refresh 30 days

**`admin-bootstrap`** — used only by `scripts/bootstrap_admin.py` and the dev-only `/admin/bootstrap-login` page (see [`auth.md` §9.2](auth.md#92-sign-in-for-the-bootstrap-admin)).
- **Generate secret**: NO (public client)
- **Auth flows**: `ALLOW_USER_PASSWORD_AUTH` + `ALLOW_REFRESH_TOKEN_AUTH`
- **Supported IdPs**: Cognito only (no federation)
- **Token validity**: same as `frontend`
- **Dev only.** In prod, bootstrap no longer creates a Cognito user at all — the operator redeems an email-bound admin invite via Google instead ([`auth.md` §9](auth.md#9-bootstrap-admin--first-group)). So `admin-bootstrap` is created only when `env == dev`: it's the inner-loop sign-in path there so dev iteration doesn't bounce through Google/Facebook every time, and the `ApiStack` JWT authorizer's audience in `prod` is the `frontend` client alone (§6.1).

### 4.4 Hosted UI Domain

- Cognito-managed domain at `{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com`. Custom domain optional in `prod` (defer).

### 4.5 Outputs

- `user_pool_id`
- `user_pool_arn`
- `user_pool_client_id` (the `frontend` client)
- `user_pool_bootstrap_client_id` (the `admin-bootstrap` client; dev only, absent in prod)
- `hosted_ui_domain`

---

## 5. `MediaPersistentStack` and `MediaPipelineStack`

The media resources are split across two stacks so that `dev` can recreate the image-processing pipeline freely without touching the buckets that hold data. See [`environments.md` §3](../development/environments.md#3-stack-lifetime-tiers) for the rationale.

- **`MediaPersistentStack`** — §5.1, §5.2: the three S3 buckets. Long-lived in dev because the buckets hold user uploads.
- **`MediaPipelineStack`** — §5.3: `lambda-image-process` and its S3 event subscription. Volatile; recreated freely.

In `prod` the split is structural-only; both stacks deploy together and behave identically to a combined stack.

### 5.1 S3 buckets (`MediaPersistentStack`)

Three buckets, all private (Block Public Access fully on):

- **`opennewsletter-media-originals-{env}-{accountId}`**
  - Versioning: disabled in both envs. Real delete ([D8](../decisions.md#d8), [`api.md` §9.4](api.md#94-delete-uploadsimageid)) only works cleanly without versioning: a delete on a versioned bucket just adds a delete marker and leaves the object recoverable, which defeats the point of a user-initiated permanent delete.
  - Lifecycle:
    - Abort incomplete multipart uploads after 1 day
    - Expire objects under prefix `uploads/` after 7 days — a safety net for an original `lambda-image-process` should already have deleted on success, on decode failure, or on oversize (§5.3; [`media.md` §5.1](media.md#51-trigger-configuration)). No Intelligent-Tiering transition: originals don't live long enough to tier.
  - CORS: allow `PUT` and `POST` from `https://{config.domain}` and `http://localhost:5173` in dev. Allowed headers: `*`, because the presign decides which headers are signed (e.g. `x-amz-checksum-sha256`, [`media.md` §3.2](media.md#32-presign)). Expose: `ETag`.
  - Notification: `s3:ObjectCreated:*` for prefix `uploads/` → invokes `lambda-image-process`.
  - Encryption: SSE-S3.

- **`opennewsletter-avatars-originals-{env}-{accountId}`** — avatars cross group boundaries, so they don't share the media bucket's group-scoped key layout ([`media.md` §11](media.md#11-avatars-separate-pipeline)).
  - Same CORS, lifecycle (including the 7-day `uploads/` expiry), and encryption settings as media originals; no versioning.
  - Notification: `s3:ObjectCreated:*` for prefix `uploads/` → invokes the same `lambda-image-process` (wired in `MediaPipelineStack`).

- **`opennewsletter-media-processed-{env}-{accountId}`** — one bucket for both pipelines: response images under `img/...` and avatars under `avatar/{avatarId}/display.webp`. (One bucket rather than one per pipeline: their processed objects need the same lifecycle and CORS rules, and the frontend gets a single media origin.)
  - No versioning. Lifecycle: abort incomplete multipart uploads after 1 day only. No Intelligent-Tiering transition: the WebP display/thumb variants are mostly under the 128 KB floor below which Intelligent-Tiering never moves an object to a cheaper tier, so each per-object transition fee would buy nothing.
  - Private; nothing reads it anonymously (§5.2). CORS: allow `GET, HEAD` from `https://{config.domain}` and `http://localhost:5173` in dev, so the browser can load a presigned URL as an `<img crossorigin="anonymous">` ([`frontend.md`](frontend.md)).
  - Read access is per-Lambda IAM, not a bucket policy: each handler that signs a GET URL ([`media.md` §4.3](media.md#43-the-media-urls-crate)) gets `s3:GetObject` scoped to the one prefix it serves — §6.2 below.

### 5.2 Image serving

The processed bucket (§5.1) is private, and every handler that returns an image or avatar URL signs an S3 GET presigned URL against it directly, using its own Lambda execution role, after the membership check the handler already does for that route. That check is the tenant boundary. The bucket's CORS is what lets the browser load those presigned URLs as CORS-mode `<img>` elements. §6.2 below has the per-Lambda signing grants.

See [`decisions.md`](../decisions.md#presigned-s3-media) for the owner decision and [`media.md` §4](media.md#4-serving-presigned-s3-gets) for the full mechanics: URL lifetime, the `media-urls` crate, SPA refresh-on-expiry, and why presigned S3 over the alternatives.

### 5.3 `lambda-image-process` (Rust, `MediaPipelineStack`)

- **Trigger**: S3 ObjectCreated, prefix `uploads/`, on **both** the media originals bucket and the avatars originals bucket. The handler branches on the source **bucket name** (not key prefix — both buckets use `uploads/`) to choose response-image vs avatar processing.
- **Memory**: 1024 MB
- **Timeout**: 60 s
- **Architecture**: arm64 (cheaper)
- **Environment**:
  - `TABLE_NAME`
  - `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET` (the handler compares the event's bucket against these)
  - `PROCESSED_BUCKET` (the one combined bucket, §5.1)
- **IAM**:
  - `s3:GetObject` + `s3:DeleteObject` on both originals buckets — delete purges oversized uploads and decode failures ([`media.md` §3.2](media.md#32-presign)), and the original on a successful process, right after its row is marked `ready` (best effort; logged at `ERROR` on failure, not retried — [`media.md` §5.1](media.md#51-trigger-configuration))
  - `s3:PutObject` on the processed bucket, prefixes `img/*` and `avatar/*`
  - `dynamodb:GetItem` + `dynamodb:UpdateItem` on table (read the row's status, then mark it `ready`/`failed` and store dimensions)
- Logic detailed in [`media.md` §5](media.md#5-lambda-image-process-s3-trigger).

### 5.4 Outputs

- `originals_bucket_name`, `originals_bucket_arn`
- `processed_bucket_name`, `processed_bucket_arn` (the one combined bucket, §5.1)
- `avatars_originals_bucket_name`
- `MediaBaseUrl` — `https://{processed-bucket}.s3.us-east-1.amazonaws.com`, the host every presigned URL resolves to ([`media.md` §4](media.md#4-serving-presigned-s3-gets)). `scripts/write_frontend_env.py` writes it into the frontend build as `VITE_MEDIA_BASE_URL`.
- `UploadOrigins` — the two originals buckets' virtual-hosted-style S3 endpoints, space-separated, written into the frontend build as `VITE_UPLOAD_ORIGINS` for CSP `connect-src` ([`frontend.md` §12.1](frontend.md#121-content-security-policy)).

Nothing media-related lives in the app secret (§7.1, §12): presigned URLs are signed with each handler's own Lambda execution-role credentials ([`media.md` §4.3](media.md#43-the-media-urls-crate)), not a stored key.

---

## 6. `ApiStack`

### 6.1 HTTP API (`apigatewayv2`)

- **Name**: `OpenNewsletter-Api-{env}`
- **CORS**:
  - Allow origins: `https://{config.domain}`, `http://localhost:5173` in dev
  - Allow methods: `GET, POST, PUT, PATCH, DELETE, OPTIONS`
  - Allow headers: `Authorization, Content-Type, x-correlation-id`
  - Expose headers: `x-correlation-id`
  - Allow credentials: **false** — no route sets or needs a cross-origin cookie; the API authenticates with the `Authorization` header, and media with presigned URLs ([`media.md` §4](media.md#4-serving-presigned-s3-gets)).
  - Max age: 600
- **Custom domain**: `{api_domain}` with ACM cert from `FrontendStack`
- **Default authorizer**: `HttpJwtAuthorizer` against the Cognito user pool (issuer URL `https://cognito-idp.us-east-1.amazonaws.com/{user_pool_id}`). In `dev`, audience: `[frontend_client_id, bootstrap_client_id]` — the `admin-bootstrap` client is included so password-auth tokens from `/admin/bootstrap-login` and `scripts/bootstrap_admin.py` pass the same authorizer ([`environments.md` §6](../development/environments.md#6-cognito-strategy-in-dev)). In `prod`, audience: `[frontend_client_id]` only, since `admin-bootstrap` doesn't exist there (§4.3).
- **Throttling**: 50 rps burst, 25 rps steady on default stage
- **Access logs**: JSON to a CloudWatch log group `/aws/http-api/OpenNewsletter-{env}`, 30/90 day retention

### 6.2 Lambda functions (request handlers)

All Rust, runtime `provided.al2023`, architecture `arm64`, memory 256 MB (512 for `lambda-responses`), timeout 10 s, log retention per config. Built with `cargo lambda build --release --arm64`.

The route column below is a **summary only** — [`api.md` §13](api.md#13-per-route-authrole-summary-table) is the authoritative route ⇄ Lambda ⇄ role table, and when they disagree, §13 wins. CDK code should be written from §13.

| Lambda | Route summary | DynamoDB | S3 | Other |
|---|---|---|---|---|
| `lambda-invites` | invite create / list / revoke / redeem; also compiles the `PreSignUp` trigger binary | RW | — | — |
| `lambda-groups` | `GET /config`, `GET /healthz`, `GET/PATCH /me`, group get/list/patch, member kick + role change | RW | `s3:DeleteObject` on avatar originals + processed (old-avatar cleanup on `PATCH /me`, [D8](../decisions.md#d8)); `s3:GetObject` on processed bucket, prefix `avatar/*` (signs `avatarUrl` on `UserResponse`/`MemberResponse` — [`media.md` §4.3](media.md#43-the-media-urls-crate)) | — |
| `lambda-newsletters` | newsletter list + detail | R | `s3:GetObject` on processed bucket, prefixes `img/*` and `avatar/*` (signs `PublishedImageResponse` and `askedBy.avatarUrl`) | — |
| `lambda-questions` | candidate list/create + votes, `DELETE /admin/groups/{g}/candidate-questions/{q}` (admin moderation only) | RW | `s3:GetObject` on processed bucket, prefix `avatar/*` (signs `askedBy.avatarUrl`) | — |
| `lambda-responses` | my-response put (drafts, publish, poll votes) | RW | — | — |
| `lambda-engagement` | comments (CRUD) + reactions (put/delete/get) | RW | `s3:DeleteObject` on media originals + processed (comment-image release on soft-delete/replace, [D8](../decisions.md#d8)); `s3:GetObject` on processed bucket, prefix `img/*` (signs `CommentImageResponse`) | — |
| `lambda-media` | uploads presign/status/caption/delete, avatar presign/status/delete (`/avatars*`) | RW | PUT presign on media + avatar originals; `s3:DeleteObject` on media + avatar originals and processed (`DELETE /uploads`, `DELETE /avatars`, [D8](../decisions.md#d8)); `s3:GetObject` on processed bucket, prefixes `img/*` and `avatar/*` (signs `ImageMediaResponse`/`AvatarMediaResponse`) | — |
| `lambda-push` | subscribe / unsubscribe / list / test / `PUT /push/preferences/{g}`; internal fan-out handlers direct-invoked by the tick Lambdas ([`notifications.md` §6](notifications.md#6-cycle-open-fan-out)) | RW | — | Secrets:GetSecretValue on the app secret (VAPID keys) — the only Lambda that reads the app secret |

**Real media deletion** ([D8](../decisions.md#d8); [`api.md` §9.4](api.md#94-delete-uploadsimageid)/§9.7). A shared helper — the `media-delete` library crate (`backend/crates/media-delete`; not `shared`, which can't depend on `persistence` and so can't delete the row) — deletes an image's S3 objects (original, then both processed variants), then deletes the `ImageMedia`/`AvatarMedia` row. Once an object is gone, its presigned URLs 403 immediately ([`media.md` §8.0](media.md#80-explicit-deletion)). The IAM grants above (`lambda-media`, `lambda-engagement`, `lambda-groups`) are scoped to the bucket prefixes each Lambda actually deletes from.

**Media-URL signing** ([`media.md` §4.3](media.md#43-the-media-urls-crate)). A second shared library crate, `backend/crates/media-urls`, wraps the presigning call every handler listed above uses to turn a stored S3 key into a GET URL, after that handler's usual membership check. Each Lambda's `s3:GetObject` grant is scoped to the one or two prefixes (`img/*`, `avatar/*`) its routes actually return URLs for — no Lambda gets a blanket grant across the whole processed bucket.

The dev-only fast-forward routes (`POST /admin/dev/tick/{cycle|notify}`, [`api.md` §11a](api.md#11a-dev-only-endpoints)) integrate the tick Lambdas from `NotificationsStack` into this API when `env == dev`.

The `PreSignUp` trigger binary is compiled from `lambda-invites` but its CDK `Function` is defined in **`AuthStack`**, not here. `user_pool.add_trigger()` attaches the wiring to the user pool's own stack, so building the function in `ApiStack` makes `AuthStack` depend on `ApiStack` while `ApiStack` already depends on `AuthStack` for the JWT authorizer — a cyclic reference that fails at synth. (Same failure mode as the `MediaPersistentStack`/`MediaPipelineStack` notification bug; the rule is: define a resource in the stack that owns the thing it attaches to.)

All Lambdas share:
- Env: `TABLE_NAME`, `RUST_LOG=info`, `ENV={env}`, `API_BASE_URL` (used to build the absolute RFC-7807 `type` URI)
- `lambda-groups` additionally gets `VAPID_PUBLIC_KEY` (a Secrets Manager dynamic reference against the `vapid_public_key` JSON key of the app secret, echoed by `GET /config` per §7.1)
- `PROCESSED_BUCKET` goes to every Lambda that signs a media URL: `lambda-groups`, `-newsletters`, `-questions`, `-engagement` and `-media` (§6.2's table; which prefix each one may sign is IAM, not env config). `MEDIA_URL_TTL_SECONDS` (1 h) and `MEDIA_URL_SIGN_WINDOW_SECONDS` (5 min) are compile-time constants from `shared::config`, not env vars.
- `lambda-media` additionally gets `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET` (for the upload presigns).
- The HTTP API's CORS does **not** set `allowCredentials` (§6.1).
- IAM (baseline): write access to its own log group only
- DynamoDB IAM: scoped to `arn:aws:dynamodb:...:table/OpenNewsletter-{env}` and the two GSIs

Each Lambda has its own CloudWatch log group with retention from config.

### 6.3 Routes and integrations

For each route in [`api.md`](api.md), CDK creates:
- An `HttpRoute` with the Cognito JWT authorizer attached — every route, including `POST /invites/redeem` (§6.5)
- An `HttpLambdaIntegration` pointing to the appropriate Lambda

### 6.4 Group-default config JSON

The defaults for newly-created groups, served as `groupDefaults` on `GET /config`. `lambda-groups` builds this object from the constants in `backend/crates/shared/src/config.rs` (`shared::config::default_group_settings`) rather than from a copy held in `api_stack.py`: an earlier, now-removed `CONFIG_JSON` env var had already drifted from `config.rs` once (`responseWindowDays` stayed at 4 after the default became 7). The keys are:

```json
{
  "questionsPerCycle": 5,
  "votesPerUserPerCycle": 3,
  "responseWindowDays": 7,
  "timezone": "America/New_York",
  "memberSoftCap": 50,
  "imagesPerAnswerMax": 10,
  "imageMaxBytes": 15728640,
  "inviteTtlDays": 7,
  "autosaveDebounceMs": 1500,
  "notificationOffsetsHoursBeforeClose": [96, 48, 24]
}
```

`responseWindowDays` default is 7 (`DEFAULT_RESPONSE_WINDOW_DAYS` in `shared/src/config.rs`). Existing dev groups keep their stored `responseWindowDays=4` until patched or reseeded. Group settings stored in DynamoDB override these defaults per-group; see [`data-model.md`](data-model.md).

### 6.5 No public routes

**All routes have JWT auth.** Invite redemption happens via `POST /invites/redeem`, which requires a valid Cognito JWT (see [`auth.md` §4](auth.md#4-invite-redemption-post-invitesredeem) for the full flow). The handler performs both first-signup user creation and additional-group joins; there is no unauthenticated path into the API.

### 6.6 Outputs

- `api_id`
- `api_endpoint` (the custom domain URL)

---

## 7. `NotificationsStack`

### 7.1 VAPID secret

VAPID keys don't get a dedicated secret. They're three JSON keys — `vapid_public_key`, `vapid_private_key`, `vapid_subject` — inside the single per-env Secrets Manager secret `opennewsletter/{env}` (§12), alongside `google_client_id`, `google_client_secret`, `facebook_app_id`, `facebook_app_secret`. Its ARN is `app_secret_arn` in `config.py` (§2).

- Generated out-of-band by `scripts/generate_vapid_keys.py` and merged into the one secret manually before first deploy.
- Public key exposed to the frontend via the `GET /config` endpoint (in `lambda-groups`).

### 7.2 `lambda-cycle-tick`

- **Trigger**: EventBridge Scheduler rule running **every 5 minutes**.
- **Memory**: 512 MB. **Timeout**: 60 s.
- **Job**: Open / lock / close / publish cycles whose timestamps have passed. Driven by GSI2. See [`lifecycle.md` §3](lifecycle.md#3-state-machine).
- **IAM**: full RW on table; `lambda:InvokeFunction` on `lambda-push`, scoped to an ARN built with `Stack.format_arn` rather than the live `ApiStack` construct, which would create a stack cycle (§1).

Async-invokes `lambda-push` (`InvocationType=Event`) for the cycle-open and publication fan-outs, found via env `PUSH_FUNCTION_NAME=OpenNewsletter-Push-{env}`.

### 7.3 `lambda-notify-tick`

- **Trigger**: EventBridge Scheduler rule running **every 15 minutes**.
- **Memory**: 512 MB. **Timeout**: 120 s.
- **Job**: Find cycles with deadlines within the next configured offset (96/48/24h) that haven't been notified yet, fan out Web Push messages.
- **IAM**: RW on table, `secretsmanager:GetSecretValue` on the app secret (`app_secret_arn`). No `lambda:InvokeFunction` — it links `lambda-push`'s library and sends reminders in-process; only `lambda-cycle-tick` invokes `lambda-push`.
- See [`notifications.md`](notifications.md) for full logic.

### 7.4 Idempotency

Both tick Lambdas write a `TICK` sentinel record (`sk=CYCLE` / `sk=NOTIFY`, with `last_ran_at`). Notification dispatch idempotency uses per-cycle markers in the cycle's partition: `NOTIFIED#OPEN`, `NOTIFIED#CLOSE#{offset_hours}` and `NOTIFIED#PUBLISH` ([`data-model.md` §2.15](data-model.md#215-counters-and-idempotency-markers)).

---

## 8. `FrontendStack`

The frontend itself ships to GitHub Pages — CDK doesn't host it. This stack manages domain plumbing only.

### 8.1 ACM certs

- One cert in `us-east-1` covering `{config.domain}`, `{api_domain}`, with DNS validation.
- If `hosted_zone_id` is set, validation records auto-created via Route53. Else, output the validation CNAMEs and stop — operator finishes DNS manually.

### 8.2 Route53 (only if `hosted_zone_id` is set)

- A `CNAME` for `{config.domain}` pointing to `{githubuser}.github.io`. Note GitHub Pages also requires apex-A records — operator must add per GitHub docs if using apex.
- An `A`-alias for `{api_domain}` pointing to the HTTP API custom domain.

### 8.3 If DNS is external

Stack outputs the required records as `CfnOutput` so the operator can copy them.

---

## 9. `MonitoringStack`

### 9.1 Dashboard

A single CloudWatch dashboard `OpenNewsletter-{env}` with widgets for:
- HTTP API 4xx/5xx rates and p50/p95/p99 latency per route
- Per-Lambda invocation count, error rate, duration p95, throttles
- DynamoDB consumed RCU/WCU, throttles, conditional check failures
- S3 storage: `BucketSizeBytes` and `NumberOfObjects` per media bucket (daily, free)
- Web Push delivery success/failure (custom metric emitted by `lambda-notify-tick` and `lambda-push`)

### 9.2 Alarms

**Prod only, ≤10 alarm metrics total** (dev keeps the dashboard in §9.1 but ships no alarms, so dev stays inside the 10-free-alarm-metrics CloudWatch tier at $0):

- Lambda error rate across all functions, account-level aggregate `AWS/Lambda` `Errors`/`Invocations` (2 metrics) → SNS topic. Replaces a per-Lambda >5% alarm, which would have needed one metric pair per function.
- HTTP API 5xx rate >1% over 10 minutes (2 metrics) → SNS topic
- DynamoDB read + write throttle events >0 over 5 minutes (2 metrics) → SNS topic
- Push delivery failure rate >20% rolling (2 metrics) → SNS topic
- `lambda-cycle-tick` errors, any, plus the tick's `AutoPublishAbandoned` EMF metric (2 metrics, `FILL(errors,0) + FILL(abandoned,0) ≥ 1`) → SNS topic. The tick logs per-cycle failures rather than failing the invocation, so Lambda `Errors` alone missed auto-publish giving up on drafts ([`lifecycle.md` §5.3](lifecycle.md#53-publishnl)).

That's all 10 of the free alarm metrics, so a new alarm must replace or merge into one of these; `lambda-notify-tick` failures are not separately alarmed (its failures mostly surface as push failures, already covered). SNS topic emails `config.alarm_email`.

### 9.2a Account-wide budget

One AWS Budget at $5/mo, alerting at 80% actual and 100% forecasted spend, defined once in the account-wide `CiStack` (`infra/opennewsletter/ci_stack.py`) rather than per-env in `MonitoringStack`. The first two budgets on an account are free.

### 9.3 Custom metrics namespace

`OpenNewsletter/{env}` — emitted via EMF (embedded metric format) from Rust Lambdas using `metrics` + `metrics-exporter-emf`-style logging.

---

## 10. IAM least-privilege summary

The general rule: each Lambda has its own role, scoped only to the resources it touches. CDK's Python `iam.PolicyStatement` with explicit `actions` and `resources` lists. Avoid `*` except where unavoidable (CloudWatch Logs put-event must be scoped to the function's log group ARN).

DynamoDB IAM patterns:
- Read-only Lambdas (`lambda-newsletters`): `Query`, `GetItem`, `BatchGetItem` on table + GSIs.
- Write Lambdas: add `PutItem`, `UpdateItem`, `DeleteItem`, `TransactWriteItems`.
- Tick Lambdas need `Query` on GSI2 ([`data-model.md` §9](data-model.md#9-why-gsi2-status--time)). No `Scan` anywhere.

S3 IAM patterns (§5.2): each media-signing Lambda gets `s3:GetObject` on the processed bucket scoped to exactly the prefix(es) its routes return URLs for (`img/*`, `avatar/*`, or both — §6.2's table); each delete-capable Lambda additionally gets `s3:DeleteObject` on the specific originals/processed prefixes it deletes from. No Lambda gets a bucket-wide or account-wide S3 grant.

---

## 10.5 Dev-environment resource policy overrides

When `config.env == "dev"`, the following one-line overrides are applied so `cdk destroy` on a volatile stack succeeds without manual intervention. These are no-ops in `prod`. Full rationale in [`environments.md` §5](../development/environments.md#5-dev-wipeability-cdk-settings).

- **DynamoDB table** (`DataStack`) — `removal_policy=DESTROY`, `point_in_time_recovery=False`.
- **S3 buckets** (`MediaPersistentStack`) — `auto_delete_objects=True`, `removal_policy=DESTROY`. Without `auto_delete_objects`, destroy fails on non-empty buckets.
- **Secrets Manager** — nothing to override: no stack creates a secret. The app secret `opennewsletter/{env}` is created by hand ([`operator-setup.md` §7](../operations/operator-setup.md#7-combine-credentials-into-one-secrets-manager-secret-per-env)), and CDK only emits dynamic references to its ARN, so `cdk destroy` never touches it.
- **CloudWatch log groups** (every Lambda) — `removal_policy=DESTROY`. Otherwise destroy succeeds but leaves orphans, and the next deploy errors on "log group already exists."
- **Cognito user pool** (`AuthStack`) — unchanged. Persistent in dev. See [`environments.md` §3](../development/environments.md#3-stack-lifetime-tiers).

---

## 11. Deployment

The operator procedure — one-time account setup, bootstrapping, and ongoing deploys — is in [`runbook.md`](../operations/runbook.md). CI-driven deploys (what `deploy.yml` does on push to `main` and on prod promotion) are covered in [`testing-and-ci.md` §6](../development/testing-and-ci.md#6-github-actions-workflows).

Structurally: CDK Lambda assets reference compiled `bootstrap` binaries in `backend/target/lambda/<crate-name>/bootstrap`, produced by `cargo lambda build --release --arm64` (`make build-lambdas`). The CDK code reads these paths via `aws_lambda.Code.from_asset(...)` and silently falls back to a shell stub if a binary is missing, so `scripts/check_lambda_binaries.sh` guards against that — run by `make deploy-dev` and by `deploy.yml`. `cdk deploy --context env=<dev|prod> --all` deploys every stack for that environment.

---

## 12. Cost expectations

Target ≈ $1–2/mo at ~30 monthly active users, prod + dev in one account:

| Item | Monthly cost |
|---|---|
| Secrets Manager ($0.40/secret/mo; 1 per env × 2) | $0.80 |
| CloudWatch alarms ($0.10/alarm metric; 10 free; prod only, §9.2) | $0 |
| CloudWatch Logs / EMF metrics | < $0.10 |
| DynamoDB on-demand + storage (+ PITR prod) | < $0.30 |
| Lambda | $0 (always-free tier) |
| HTTP API ($1/M req after the 12-month free tier) | < $0.10 |
| S3 storage (a few GB; originals are deleted once processed, §5.3) | < $0.25 |
| S3 GET requests + egress (§5.2: $0.0004/1k GETs; egress free under AWS's 100 GB/mo always-free data transfer out, then ~$0.09/GB) | < $0.10 |
| Cognito (free tier) | $0 |
| EventBridge scheduled rules | $0 |
| **Total** | **≈ $1–2/mo** |

Domain registration and DNS stay with the registrar (Namecheap) rather than Route53, so there is no Route53 hosted zone; see `hosted_zone_id: None` (§2) and §8.3. Prices are approximate list prices (us-east-1).

This depends on: one Secrets Manager secret per env, `opennewsletter/{env}` (§2, §7.1); alarms in prod only, ≤10 alarm metrics (§9.2), with dev keeping its dashboard but no alarms; one account-wide AWS Budget at $5/mo in `CiStack`, not per-env (§9.2a); and DynamoDB on the AWS-owned key rather than the AWS-managed `aws/dynamodb` KMS key (§3.1). [`environments.md` §12](../development/environments.md#12-cost-expectations) covers the dev-only slice of this total (≈ $0.40–1/mo).

**At scale.** The table above is for ~30 users. Most of it stays near zero as groups grow: Lambda and DynamoDB scale with requests and remain cents to low dollars, and S3's always-free 100 GB/mo data-transfer-out tier covers image egress for a while, growing at ~$0.09/GB past that (the service worker's image cache means each device only fetches a given image once — [`media.md` §6](media.md#6-cache-and-tenant-isolation-summary)). One line still grows:
- **Cognito**: free up to 10,000 monthly active users, then billed per MAU at a rate set by the user pool's feature plan (Lite is the cheapest; check the pool's plan and current pricing before crossing 10k).

S3 originals don't accumulate: `lambda-image-process` deletes the original right after marking its row `ready` (best effort — §5.3), as it does on decode failure or oversize. The 7-day `uploads/` lifecycle expiry on both originals buckets (§5.1) is a safety net for whatever that delete missed — see [`decisions.md`](../decisions.md#originals-deleted-on-success).
