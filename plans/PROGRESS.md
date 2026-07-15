# Progress & Blockers

Snapshot as of 2026-07-14. Working document — update as milestones complete or
blockers resolve. Authoritative milestone definitions live in
[`12-build-order.md`](12-build-order.md).

---

## Milestone status

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Repo skeleton | ✅ done | Layout, READMEs, `.gitignore`, `rust-toolchain.toml`, etc. landed in prior commits. |
| M1 | Operator prerequisites | 🟡 partial | `scripts/generate_vapid_keys.py`, `infra/keys/`, `cf-signing.key` present as untracked files. Google/Apple/Facebook OAuth apps, Cognito domain, and DNS records still owed by the operator. |
| **M2** | **Backend foundations (domain + persistence)** | 🟡 **code-complete, unverified** | See "M2 detail" below. |
| **M3** | **Infrastructure baseline (CDK)** | 🟡 **synth + tests verified, deploy unverified** | See "M3 detail" below. |
| **M3.5** | **Dev environment online** | 🟡 **code-complete, unverified** | See "M3.5 detail" below. |
| M4 | Auth + bootstrap | ⬜ | |
| M5 | Lifecycle engine + cycle CRUD | ⬜ | |
| M6 | Responses + drafts | ⬜ | |
| M7 | Frontend skeleton + auth | ⬜ (existing mock UI predates real API) | Commit `91188ba` shipped a rich mock-only UI; will need rework against real endpoints in M7. |
| M8 | Media pipeline | ⬜ | |
| M9 | Newsletter UI | ⬜ | |
| M10 | Engagement | ⬜ | |
| M11 | Notifications | ⬜ | |
| M12 | Admin UI | ⬜ | |
| M13 | Hardening | ⬜ | |
| M14 | Production deploy | ⬜ | |

Roughly 2 of 15 milestones complete (~13%); M3's `cdk synth`/`pytest infra/tests/` are now verified (a real cyclic-stack-dependency bug was found and fixed — see M3 detail), but the actual `cdk deploy` gate in `12-build-order.md` still requires an AWS account and is unverified. M3.5 is code-complete and awaiting the same deploy step.

---

## M2 detail — what landed

All paths relative to `backend/`.

- **Workspace** — `Cargo.toml` wires all 14 crates with pinned workspace dependencies.
- **Lambda stubs** — every `crates/lambda-*` has a minimal `Cargo.toml` + `src/main.rs` so the workspace builds. Real handlers land in M4+.
- **`shared` crate** — [`config.rs`](../backend/crates/shared/src/config.rs): every tunable named in the plans (cycle sizes, vote caps, image/comment limits, invite TTL, vote-count pad width) in one file.
- **`domain` crate**
  - [`ids.rs`](../backend/crates/domain/src/ids.rs) — `UserId`, `GroupId`, `CycleId`, `QuestionId`, `ResponseId`, `CommentId`, `ImageId`, `AvatarId`, `PollOptionId`, `SubscriptionId`, `CognitoSub`, `InviteCode` as newtypes. Implement `Display`, `FromStr`, `AsRef<str>`, serde transparent. `UserId::generate()` etc. mint UUIDv7.
  - [`entities.rs`](../backend/crates/domain/src/entities.rs) — every entity from `02-data-model-dynamodb.md` §2: `User`, `CognitoSubLookup`, `GroupMembership`, `Group` (+ `CycleSettings`, `NotificationSettings`), `Invite`, `Newsletter`, `CandidateQuestion`, `CandidateVote`, `LockedQuestion`, `PollOption`, `Response`, `ImageMedia`, `AvatarMedia`, `Comment`, `Reaction`, `PushSubscription`, `NotificationPref`. Enums: `Role`, `NewsletterStatus`, `ResponseStatus`, `InviteStatus`, `MediaStatus`, `QuestionKind`, `ImageMimeType`.
  - [`error.rs`](../backend/crates/domain/src/error.rs) — `ApiError` + `ApiErrorCode` with HTTP-status mapping mirroring `03-api-contract.md` §1.1.
- **`persistence` crate**
  - [`keys.rs`](../backend/crates/persistence/src/keys.rs) — single source of truth for every `pk` / `sk` / `gsi1pk` / `gsi1sk` / `gsi2pk` / `gsi2sk` shape. Centralised `attr::*` and `index::*` constants. **Comprehensive unit tests** covering padding, lexicographic ordering, and every entity family.
  - [`repo.rs`](../backend/crates/persistence/src/repo.rs) — `Repo { client, table }` handle.
  - [`error.rs`](../backend/crates/persistence/src/error.rs) — `RepoError` enum, with `From` impls for `aws_sdk_dynamodb::error::SdkError` and `serde_dynamo::Error`.
  - Per-entity-family modules with the access-pattern functions named after the AP they serve:
    - [`users.rs`](../backend/crates/persistence/src/users.rs) — AP1, Cognito-sub lookup, profile upsert
    - [`groups.rs`](../backend/crates/persistence/src/groups.rs) — AP2, AP3, AP4 + transaction §4 #6 (leave group)
    - [`invites.rs`](../backend/crates/persistence/src/invites.rs) — AP5, AP6, revoke + transaction §4 #5 (join via invite)
    - [`newsletters.rs`](../backend/crates/persistence/src/newsletters.rs) — AP7, AP8, AP9 + `write_status_transition` (the GSI2-consistent writer required by §9)
    - [`questions.rs`](../backend/crates/persistence/src/questions.rs) — AP10, AP11, AP12, AP13 + transactions §4 #1 (cast vote), #2 (withdraw vote), #3 (promote candidates)
    - [`responses.rs`](../backend/crates/persistence/src/responses.rs) — AP14, AP15, AP16, last-write-wins `save_draft`, transaction §4 #4 (publish response with first-publish editions_answered bump)
    - [`engagement.rs`](../backend/crates/persistence/src/engagement.rs) — AP18, AP19, comment soft-delete, reaction toggle
    - [`media.rs`](../backend/crates/persistence/src/media.rs) — AP17, image + avatar read/write
    - [`push.rs`](../backend/crates/persistence/src/push.rs) — AP20, AP21, subscribe/unsubscribe
  - [`test_factories.rs`](../backend/crates/persistence/src/test_factories.rs) behind the `test-utils` feature.

### M2 coverage vs. plan

- ✅ Every access pattern AP1–AP21 has a named function.
- ✅ Every transaction in `02-data-model-dynamodb.md` §4 (#1–#6) implemented with `TransactWriteItems`.
- ✅ `keys.rs` unit tests in place (15 tests).
- ✅ **Integration tests written and passing** — 49 tests across 9 files in `backend/crates/persistence/tests/`, run against `testcontainers-modules` / `amazon/dynamodb-local`. All green: `cargo test -p persistence` (49 integration + 15 `keys.rs` unit tests, 64 total). `cargo fmt --check` and `cargo clippy --workspace --tests --all-targets -- -D warnings` both clean.

---

## M3 detail — what landed

All paths relative to `infra/`.

- **`cdk.json`** — CDK app config with `python3 app.py` entry point and feature flags.
- **`requirements.txt`** — `aws-cdk-lib>=2.100.0`, `constructs`, `python-dotenv`, `pytest`, `ruff`, `mypy`.
- **`app.py`** — instantiates all M3 stacks; reads `--context env=<dev|prod>` and passes `EnvConfig` into each stack constructor.
- **`opennewsletter/config.py`** — `EnvConfig` frozen dataclass; `load_config(env)` reads `.env.local` (dev) or env vars (CI/prod); falls back to placeholder ARNs when M1 values are absent.
- **`opennewsletter/data_stack.py`** — `DataStack`: DDB table `OpenNewsletter-{env}`, pay-per-request, TTL on `ttl`, two GSIs (`gsi1`, `gsi2`), AWS-managed KMS, PITR in prod only, DESTROY removal in dev.
- **`opennewsletter/auth_stack.py`** — `AuthStack`: Cognito User Pool, 3 federated IdPs (Google/Apple/Facebook) via CFN dynamic references to Secrets Manager, `frontend` + `admin-bootstrap` app clients, Cognito-managed hosted UI domain. Lambda triggers (PreSignUp/PostConfirmation) wired in M4.
- **`opennewsletter/frontend_stack.py`** — `FrontendStack`: ACM cert (us-east-1) covering `domain`, `cdn.domain`, and `api_domain`; optional Route53 CNAME if `hosted_zone_id` is set.
- **`opennewsletter/media_persistent_stack.py`** — `MediaPersistentStack`: S3 originals + processed buckets (both private, Block-Public-Access all-on), CloudFront distribution with OAC, signed-cookie `KeyGroup` reading `infra/keys/cf-signing.pub.pem`.
- **`opennewsletter/media_pipeline_stack.py`** — `MediaPipelineStack`: `lambda-image-process` (shell stub, arm64, 1024 MB, `provided.al2023`), S3 `ObjectCreated` notification on `uploads/` prefix, least-privilege IAM.
- **`opennewsletter/monitoring_stack.py`** — `MonitoringStack` skeleton: SNS alarm topic (+ email subscription if `alarm_email` set), empty CloudWatch dashboard, AWS Budgets alarm ($10/mo). Metric widgets + alarms land in M13.
- **`infra/keys/cf-signing.pub.pem`** — dev placeholder RSA-2048 public key for CloudFront `PublicKey`. In prod, operator generates real keypair: `openssl genrsa 2048 | openssl rsa -pubout > infra/keys/cf-signing.pub.pem`, uploads private key to Secrets Manager.
- **`backend/lambda-stubs/lambda-image-process/bootstrap`** — shell stub so CDK asset hashing works at synth time; replaced by the real Rust binary in M8.
- **`infra/tests/test_stacks.py`** — 20 CDK assertion tests covering DDB keys/TTL/GSIs, Cognito user pool settings (2 clients, 3 IdPs, no self-signup), S3 Block-Public-Access, CloudFront KeyGroup attachment, Lambda arm64/1024 MB.

### M3 coverage vs. plan

- ✅ `DataStack` — DynamoDB table + 2 GSIs.
- ✅ `AuthStack` — Cognito user pool + 3 IdPs + 2 app clients + hosted UI domain.
- ✅ `MediaPersistentStack` — S3 originals + processed + CloudFront + KeyGroup.
- ✅ `MediaPipelineStack` — `lambda-image-process` stub + S3 event subscription.
- ✅ `FrontendStack` — ACM cert + optional Route53 records.
- ✅ `MonitoringStack` skeleton + AWS Budgets alarm ($10/mo).
- ✅ `infra/tests/test_stacks.py` — 21 assertion tests, all green.
- ✅ **`cdk synth --context env=dev` and `--context env=prod` both verified** (2026-07-14). `infra/.venv` recreated from `requirements.txt` (it's gitignored, not committed); `cdk` CLI run via `npx aws-cdk@2` since it isn't installed globally.
- 🟡 **`cdk deploy` still unverified** — needs an AWS account + `cdk bootstrap`. Blocked on B5 (M1 operator tasks) for a full end-to-end check; see "Suggested next steps."

**Bug found + fixed during verification**: `MediaPersistentStack` and `MediaPipelineStack` had a real circular dependency, not just an ordering issue. `MediaPipelineStack.add_event_notification()` was called directly on the `originals_bucket` object passed in from `MediaPersistentStack`; CDK attaches the `BucketNotifications` custom resource (and its singleton handler Lambda) to the *bucket's own* stack, so that resource ended up needing `MediaPipelineStack`'s Lambda ARN from `MediaPersistentStack` — while the Lambda's IAM grants (`grant_read`, `grant_put`) already needed `MediaPersistentStack`'s bucket ARNs the other way. `cdk synth` failed with `RuntimeError: ... would create a cyclic reference`.

  Fix ([`media_pipeline_stack.py`](../infra/opennewsletter/media_pipeline_stack.py)): inside `MediaPipelineStack`, re-import the originals bucket via `s3.Bucket.from_bucket_attributes(self, ..., bucket_arn=originals_bucket.bucket_arn)` and call `add_event_notification` on that imported reference instead of the real bucket object. This scopes the notification custom resource (and its singleton handler) to `MediaPipelineStack` — matching the stated design intent that redeploying the volatile pipeline stack should never require touching the stable persistent stack. Direct S3→Lambda notifications (not EventBridge) were kept per `08-media-uploads.md`'s explicit "no batching configurable on direct S3→Lambda notifications" requirement.

  This added a second `AWS::Lambda::Function` (CDK's own `BucketNotificationsHandler` singleton) to `MediaPipelineStack`'s template, so `test_image_process_lambda_exists` in `infra/tests/test_stacks.py` was updated to match on `FunctionName` rather than asserting a raw count of 1.

**Other cleanup during verification** (pre-existing issues, not introduced by the above): `ruff check .` found 3 unused imports (`aws_route53_targets` in `frontend_stack.py`, `aws_iam` in `media_pipeline_stack.py`, `MonitoringStack` in `test_stacks.py`) — auto-fixed. `mypy` found 43 errors, all from the same pattern: every stack's `**kwargs: object` doesn't satisfy `cdk.Stack.__init__`'s keyword-only parameter types; changed to `**kwargs: Any` across all 6 stack files. Also fixed a real `Optional[str]`-vs-`str` type error in `config.py`'s `get()` helper. `ruff` and `mypy` are both clean now.

---

## M3.5 detail — what landed

- **[`Makefile`](../Makefile)** — six targets at repo root:
  - `make deploy-dev` — `cdk deploy --all --outputs-file cdk.out/dev-outputs.json` + writes `frontend/.env.dev`
  - `make seed` — runs `scripts/seed_dev_data.py --env dev`
  - `make redeploy-volatile` — destroy + redeploy `DataStack-dev`, `MediaPipelineStack-dev`, `MonitoringStack-dev`
  - `make reset-all` — interactive prompt then `cdk destroy --all --force` (warns about Cognito loss)
  - `make deploy-lambda LAMBDA=<name>` — `cargo lambda build --release --arm64` + `aws lambda update-function-code` (~5s path)
  - `make fe` — `cd frontend && npm run dev -- --mode dev`
- **[`scripts/seed_dev_data.py`](../scripts/seed_dev_data.py)** — idempotent + destructive:
  - Reads `infra/cdk.out/dev-outputs.json` for table name, bucket names, user pool ID.
  - Scan + batch-deletes all DynamoDB items; deletes all S3 objects from originals + processed buckets.
  - Creates or finds the bootstrap admin Cognito user (idempotent; skips creation if already exists).
  - Writes 5 DynamoDB fixture items: CognitoSubLookup, User, Group, GroupMembership (admin), Newsletter (voting).
  - IDs are derived deterministically from the Cognito sub so re-seeding gives the same user/group IDs.
  - Accepts `--cycle-close-in DUR` (e.g. `5m`, `2h`, `4d`) to set the response-window deadline relative to now.
  - Prints sign-in credentials and the `admin-initiate-auth` CLI command for inner-loop testing.
- **[`scripts/write_frontend_env.py`](../scripts/write_frontend_env.py)** — reads `infra/cdk.out/{env}-outputs.json`, writes `frontend/.env.{env}` with all `VITE_*` vars. `VITE_API_BASE_URL` falls back to a TODO placeholder until `ApiStack` lands in M4.
- **Tick Lambda stubs upgraded** — `lambda-cycle-tick` and `lambda-notify-tick` replaced `fn main() {}` with proper `lambda_runtime = "1"` handlers (accept any event, return stub JSON). Added `lambda_runtime = "1"` to workspace deps. `cargo check --workspace` clean.
- **AWS Budgets alarm** (in `MonitoringStack`) — `CfnBudget` at $10/mo; alerts at 80% actual and 100% forecasted to `config.alarm_email`.

### M3.5 coverage vs. plan

- ✅ `Makefile` — all 6 targets per `13-dev-environments.md` §8.
- ✅ `scripts/seed_dev_data.py` — idempotent, destructive, `--cycle-close-in` supported.
- ✅ `scripts/write_frontend_env.py` — writes `frontend/.env.{env}` from CDK outputs.
- ✅ Dev removal-policy overrides — already applied in M3 (DynamoDB DESTROY, S3 auto_delete, log groups DESTROY).
- ✅ `MediaStack` split — already done in M3 (`MediaPersistentStack` + `MediaPipelineStack`).
- ✅ Tick Lambda stubs — `lambda-cycle-tick` and `lambda-notify-tick` are proper `lambda_runtime` binaries.
- ✅ AWS Budgets alarm — $10/mo in `MonitoringStack`.
- 🟡 **`POST /admin/dev/tick/{cycle|notify}` HTTP routes** — Lambda code ready; API Gateway routes wired in M5 when `ApiStack` is created.
- 🟡 **End-to-end verification blocked** — `infra/.venv` is ready; blocked on M1 operator tasks + CDK bootstrap + AWS account for `cdk deploy`. See `docs/RUNBOOK.md`.

---

## Active blockers

### B4 — Pre-existing mock-only frontend will need replacement in M7

Commit `91188ba` shipped a rich UI built against in-memory mocks. M7's deliverables (Vite + Tailwind + React Router skeleton wired to the real API + Cognito) overlap heavily with what's already there; expect that work to be partial rewrite, not greenfield.

### B5 — M1 operator tasks outstanding

OAuth app registrations (Google / Apple / Facebook), Cognito hosted-UI domain, and DNS records are still owed by the human operator. These are non-blocking for M2/M3/M3.5 code work but block any end-to-end auth verification in M4.

---

## Suggested next steps (in order)

1. ~~Verify CDK synth~~ — done 2026-07-14 (`cdk synth` clean for `dev` and `prod`, `pytest infra/tests/` 21/21, `ruff`/`mypy` clean). Note: `infra/.venv` is gitignored and not committed — recreate with `python3 -m venv .venv && source .venv/bin/activate && pip install -r requirements.txt`. The `cdk` CLI isn't installed globally in this environment either; invoke it via `npx aws-cdk@2 <command>`.
2. **Complete M1 operator tasks** (OAuth apps, VAPID keys, DNS) to clear B5 and enable end-to-end deploy.
   See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md) for the full step-by-step.
3. **Bootstrap CDK** in the dev AWS account:
   ```bash
   source infra/.venv/bin/activate && npx aws-cdk@2 bootstrap aws://ACCOUNT_ID/us-east-1
   ```
4. **Deploy and verify M3** (the `cdk deploy` half of the done-when gate) **and M3.5**: `make deploy-dev && make seed && make fe`
   (see `docs/RUNBOOK.md` §M3/M3.5 for what to check).
5. **Begin M4** (Auth + bootstrap): `lambda-invites` with PreSignUp trigger, `lambda-groups` with `/me` + `/groups`, `ApiStack`.

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
