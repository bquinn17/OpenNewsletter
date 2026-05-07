# 01 — Infrastructure (AWS CDK, Python)

This document specifies every AWS resource the project provisions, organized into CDK stacks. A subagent should be able to translate this directly into Python CDK code.

Region: **us-east-1**. Account: single account for both `dev` and `prod`, distinguished by stack suffix (`-dev`, `-prod`).

---

## 1. Stack inventory

| Stack | Purpose | Depends on |
|---|---|---|
| `DataStack` | DynamoDB table + GSIs | — |
| `AuthStack` | Cognito user pool, IdPs, hosted UI domain | — |
| `MediaStack` | S3 buckets, CloudFront distribution, image-process Lambda | `DataStack` |
| `ApiStack` | HTTP API, request Lambdas, routes, JWT authorizer | `DataStack`, `AuthStack`, `MediaStack` |
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
    domain: str                       # "opennewsletter.example.com" or "dev.opennewsletter.example.com"
    api_domain: str                   # "api.opennewsletter.example.com" or "api-dev..."
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
- **Streams**: `NEW_AND_OLD_IMAGES` enabled (consumed by archival Lambda — see `10-archival.md`)
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
- `table_stream_arn`

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
- **Custom attributes**:
  - `pendingInvite` (mutable string, max 64) — set during the OAuth callback before the user redeems
- **Lambda triggers**:
  - `PreSignUp` Lambda (Rust, in `lambda-invites` crate) — validates invite code from `clientMetadata.invite`, marks the code consumed-on-success in DynamoDB. Rejects signup if invite is missing/invalid/expired/already-used.
  - `PostConfirmation` Lambda (Rust, same crate) — creates the User entity and the GroupMembership entity in DynamoDB.

### 4.2 Identity Providers (federated)

- **Google** — `UserPoolIdentityProviderGoogle`. Reads client ID/secret from Secrets Manager ARNs in config.
- **Apple** — `UserPoolIdentityProviderApple`. Reads team ID, key ID, private key.
- **Facebook** — `UserPoolIdentityProviderFacebook`.

For all three: attribute mapping `email -> email`, `name -> name`.

### 4.3 App Client

- **Name**: `frontend`
- **Generate secret**: NO (public client)
- **OAuth flows**: Authorization Code with PKCE
- **OAuth scopes**: `openid`, `email`, `profile`
- **Callback URLs**: `https://{config.domain}/auth/callback`, plus `http://localhost:5173/auth/callback` in `dev`
- **Logout URLs**: `https://{config.domain}/`, plus `http://localhost:5173/` in `dev`
- **Supported IdPs**: Google, Apple, Facebook (Cognito itself disabled for end users — bootstrap admin uses a separate "admin" client)
- **Token validity**: ID 60min, Access 60min, Refresh 30 days

### 4.4 Hosted UI Domain

- Cognito-managed domain at `{cognito_domain_prefix}.auth.us-east-1.amazoncognito.com`. Custom domain optional in `prod` (defer).

### 4.5 Outputs

- `user_pool_id`
- `user_pool_arn`
- `user_pool_client_id`
- `hosted_ui_domain`

---

## 5. `MediaStack`

### 5.1 S3 buckets

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

### 5.2 CloudFront distribution

- **Origins**:
  - `processed` bucket via OAC (default origin)
  - `originals` bucket via OAC (only used by signed-URL fetch in admin mode — defer)
- **Default behavior**:
  - Path pattern: `/img/*` → maps to `processed` bucket via origin path `/`
  - Allowed methods: `GET, HEAD`
  - Viewer protocol: redirect-to-HTTPS
  - Cache policy: `CachingOptimized` (managed)
  - **Trusted key groups**: a `KeyGroup` containing one CloudFront public key (private key in Secrets Manager) → enables **signed cookies** for tenant isolation. See `08-media-uploads.md` §6.
- **Price class**: `PriceClass_100` (NA + EU only) for cost.
- **Domain alias**: `cdn.opennewsletter.example.com` (or `cdn-dev...`).
- **Certificate**: ACM cert in `us-east-1` (created in `FrontendStack`).

### 5.3 `lambda-image-process` (Rust)

- **Trigger**: S3 ObjectCreated on originals bucket, prefix `uploads/`
- **Memory**: 1024 MB
- **Timeout**: 60 s
- **Architecture**: arm64 (cheaper)
- **Environment**:
  - `TABLE_NAME`
  - `PROCESSED_BUCKET`
- **IAM**:
  - `s3:GetObject` on originals bucket
  - `s3:PutObject` on processed bucket (prefix `img/{groupId}/{yyyymm}/{questionId}/{userId}/{imgId}/*`)
  - `dynamodb:UpdateItem` on table (for marking image record `READY` and storing dimensions)
- Logic detailed in `08-media-uploads.md` §5.

### 5.4 Outputs

- `originals_bucket_name`, `originals_bucket_arn`
- `processed_bucket_name`, `processed_bucket_arn`
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

| Lambda | Routes (see `03-api-contract.md`) | DynamoDB | S3 | Other |
|---|---|---|---|---|
| `lambda-invites` | `POST /admin/invites`, `POST /invites/redeem` | RW | — | — |
| `lambda-groups` | `GET /me`, `GET /groups`, `GET /groups/{g}`, `PATCH /groups/{g}` (admin) | RW | — | — |
| `lambda-newsletters` | `GET /groups/{g}/newsletters`, `GET /groups/{g}/newsletters/{nl}` | R | — | — |
| `lambda-questions` | `GET/POST /groups/{g}/candidate-questions`, `POST/DELETE .../{q}/votes`, admin curate routes | RW | — | — |
| `lambda-responses` | response CRUD + autosave + publish + poll vote | RW | — | — |
| `lambda-engagement` | comments + reactions | RW | — | — |
| `lambda-media` | `POST /uploads`, `POST /uploads/{id}/complete`, `GET /media-cookie` | RW | PUT presign on originals; CloudFront cookie sign | Secrets:GetSecretValue on signing key |
| `lambda-push` | subscribe, unsubscribe, list, test | RW | — | Secrets:GetSecretValue on VAPID |

All Lambdas share:
- Env: `TABLE_NAME`, `RUST_LOG=info`, `ENV={env}`, `CONFIG_JSON=` (group-defaults JSON; see §6.4)
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

### 6.5 Public route exception

`POST /invites/redeem` is NOT protected by the JWT authorizer — it accepts an unauthenticated Cognito access token from the OAuth callback flow as part of its body validation. See `05-auth-flow.md` §4.

Actually — given Cognito triggers (`PreSignUp` + `PostConfirmation`) handle invite consumption end-to-end, `POST /invites/redeem` only exists as a fallback for **adding an existing user to an additional group**. It DOES require auth. So **all routes have JWT auth**.

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
