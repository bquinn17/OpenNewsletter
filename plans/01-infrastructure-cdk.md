# 01 — Infrastructure (AWS CDK, Python)

This document specifies every AWS resource the project provisions, organized into CDK stacks. A subagent should be able to translate this directly into Python CDK code.

Region: **us-east-1**. Account: single personal AWS account for both `dev` and `prod`, distinguished by stack suffix (`-dev`, `-prod`). See `00-overview.md` §7 for the rationale.

---

## 1. Stack inventory

| Stack | Purpose | Depends on |
|---|---|---|
| `DataStack` | DynamoDB table + GSIs | — |
| `AuthStack` | Cognito user pool, IdPs, hosted UI domain | — |
| `MediaPersistentStack` | S3 buckets (originals + processed), CloudFront distribution, KeyGroup | `DataStack` |
| `MediaPipelineStack` | `lambda-image-process` and its S3 event subscription | `DataStack`, `MediaPersistentStack` |
| `ApiStack` | HTTP API, request Lambdas, routes, JWT authorizer | `DataStack`, `AuthStack`, `MediaPersistentStack`, `NotificationsStack` |
| `NotificationsStack` | EventBridge schedules, tick Lambdas, VAPID secret | `DataStack` |
| `FrontendStack` | ACM cert (`us-east-1`) + Route53 records (if Route53) | — |
| `MonitoringStack` | CloudWatch dashboards + alarms | All others |
| `CiStack` | Account-wide, unsuffixed (one per account, not per env): GitHub OIDC provider + deploy/diff roles (`11-testing-ci-cd.md` §6), and the account's one AWS Budget (§9.2a) | — |

CDK app entry point: `infra/app.py`. Uses `--context env=<dev|prod>` to select environment-specific config from `infra/opennewsletter/config.py`.

**Correction (M5)**: this table originally listed `NotificationsStack` as depending on `ApiStack` (presumably for a future `lambda:InvokeFunction` grant onto `lambda-push`). That's backwards for what M5 actually needed: `ApiStack` wires the dev-only `POST /admin/dev/tick/cycle` route (§6, §11a) directly to `NotificationsStack`'s `cycle-tick` Lambda, which requires `ApiStack` to receive that Lambda as a constructor parameter — i.e. `ApiStack` depends on `NotificationsStack`, not the reverse. `app.py` instantiates `NotificationsStack` before `ApiStack` accordingly. In M11 it was `lambda-cycle-tick`, not `lambda-notify-tick`, that needed to invoke `lambda-push` (notify-tick sends reminders in-process, §7.3). The grant is on an ARN built with `Stack.format_arn` inside `NotificationsStack`, rather than on the live `ApiStack` construct, so no CDK-level cycle is created (§7.2).

---

## 2. `config.py`

```python
from dataclasses import dataclass

@dataclass(frozen=True)
class EnvConfig:
    env: str                          # "dev" | "prod"
    domain: str | None                # "opennewsletter.example.com" in prod; None in dev (raw AWS endpoints, 13-dev-environments.md §2)
    api_domain: str | None            # "api.opennewsletter.example.com" in prod; None in dev
    hosted_zone_id: str | None        # if Route53, else None (DNS managed externally)
    hosted_zone_name: str | None
    cognito_domain_prefix: str        # globally unique; e.g. "opennewsletter-prod"
    app_secret_arn: str               # ARN of the one per-env Secrets Manager secret (§7.1, §12); replaces the four *_secret_arn fields below (decided 2026-10-07; lands in M13.5)
    # No Apple IdP — dropped, see plans/PROGRESS.md decisions (M1).
    log_retention_days: int           # 30 in dev, 90 in prod
    alarm_email: str | None
    bootstrap_admin_email: str | None # prod only; target of the email-bound admin invite scripts/bootstrap_admin.py creates (05 §9; decided 2026-10-07; lands in M13.5)
```

Provider OAuth credentials, VAPID keys, and the CloudFront signing key are pre-populated in AWS Secrets Manager out-of-band (not in CDK), as JSON keys inside the one per-env secret `opennewsletter/{env}` referenced by `app_secret_arn` (§7.1, §12; decided 2026-10-07; lands in M13.5 — previously four separate secrets, one per credential pair). `config.py` only stores the ARN.

---

## 3. `DataStack`

### 3.1 DynamoDB table `OpenNewsletter-{env}`

- **Billing**: `PAY_PER_REQUEST`
- **Partition key**: `pk` (String)
- **Sort key**: `sk` (String)
- **Time-to-live attribute**: `ttl` (Number, epoch seconds) — used for invite expirations and ephemeral entities
- **Streams**: **disabled in v1**. The only planned consumer is the archival Lambda (`10-archival.md`), which is deferred. Re-enable as `NEW_AND_OLD_IMAGES` when archival lands.
- **Point-in-time recovery**: enabled in `prod`, disabled in `dev`
- **Removal policy**: `RETAIN` in `prod`, `DESTROY` in `dev`
- **Encryption**: AWS-owned key (the free default; decided 2026-10-07; lands in M13.5 — was the AWS-managed `aws/dynamodb` KMS key, which bills per-key)

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

See `02-data-model-dynamodb.md` for exact key compositions.

### 3.3 Outputs (for cross-stack reference)

- `table_name`
- `table_arn`

---

## 4. `AuthStack`

### 4.1 Cognito User Pool

- **Name**: `OpenNewsletter-{env}`
- **Sign-in aliases**: email
- **Self-signup**: **disabled** (signup goes through invite redemption — see `05-auth-flow.md`)
- **Password policy**: 12 chars min, require lowercase, uppercase, digit, symbol (used only for the bootstrap admin path; OAuth users don't see this)
- **MFA**: optional, TOTP only
- **Account recovery**: email
- **Standard attributes**: email (required), name (optional)
- **Custom attributes**: none. (The invite code survives the OAuth round-trip inside the OIDC `state` parameter managed by the SPA — see `05-auth-flow.md` §3 — so no Cognito-side storage is needed.)
- **Lambda triggers**:
  - `PreSignUp` Lambda (Rust, in `lambda-invites` crate) — **pure pass-through** that auto-confirms federated accounts (`auto_confirm_user = true`). It does NOT see, validate, or consume invite codes. Invite consumption happens after login via `POST /invites/redeem` (§6.5 below and `05-auth-flow.md` §4), which also lazily creates the `User` row on first redemption.
  - There is **no `PostConfirmation` trigger** (`05-auth-flow.md` §5).

### 4.2 Identity Providers (federated)

- **Google** — `UserPoolIdentityProviderGoogle`. Reads `google_client_id`/`google_client_secret` via CFN dynamic references against the app secret (`app_secret_arn`; decided 2026-10-07 — was a dedicated secret ARN pair), e.g. `{{resolve:secretsmanager:<app_secret_arn>:SecretString:google_client_id}}`.
- **Facebook** — `UserPoolIdentityProviderFacebook`. Same pattern against `facebook_app_id`/`facebook_app_secret`.

No Apple IdP — dropped, see `plans/PROGRESS.md` decisions (M1); Apple
Developer membership is $99/yr.

For both: attribute mapping `email -> email`, `name -> name`. Google additionally maps `email_verified -> email_verified` (decided 2026-10-07; lands in M13.5 — needed so `POST /invites/redeem` can check a bound invite's email-verified claim, `05` §4.2/§9). Facebook doesn't supply `email_verified`, so an email-bound invite can only be redeemed via Google.

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

**`admin-bootstrap`** — used only by `scripts/bootstrap_admin.py` and the dev-only `/admin/bootstrap-login` page (see `05-auth-flow.md` §9.2).
- **Generate secret**: NO (public client)
- **Auth flows**: `ALLOW_USER_PASSWORD_AUTH` + `ALLOW_REFRESH_TOKEN_AUTH`
- **Supported IdPs**: Cognito only (no federation)
- **Token validity**: same as `frontend`
- **Dev only** (decided 2026-10-07; lands in M13.5). Previously this client also existed in `prod` for the one-time bootstrap; prod bootstrap no longer creates a Cognito user at all — the operator redeems an email-bound admin invite via Google instead (`05` §9). So `admin-bootstrap` is created only when `env == dev`, it's the inner-loop sign-in path there so dev iteration doesn't bounce through Google/Facebook every time, and the `ApiStack` JWT authorizer's audience in `prod` is the `frontend` client alone (§6.1).

### 4.4 Hosted UI Domain

- Cognito-managed domain at `{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com`. Custom domain optional in `prod` (defer).

### 4.5 Outputs

- `user_pool_id`
- `user_pool_arn`
- `user_pool_client_id` (the `frontend` client)
- `user_pool_bootstrap_client_id` (the `admin-bootstrap` client; dev only, decided 2026-10-07 — absent in prod)
- `hosted_ui_domain`

---

## 5. `MediaPersistentStack` and `MediaPipelineStack`

The media resources are split across two stacks so that `dev` can recreate the image-processing pipeline freely without touching the slow-to-rebuild S3 + CloudFront infrastructure. See [`13-dev-environments.md` §3](13-dev-environments.md) for the rationale.

- **`MediaPersistentStack`** — §5.1, §5.2: S3 buckets, CloudFront distribution, KeyGroup. Long-lived in dev (CloudFront takes 15–30 min to delete).
- **`MediaPipelineStack`** — §5.3: `lambda-image-process` and its S3 event subscription. Volatile; recreated freely.

In `prod` the split is structural-only; both stacks deploy together and behave identically to a combined stack.

### 5.1 S3 buckets (`MediaPersistentStack`)

Two buckets, both private (Block Public Access fully on):

- **`opennewsletter-media-originals-{env}-{accountId}`**
  - Versioning: disabled in both envs (decided 2026-10-07; lands in M13.5 — was enabled in `prod`. Real delete (D8, `03` §9.4) only works cleanly without versioning: a delete on a versioned bucket just adds a delete marker and leaves the object recoverable, which defeats the point of a user-initiated permanent delete)
  - Lifecycle:
    - Abort incomplete multipart uploads after 1 day
    - Transition to `INTELLIGENT_TIERING` after 30 days
  - CORS: allow `PUT` and `POST` from `https://{config.domain}` and `http://localhost:5173` in dev. Allowed headers: `*`, because the presign decides which headers are signed (e.g. `x-amz-checksum-sha256`, `08` §3.2; decided M8). Expose: `ETag`.
  - Notification: `s3:ObjectCreated:*` for prefix `uploads/` → invokes `lambda-image-process`.
  - Encryption: SSE-S3.

- **`opennewsletter-media-processed-{env}-{accountId}`**
  - No versioning.
  - Lifecycle: same as originals.
  - Read access only via CloudFront origin access control (OAC).

Two more buckets for the **avatar pipeline** (`08-media-uploads.md` §11 — avatars cross group boundaries, so they don't share the media buckets' group-scoped key layout or signing):

- **`opennewsletter-avatars-originals-{env}-{accountId}`**
  - Same CORS, lifecycle, and encryption settings as media originals; no versioning.
  - Notification: `s3:ObjectCreated:*` for prefix `uploads/` → invokes the same `lambda-image-process` (wired in `MediaPipelineStack`).
- **`opennewsletter-avatars-processed-{env}-{accountId}`**
  - No versioning. Read access only via the CloudFront `/avatar/*` behavior (§5.2) — no key group.

### 5.2 CloudFront distribution (`MediaPersistentStack`)

- **Origins**:
  - `processed` bucket via OAC (default origin)
  - `originals` bucket via OAC (only used by signed-URL fetch in admin mode — defer)
- **Default behavior**:
  - Path pattern: `/img/*` → maps to `processed` bucket via origin path `/`
  - Allowed methods: `GET, HEAD`
  - Viewer protocol: redirect-to-HTTPS
  - Cache policy: `CachingOptimized` (managed)
  - **Trusted key groups**: a `KeyGroup` containing one CloudFront public key — **RSA 2048**, its own pair per env, read from `infra/keys/cf-signing.pub.pem` (dev) or `infra/keys/cf-signing-prod.pub.pem` (prod); private key in the env's app secret (`05` §10) → enables **signed cookies** for tenant isolation. See `08-media-uploads.md` §6. (In dev the same key group is used but the SPA presents the signature as signed-URL query params — `08-media-uploads.md` §4.5.)
- **Avatar behavior**: path pattern `/avatar/*` → `avatars-processed` bucket via OAC. `GET, HEAD`, `CachingOptimized`, **no trusted key group** — avatars are unsigned by design (`08-media-uploads.md` §11.3).
- **Price class**: `PriceClass_100` (NA + EU only) for cost.
- **Domain alias**: `cdn.{config.domain}` in prod. None in dev, which uses the raw `*.cloudfront.net` domain (`13-dev-environments.md` §2).
- **Certificate**: ACM cert in `us-east-1` (created in `FrontendStack`).

### 5.3 `lambda-image-process` (Rust, `MediaPipelineStack`)

- **Trigger**: S3 ObjectCreated, prefix `uploads/`, on **both** the media originals bucket and the avatars originals bucket. The handler branches on the source **bucket name** (not key prefix — both buckets use `uploads/`) to choose response-image vs avatar processing.
- **Memory**: 1024 MB
- **Timeout**: 60 s
- **Architecture**: arm64 (cheaper)
- **Environment**:
  - `TABLE_NAME`
  - `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET` (the handler compares the event's bucket against these)
  - `PROCESSED_BUCKET`
  - `AVATARS_PROCESSED_BUCKET`
- **IAM**:
  - `s3:GetObject` + `s3:DeleteObject` on both originals buckets (delete purges oversized uploads — `08-media-uploads.md` §3.2)
  - `s3:PutObject` on processed bucket (prefix `img/*`) and avatars-processed bucket (prefix `avatar/*`)
  - `dynamodb:GetItem` + `dynamodb:UpdateItem` on table (read the row's status, then mark it `ready`/`failed` and store dimensions)
- Logic detailed in `08-media-uploads.md` §5.

### 5.4 Outputs

- `originals_bucket_name`, `originals_bucket_arn`
- `processed_bucket_name`, `processed_bucket_arn`
- `avatars_originals_bucket_name`, `avatars_processed_bucket_name`
- `cloudfront_distribution_id`
- `cloudfront_domain` (the alias)
- `cloudfront_key_group_id`

The CloudFront signing private key no longer gets its own output — it's one JSON key (`cdn_signing_private_key`) inside the single per-env `app_secret_arn` secret (§7.1, §12; decided 2026-10-07; lands in M13.5), which `lambda-media` reads directly rather than via a `MediaPersistentStack` output.

---

## 6. `ApiStack`

### 6.1 HTTP API (`apigatewayv2`)

- **Name**: `OpenNewsletter-Api-{env}`
- **CORS**:
  - Allow origins: `https://{config.domain}`, `http://localhost:5173` in dev
  - Allow methods: `GET, POST, PUT, PATCH, DELETE, OPTIONS`
  - Allow headers: `Authorization, Content-Type, x-correlation-id`
  - Expose headers: `x-correlation-id`
  - Max age: 600
- **Custom domain**: `{api_domain}` with ACM cert from `FrontendStack`
- **Default authorizer**: `HttpJwtAuthorizer` against the Cognito user pool (issuer URL `https://cognito-idp.us-east-1.amazonaws.com/{user_pool_id}`). In `dev`, audience: `[frontend_client_id, bootstrap_client_id]` — the `admin-bootstrap` client is included so password-auth tokens from `/admin/bootstrap-login` and `scripts/bootstrap_admin.py` pass the same authorizer (`13-dev-environments.md` §6); added in M7, after it was found missing. In `prod`, audience: `[frontend_client_id]` only, since `admin-bootstrap` no longer exists there (decided 2026-10-07; lands in M13.5 — §4.3).
- **Throttling**: 50 rps burst, 25 rps steady on default stage
- **Access logs**: JSON to a CloudWatch log group `/aws/http-api/OpenNewsletter-{env}`, 30/90 day retention

### 6.2 Lambda functions (request handlers)

All Rust, runtime `provided.al2023`, architecture `arm64`, memory 256 MB (512 for `lambda-responses`), timeout 10 s, log retention per config. Built with `cargo lambda build --release --arm64`.

The route column below is a **summary only** — `03-api-contract.md` §13 is the authoritative route ⇄ Lambda ⇄ role table, and when they disagree, §13 wins. CDK code should be written from §13.

| Lambda | Route summary | DynamoDB | S3 | Other |
|---|---|---|---|---|
| `lambda-invites` | invite create / list / revoke / redeem; also compiles the `PreSignUp` trigger binary | RW | — | — |
| `lambda-groups` | `GET /config`, `GET /healthz`, `GET/PATCH /me`, group get/list/patch, member kick + role change | RW | `s3:DeleteObject` on avatar originals + processed (old-avatar cleanup on `PATCH /me`, D8); `cloudfront:CreateInvalidation` on the distribution (`/avatar/*`) | — |
| `lambda-newsletters` | newsletter list + detail | R | — | — |
| `lambda-questions` | candidate list/create + votes, `DELETE /admin/groups/{g}/candidate-questions/{q}` (admin moderation only) | RW | — | — |
| `lambda-responses` | my-response get/list/put (drafts, publish, poll votes) | RW | — | — |
| `lambda-engagement` | comments (CRUD) + reactions (put/delete/get) | RW | `s3:DeleteObject` on media originals + processed (comment-image release on soft-delete/replace, D8); `cloudfront:CreateInvalidation` on the distribution (`/img/*`) | — |
| `lambda-media` | uploads presign/status/caption/delete, `GET /media-cookie`, avatar presign/status/delete (`/avatars*`) | RW | PUT presign on media + avatar originals; CloudFront signing; `s3:DeleteObject` on media + avatar originals and processed (`DELETE /uploads`, `DELETE /avatars`, D8); `cloudfront:CreateInvalidation` on the distribution | Secrets:GetSecretValue on the app secret (signing key) |
| `lambda-push` | subscribe / unsubscribe / list / test / `PUT /push/preferences/{g}`; internal fan-out handlers direct-invoked by the tick Lambdas (`07-notifications.md` §6) | RW | — | Secrets:GetSecretValue on the app secret (VAPID keys) |

**Real media deletion (decided 2026-10-07; lands in M13.5, `03` §9.4/§9.7, D8).** A shared helper — the `media-delete` library crate (`backend/crates/media-delete`; not `shared`, which can't depend on `persistence` and so can't delete the row — decided M13.5) — deletes an image's S3 objects, invalidates its CloudFront path, then deletes the `ImageMedia`/`AvatarMedia` row. The three IAM grants above (`lambda-media`, `lambda-engagement`, `lambda-groups`) are scoped to the bucket prefixes and the one CloudFront distribution each Lambda actually deletes from; no Lambda gets a blanket `cloudfront:CreateInvalidation` across the whole account.

The dev-only fast-forward routes (`POST /admin/dev/tick/{cycle|notify}`, `03-api-contract.md` §11a) integrate the tick Lambdas from `NotificationsStack` into this API when `env == dev`.

The `PreSignUp` trigger binary is compiled from `lambda-invites` but its CDK `Function` is defined in **`AuthStack`**, not here. `user_pool.add_trigger()` attaches the wiring to the user pool's own stack, so building the function in `ApiStack` makes `AuthStack` depend on `ApiStack` while `ApiStack` already depends on `AuthStack` for the JWT authorizer — a cyclic reference that fails at synth. (Same failure mode as the `MediaPersistentStack`/`MediaPipelineStack` notification bug; the rule is: define a resource in the stack that owns the thing it attaches to.)

All Lambdas share:
- Env: `TABLE_NAME`, `RUST_LOG=info`, `ENV={env}`, `CONFIG_JSON=` (group-defaults JSON; see §6.4), `API_BASE_URL` (used to build the absolute RFC-7807 `type` URI)
- `lambda-groups` additionally gets `CDN_BASE_URL` (avatar URLs) and `VAPID_PUBLIC_KEY` (a Secrets Manager dynamic reference against the `vapid_public_key` JSON key of the app secret, echoed by `GET /config` per §7.1)
- `CDN_BASE_URL` goes to every Lambda that builds media URLs: `lambda-groups`, `-newsletters`, `-questions` and `-media`. It is `https://cdn.{domain}` in prod and `https://{distribution domain}.cloudfront.net` in dev, which has no custom domains (decided M8).
- `lambda-media` additionally gets `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET`, `CDN_KEY_PAIR_ID` (the CloudFront public key's ID), `APP_SECRET_ARN` (the signing key is the `cdn_signing_private_key` JSON key within it; decided 2026-10-07 — was `CDN_SIGNING_SECRET_ARN` against a dedicated secret) and `MEDIA_COOKIE_DOMAIN` (`.{domain}` in prod, empty in dev, where no `Set-Cookie` is sent — `08` §4.5)
- The HTTP API's CORS sets `allowCredentials: true`, because the SPA calls `GET /media-cookie` with `credentials: "include"`
- IAM (baseline): write access to its own log group only
- DynamoDB IAM: scoped to `arn:aws:dynamodb:...:table/OpenNewsletter-{env}` and the two GSIs

Each Lambda has its own CloudWatch log group with retention from config.

### 6.3 Routes and integrations

For each route in `03-api-contract.md`, CDK creates:
- An `HttpRoute` with the Cognito JWT authorizer attached — every route, including `POST /invites/redeem` (§6.5)
- An `HttpLambdaIntegration` pointing to the appropriate Lambda

### 6.4 Group-default config JSON

A static JSON document baked into Lambda env at deploy time, providing defaults for newly-created groups:

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

`responseWindowDays` default is 7 (decided 2026-10-07; lands in M13.5 — was 4; `DEFAULT_RESPONSE_WINDOW_DAYS` in `shared/src/config.rs`). Existing dev groups keep their stored `responseWindowDays=4` until patched or reseeded. Group settings stored in DynamoDB override these defaults per-group; see `02-data-model-dynamodb.md`.

### 6.5 No public routes

**All routes have JWT auth.** Invite redemption happens via `POST /invites/redeem`, which requires a valid Cognito JWT (see `05-auth-flow.md` §4 for the full flow). The handler performs both first-signup user creation and additional-group joins; there is no unauthenticated path into the API.

### 6.6 Outputs

- `api_id`
- `api_endpoint` (the custom domain URL)

---

## 7. `NotificationsStack`

### 7.1 VAPID secret

VAPID keys no longer get a dedicated secret (decided 2026-10-07; lands in M13.5). They're three JSON keys — `vapid_public_key`, `vapid_private_key`, `vapid_subject` — inside the single per-env Secrets Manager secret `opennewsletter/{env}` (§12), alongside `google_client_id`, `google_client_secret`, `facebook_app_id`, `facebook_app_secret`, and `cdn_signing_private_key`. Its ARN is `app_secret_arn` in `config.py` (§2).

- Generated out-of-band by `scripts/generate_vapid_keys.py` and merged into the one secret manually before first deploy.
- Public key exposed to the frontend via the `GET /config` endpoint (in `lambda-groups`).
- The four pre-M13.5 dev secrets (Google, Facebook, VAPID, CDN signing) are migrated into the one secret, then deleted.

### 7.2 `lambda-cycle-tick`

- **Trigger**: EventBridge Scheduler rule running **every 5 minutes**.
- **Memory**: 512 MB. **Timeout**: 60 s.
- **Job**: Open / lock / close / publish cycles whose timestamps have passed. Driven by GSI2. See `06-newsletter-lifecycle.md` §3.
- **IAM**: full RW on table; `lambda:InvokeFunction` on `lambda-push` (or call directly via async invoke).

**M11 status**: `lambda-cycle-tick` now async-invokes `lambda-push` (`InvocationType=Event`) for the cycle-open and publication fan-outs. It finds the function through env `PUSH_FUNCTION_NAME=OpenNewsletter-Push-{env}`, and its `lambda:InvokeFunction` grant is on an ARN built with `Stack.format_arn`, not on the `ApiStack` construct, which would create a stack cycle (§1 "Correction (M5)").

**M5 status**: the stack, schedule, and table RW grant above are built. The `lambda:InvokeFunction` grant on `lambda-push` isn't — that Lambda doesn't exist until M11, and cycle-tick has no push fan-out to do yet (§7.4's `lambda-notify-tick` owns that). No VAPID secret access is wired to `lambda-cycle-tick` either; only `lambda-notify-tick` needs it (§7.3).

### 7.3 `lambda-notify-tick`

- **Trigger**: EventBridge Scheduler rule running **every 15 minutes**.
- **Memory**: 512 MB. **Timeout**: 120 s.
- **Job**: Find cycles with deadlines within the next configured offset (96/48/24h) that haven't been notified yet, fan out Web Push messages.
- **IAM**: RW on table, `secretsmanager:GetSecretValue` on the app secret (`app_secret_arn`; decided 2026-10-07 — was a dedicated VAPID secret). (M11: no `lambda:InvokeFunction` — it links `lambda-push`'s library and sends reminders in-process; only `lambda-cycle-tick` invokes `lambda-push`.)
- See `07-notifications.md` for full logic.

### 7.4 Idempotency

Both tick Lambdas write a `TICK` sentinel record (`sk=CYCLE` / `sk=NOTIFY`, with `last_ran_at`). Notification dispatch idempotency uses per-cycle markers in the cycle's partition: `NOTIFIED#OPEN`, `NOTIFIED#CLOSE#{offset_hours}` and `NOTIFIED#PUBLISH` (`02-data-model-dynamodb.md` §2.15).

---

## 8. `FrontendStack`

The frontend itself ships to GitHub Pages — CDK doesn't host it. This stack manages domain plumbing only.

### 8.1 ACM certs

- One cert in `us-east-1` covering `{config.domain}`, `cdn.{config.domain}`, `{api_domain}`, with DNS validation.
- If `hosted_zone_id` is set, validation records auto-created via Route53. Else, output the validation CNAMEs and stop — operator finishes DNS manually.

### 8.2 Route53 (only if `hosted_zone_id` is set)

- A `CNAME` for `{config.domain}` pointing to `{githubuser}.github.io`. Note GitHub Pages also requires apex-A records — operator must add per GitHub docs if using apex.
- An `A`-alias for `{api_domain}` pointing to the HTTP API custom domain.
- An `A`-alias for `cdn.{config.domain}` pointing to the CloudFront distribution.

### 8.3 If DNS is external

Stack outputs the required records as `CfnOutput` so the operator can copy them.

---

## 9. `MonitoringStack`

### 9.1 Dashboard

A single CloudWatch dashboard `OpenNewsletter-{env}` with widgets for:
- HTTP API 4xx/5xx rates and p50/p95/p99 latency per route
- Per-Lambda invocation count, error rate, duration p95, throttles
- DynamoDB consumed RCU/WCU, throttles, conditional check failures
- CloudFront request count + 4xx/5xx
- Web Push delivery success/failure (custom metric emitted by `lambda-notify-tick` and `lambda-push`)

### 9.2 Alarms

**Prod only, ≤10 alarm metrics total** (decided 2026-10-07; lands in M13.5 — dev keeps the dashboard in §9.1 but ships no alarms, so dev stays inside the 10-free-alarm-metrics CloudWatch tier at $0):

- Lambda error rate across all functions, account-level aggregate `AWS/Lambda` `Errors`/`Invocations` (2 metrics) → SNS topic. Replaces a per-Lambda >5% alarm, which would have needed one metric pair per function.
- HTTP API 5xx rate >1% over 10 minutes (2 metrics) → SNS topic
- DynamoDB read + write throttle events >0 over 5 minutes (2 metrics) → SNS topic
- Push delivery failure rate >20% rolling (2 metrics) → SNS topic
- `lambda-cycle-tick` errors, any, plus the tick's `AutoPublishAbandoned` EMF metric (2 metrics, `FILL(errors,0) + FILL(abandoned,0) ≥ 1`) → SNS topic. The tick logs per-cycle failures rather than failing the invocation, so Lambda `Errors` alone missed auto-publish giving up on drafts (`06` §5.3; added 2026-10-08, review fix).

That's all 10 of the free alarm metrics, so a new alarm must replace or merge into one of these; `lambda-notify-tick` failures are not separately alarmed (its failures mostly surface as push failures, already covered). SNS topic emails `config.alarm_email`.

### 9.2a Account-wide budget

One AWS Budget at $5/mo, alerting at 80% actual and 100% forecasted spend, defined once in the account-wide `CiStack` (`infra/opennewsletter/ci_stack.py`) rather than per-env in `MonitoringStack` (decided 2026-10-07; lands in M13.5). The first two budgets on an account are free.

### 9.3 Custom metrics namespace

`OpenNewsletter/{env}` — emitted via EMF (embedded metric format) from Rust Lambdas using `metrics` + `metrics-exporter-emf`-style logging.

---

## 10. IAM least-privilege summary

The general rule: each Lambda has its own role, scoped only to the resources it touches. CDK's Python `iam.PolicyStatement` with explicit `actions` and `resources` lists. Avoid `*` except where unavoidable (CloudWatch Logs put-event must be scoped to the function's log group ARN).

DynamoDB IAM patterns:
- Read-only Lambdas (`lambda-newsletters`): `Query`, `GetItem`, `BatchGetItem` on table + GSIs.
- Write Lambdas: add `PutItem`, `UpdateItem`, `DeleteItem`, `TransactWriteItems`.
- Tick Lambdas need `Query` on GSI2 (`02-data-model-dynamodb.md` §9). No `Scan` anywhere.

---

## 10.5 Dev-environment resource policy overrides

When `config.env == "dev"`, the following one-line overrides are applied so `cdk destroy` on a volatile stack succeeds without manual intervention. These are no-ops in `prod`. Full rationale in [`13-dev-environments.md` §5](13-dev-environments.md).

- **DynamoDB table** (`DataStack`) — `removal_policy=DESTROY`, `point_in_time_recovery=False`.
- **S3 buckets** (`MediaPersistentStack`) — `auto_delete_objects=True`, `removal_policy=DESTROY`. Without `auto_delete_objects`, destroy fails on non-empty buckets.
- **Secrets Manager secrets** (every stack that creates one) — `removal_policy=DESTROY` and `recovery_window=Duration.days(0)`. The default 7-day soft-delete window blocks redeploy-within-a-week.
- **CloudWatch log groups** (every Lambda) — `removal_policy=DESTROY`. Otherwise destroy succeeds but leaves orphans, and the next deploy errors on "log group already exists."
- **Cognito user pool** (`AuthStack`) — unchanged. Persistent in dev. See [`13-dev-environments.md` §3](13-dev-environments.md).

---

## 11. Deployment

```bash
# One-time per env
cd infra
python -m venv .venv && . .venv/bin/activate   # or .venv/Scripts/activate on Windows
pip install -r requirements.txt
cdk bootstrap aws://{accountId}/us-east-1

# Build Lambdas (separate step, fed via cdk asset bundling — see deploy.yml)
cd ../backend
cargo lambda build --release --arm64

# Deploy
cd ../infra
cdk deploy --context env=dev --all
```

CDK Lambda assets reference compiled `bootstrap` binaries in `backend/target/lambda/<crate-name>/bootstrap`. The CDK code reads these paths via `aws_lambda.Code.from_asset(...)`.

---

## 12. Cost expectations (revised 2026-10-07; lands in M13.5)

Target ≈ $1–2/mo at ~30 monthly active users, prod + dev in one account:

| Item | As built (M13) | After M13.5 |
|---|---|---|
| Secrets Manager ($0.40/secret/mo) | 4 per env × 2 = $3.20 | 1 per env × 2 = $0.80 |
| CloudWatch alarms ($0.10/alarm metric; 10 free) | 17 alarms/env, ~30 alarm metrics/env incl. metric math ≈ $5 | prod only, ≤10 alarm metrics = $0 |
| CloudWatch Logs / EMF metrics | < $0.10 | < $0.10 |
| DynamoDB on-demand + storage (+ PITR prod) | < $0.30 | < $0.30 |
| Lambda | $0 (always-free tier) | $0 |
| HTTP API ($1/M req after the 12-month free tier) | < $0.10 | < $0.10 |
| S3 (a few GB) | < $0.25 | < $0.25 |
| CloudFront (always-free 1 TB + 10M req) | $0 | $0 |
| Cognito (free tier) | $0 | $0 |
| EventBridge scheduled rules | $0 | $0 |
| **Total** | **≈ $8–9/mo** | **≈ $1–2/mo** |

Domain registration (Namecheap) is outside AWS; DNS stays on Namecheap (`PROGRESS.md` B5), so there is no Route53 hosted zone. Prices are approximate list prices (us-east-1).

Changes driving the drop (D7):
- **One Secrets Manager secret per env**, `opennewsletter/{env}` (§2, §7.1), collapsing the four dev secrets (Google, Facebook, VAPID, CDN signing) into one JSON object.
- **Alarms in prod only, ≤10 alarm metrics** (§9.2); dev keeps its dashboard but no alarms.
- **One account-wide AWS Budget at $5/mo** in `CiStack`, not per-env (§9.2a).
- **DynamoDB on the AWS-owned key** instead of the AWS-managed `aws/dynamodb` KMS key (§3.1).
- `13-dev-environments.md` §12's dev-only estimate becomes ≈ $0.40–1/mo.
