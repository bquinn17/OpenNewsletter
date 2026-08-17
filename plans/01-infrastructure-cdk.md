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
| `ApiStack` | HTTP API, request Lambdas, routes, JWT authorizer | `DataStack`, `AuthStack`, `MediaPersistentStack` |
| `NotificationsStack` | EventBridge schedules, tick Lambdas, VAPID secret | `DataStack`, `ApiStack` |
| `FrontendStack` | ACM cert (`us-east-1`) + Route53 records (if Route53) | — |
| `MonitoringStack` | CloudWatch dashboards + alarms | All others |

CDK app entry point: `infra/app.py`. Uses `--context env=<dev|prod>` to select environment-specific config from `infra/opennewsletter/config.py`.

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
    google_client_id_secret_arn: str
    google_client_secret_secret_arn: str
    apple_client_id: str
    apple_team_id: str
    apple_key_id: str
    apple_private_key_secret_arn: str
    facebook_app_id_secret_arn: str
    facebook_app_secret_secret_arn: str
    log_retention_days: int           # 30 in dev, 90 in prod
    alarm_email: str | None
```

Provider OAuth credentials are pre-populated in AWS Secrets Manager out-of-band (not in CDK). `config.py` only stores ARNs.

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
- **Encryption**: AWS-managed KMS

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

- **Google** — `UserPoolIdentityProviderGoogle`. Reads client ID/secret from Secrets Manager ARNs in config.
- **Apple** — `UserPoolIdentityProviderApple`. Reads team ID, key ID, private key.
- **Facebook** — `UserPoolIdentityProviderFacebook`.

For all three: attribute mapping `email -> email`, `name -> name`.

### 4.3 App Clients

Two app clients on the same user pool:

**`frontend`** — the public client every end user goes through.
- **Generate secret**: NO (public client)
- **OAuth flows**: Authorization Code with PKCE
- **OAuth scopes**: `openid`, `email`, `profile`
- **Callback URLs**: `https://{config.domain}/auth/callback`, plus `http://localhost:5173/auth/callback` in `dev`
- **Logout URLs**: `https://{config.domain}/`, plus `http://localhost:5173/` in `dev`
- **Supported IdPs**: Google, Apple, Facebook (Cognito itself disabled for end users)
- **Token validity**: ID 60min, Access 60min, Refresh 30 days

**`admin-bootstrap`** — used only by `scripts/bootstrap_admin.py` and the dev-only `/admin/bootstrap-login` page (see `05-auth-flow.md` §9.2).
- **Generate secret**: NO (public client)
- **Auth flows**: `ALLOW_USER_PASSWORD_AUTH` + `ALLOW_REFRESH_TOKEN_AUTH`
- **Supported IdPs**: Cognito only (no federation)
- **Token validity**: same as `frontend`
- In `prod` this client exists but is exercised once at first-admin bootstrap and then dormant; in `dev` it's the inner-loop sign-in path so we don't bounce through Google/Apple/Facebook for every iteration.

### 4.4 Hosted UI Domain

- Cognito-managed domain at `{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com`. Custom domain optional in `prod` (defer).

### 4.5 Outputs

- `user_pool_id`
- `user_pool_arn`
- `user_pool_client_id` (the `frontend` client)
- `user_pool_bootstrap_client_id` (the `admin-bootstrap` client)
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
  - Versioning: enabled in `prod`, disabled in `dev`
  - Lifecycle:
    - Abort incomplete multipart uploads after 1 day
    - Transition to `INTELLIGENT_TIERING` after 30 days
  - CORS: allow `PUT` and `POST` from `https://{config.domain}` and `http://localhost:5173` in dev. Headers: `Content-Type`, `x-amz-content-sha256`, `x-amz-date`, `Authorization`. Expose: `ETag`.
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
  - **Trusted key groups**: a `KeyGroup` containing one CloudFront public key (private key in Secrets Manager) → enables **signed cookies** for tenant isolation. See `08-media-uploads.md` §6. (In dev the same key group is used but the SPA presents the signature as signed-URL query params — `08-media-uploads.md` §4.5.)
- **Avatar behavior**: path pattern `/avatar/*` → `avatars-processed` bucket via OAC. `GET, HEAD`, `CachingOptimized`, **no trusted key group** — avatars are unsigned by design (`08-media-uploads.md` §11.3).
- **Price class**: `PriceClass_100` (NA + EU only) for cost.
- **Domain alias**: `cdn.opennewsletter.example.com` (or `cdn-dev...`).
- **Certificate**: ACM cert in `us-east-1` (created in `FrontendStack`).

### 5.3 `lambda-image-process` (Rust, `MediaPipelineStack`)

- **Trigger**: S3 ObjectCreated, prefix `uploads/`, on **both** the media originals bucket and the avatars originals bucket. The handler branches on the source **bucket name** (not key prefix — both buckets use `uploads/`) to choose response-image vs avatar processing.
- **Memory**: 1024 MB
- **Timeout**: 60 s
- **Architecture**: arm64 (cheaper)
- **Environment**:
  - `TABLE_NAME`
  - `PROCESSED_BUCKET`
  - `AVATARS_PROCESSED_BUCKET`
- **IAM**:
  - `s3:GetObject` + `s3:DeleteObject` on both originals buckets (delete purges oversized uploads — `08-media-uploads.md` §3.2)
  - `s3:PutObject` on processed bucket (prefix `img/*`) and avatars-processed bucket (prefix `avatar/*`)
  - `dynamodb:UpdateItem` on table (for marking image record `READY` and storing dimensions)
- Logic detailed in `08-media-uploads.md` §5.

### 5.4 Outputs

- `originals_bucket_name`, `originals_bucket_arn`
- `processed_bucket_name`, `processed_bucket_arn`
- `avatars_originals_bucket_name`, `avatars_processed_bucket_name`
- `cloudfront_distribution_id`
- `cloudfront_domain` (the alias)
- `cloudfront_key_group_id`
- `cloudfront_signing_key_secret_arn`

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
- **Default authorizer**: `HttpJwtAuthorizer` against the Cognito user pool (issuer URL `https://cognito-idp.us-east-1.amazonaws.com/{user_pool_id}`, audience: `[user_pool_client_id]`)
- **Throttling**: 50 rps burst, 25 rps steady on default stage
- **Access logs**: JSON to a CloudWatch log group `/aws/http-api/OpenNewsletter-{env}`, 30/90 day retention

### 6.2 Lambda functions (request handlers)

All Rust, runtime `provided.al2023`, architecture `arm64`, memory 256 MB (512 for `lambda-responses`), timeout 10 s, log retention per config. Built with `cargo lambda build --release --arm64`.

The route column below is a **summary only** — `03-api-contract.md` §13 is the authoritative route ⇄ Lambda ⇄ role table, and when they disagree, §13 wins. CDK code should be written from §13.

| Lambda | Route summary | DynamoDB | S3 | Other |
|---|---|---|---|---|
| `lambda-invites` | invite create / list / revoke / redeem; also compiles the `PreSignUp` trigger binary | RW | — | — |
| `lambda-groups` | `GET /config`, `GET /healthz`, `GET/PATCH /me`, group get/list/patch, member kick + role change | RW | — | — |
| `lambda-newsletters` | newsletter list + detail | R | — | — |
| `lambda-questions` | candidate list/create + votes, `DELETE /admin/groups/{g}/candidate-questions/{q}` (admin moderation only) | RW | — | — |
| `lambda-responses` | my-response get/list/put (drafts, publish, poll votes) | RW | — | — |
| `lambda-engagement` | comments (CRUD) + reactions (put/delete/get) | RW | — | — |
| `lambda-media` | uploads presign/status/caption/delete, `GET /media-cookie`, avatar presign/status/delete (`/avatars*`) | RW | PUT presign on media + avatar originals; CloudFront signing | Secrets:GetSecretValue on signing key |
| `lambda-push` | subscribe / unsubscribe / list / test / `PUT /push/preferences/{g}`; internal fan-out handlers direct-invoked by the tick Lambdas (`07-notifications.md` §6) | RW | — | Secrets:GetSecretValue on VAPID |

The dev-only fast-forward routes (`POST /admin/dev/tick/{cycle|notify}`, `03-api-contract.md` §11a) integrate the tick Lambdas from `NotificationsStack` into this API when `env == dev`.

The `PreSignUp` trigger binary is compiled from `lambda-invites` but its CDK `Function` is defined in **`AuthStack`**, not here. `user_pool.add_trigger()` attaches the wiring to the user pool's own stack, so building the function in `ApiStack` makes `AuthStack` depend on `ApiStack` while `ApiStack` already depends on `AuthStack` for the JWT authorizer — a cyclic reference that fails at synth. (Same failure mode as the `MediaPersistentStack`/`MediaPipelineStack` notification bug; the rule is: define a resource in the stack that owns the thing it attaches to.)

All Lambdas share:
- Env: `TABLE_NAME`, `RUST_LOG=info`, `ENV={env}`, `CONFIG_JSON=` (group-defaults JSON; see §6.4), `API_BASE_URL` (used to build the absolute RFC-7807 `type` URI)
- `lambda-groups` additionally gets `CDN_BASE_URL` (avatar URLs) and `VAPID_PUBLIC_KEY` (a Secrets Manager dynamic reference, echoed by `GET /config` per §7.1)
- IAM (baseline): write access to its own log group only
- DynamoDB IAM: scoped to `arn:aws:dynamodb:...:table/OpenNewsletter-{env}` and the two GSIs

Each Lambda has its own CloudWatch log group with retention from config.

### 6.3 Routes and integrations

For each route in `03-api-contract.md`, CDK creates:
- An `HttpRoute` with the Cognito JWT authorizer attached (except `POST /invites/redeem` — see §6.5)
- An `HttpLambdaIntegration` pointing to the appropriate Lambda

### 6.4 Group-default config JSON

A static JSON document baked into Lambda env at deploy time, providing defaults for newly-created groups:

```json
{
  "questionsPerCycle": 5,
  "votesPerUserPerCycle": 3,
  "responseWindowDays": 4,
  "timezone": "America/New_York",
  "memberSoftCap": 50,
  "imagesPerAnswerMax": 10,
  "imageMaxBytes": 15728640,
  "inviteTtlDays": 7,
  "autosaveDebounceMs": 1500,
  "notificationOffsetsHoursBeforeClose": [96, 48, 24]
}
```

Group settings stored in DynamoDB override these defaults per-group; see `02-data-model-dynamodb.md`.

### 6.5 No public routes

**All routes have JWT auth.** Invite redemption happens via `POST /invites/redeem`, which requires a valid Cognito JWT (see `05-auth-flow.md` §4 for the full flow). The handler performs both first-signup user creation and additional-group joins; there is no unauthenticated path into the API.

### 6.6 Outputs

- `api_id`
- `api_endpoint` (the custom domain URL)

---

## 7. `NotificationsStack`

### 7.1 VAPID secret

- AWS Secrets Manager secret `opennewsletter/vapid/{env}` containing JSON `{ "publicKey": "...", "privateKey": "...", "subject": "mailto:admin@opennewsletter.example.com" }`.
- Generated out-of-band by `scripts/generate_vapid_keys.py` and stored manually before first deploy.
- Public key exposed to the frontend via the `GET /config` endpoint (in `lambda-groups`).

### 7.2 `lambda-cycle-tick`

- **Trigger**: EventBridge Scheduler rule running **every 5 minutes**.
- **Memory**: 512 MB. **Timeout**: 60 s.
- **Job**: Open / lock / close / publish cycles whose timestamps have passed. Driven by GSI2. See `06-newsletter-lifecycle.md` §3.
- **IAM**: full RW on table; `lambda:InvokeFunction` on `lambda-push` (or call directly via async invoke).

### 7.3 `lambda-notify-tick`

- **Trigger**: EventBridge Scheduler rule running **every 15 minutes**.
- **Memory**: 512 MB. **Timeout**: 120 s.
- **Job**: Find cycles with deadlines within the next configured offset (96/48/24h) that haven't been notified yet, fan out Web Push messages.
- **IAM**: RW on table, `secretsmanager:GetSecretValue` on VAPID secret, `lambda:InvokeFunction` on `lambda-push`.
- See `07-notifications.md` for full logic.

### 7.4 Idempotency

Both tick Lambdas write a `LastTickedAt` sentinel record so re-runs after failure don't double-fire. Notification dispatch idempotency uses a per-cycle, per-offset marker `NL#{nl}#NOTIFIED#{offsetHours}`.

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

- Any Lambda error rate >5% over 10 minutes → SNS topic
- HTTP API 5xx >1% over 10 minutes → SNS topic
- DynamoDB throttles >0 over 5 minutes → SNS topic
- `lambda-cycle-tick` or `lambda-notify-tick` failure (any) → SNS topic
- Push delivery failure rate >20% (rolling) → SNS topic

SNS topic emails `config.alarm_email`.

### 9.3 Custom metrics namespace

`OpenNewsletter/{env}` — emitted via EMF (embedded metric format) from Rust Lambdas using `metrics` + `metrics-exporter-emf`-style logging.

---

## 10. IAM least-privilege summary

The general rule: each Lambda has its own role, scoped only to the resources it touches. CDK's Python `iam.PolicyStatement` with explicit `actions` and `resources` lists. Avoid `*` except where unavoidable (CloudWatch Logs put-event must be scoped to the function's log group ARN).

DynamoDB IAM patterns:
- Read-only Lambdas (`lambda-newsletters`): `Query`, `GetItem`, `BatchGetItem` on table + GSIs.
- Write Lambdas: add `PutItem`, `UpdateItem`, `DeleteItem`, `TransactWriteItems`.
- Tick Lambdas need `Scan` on GSI2 (bounded by time-bucket prefix — see `02-data-model-dynamodb.md` §10).

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

# Build Lambdas (separate step, fed via cdk asset bundling — see backend-deploy.yml)
cd ../backend
cargo lambda build --release --arm64

# Deploy
cd ../infra
cdk deploy --context env=dev --all
```

CDK Lambda assets reference compiled `bootstrap` binaries in `backend/target/lambda/<crate-name>/bootstrap`. The CDK code reads these paths via `aws_lambda.Code.from_asset(...)`.

---

## 12. Cost expectations (rough, prod, ≤200 active users)

- DynamoDB on-demand: ~$1–3/mo
- Lambda + HTTP API: ~$1–2/mo (idle backend)
- S3 storage + requests: ~$0.50–2/mo (depends on image volume)
- CloudFront egress: ~$0.085/GB cache miss (PriceClass_100, NA + EU); cache hits dominate after the first viewer per published newsletter. Fixed CloudFront cost is **$0** — the distribution itself is free; only egress and requests are billed. See [`08-media-uploads.md` §6.1](08-media-uploads.md) for why CloudFront is retained vs. direct-from-S3.
- Cognito: free tier (50k MAU)
- EventBridge schedules: <$1/mo
- Secrets Manager: ~$0.40/secret/mo × ~5 secrets = ~$2/mo
- Route53: $0.50/zone if used

**Estimate**: under $15/mo all-in at the target scale. Most of this is fixed (Secrets Manager + Route53). Variable costs scale with image traffic.
