# Progress & Blockers

Snapshot as of 2026-10-05. Working document — update as milestones complete or
blockers resolve. Authoritative milestone definitions live in
[`12-build-order.md`](12-build-order.md).

---

## Milestone status

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Repo skeleton | ✅ done | Layout, READMEs, `.gitignore`, `rust-toolchain.toml`, etc. landed in prior commits. |
| M1 | Operator prerequisites | 🟡 partial | Domain, Cognito domain prefix, `cdk bootstrap`, IAM user credentials, and VAPID secret are done. Google/Facebook OAuth apps and the real CloudFront signing keypair still owed by the operator (B5). No Apple IdP — dropped (see B5 decision). |
| **M2** | **Backend foundations (domain + persistence)** | ✅ **done** | See "M2 detail" below. All 64 tests green (`cargo test -p persistence`), fmt + clippy clean. Small follow-ups from the 2026-08-09 plan reconciliation listed under "M2 follow-ups". |
| **M3** | **Infrastructure baseline (CDK)** | 🟡 **synth + tests verified, deploy unverified** | See "M3 detail" below. |
| **M3.5** | **Dev environment online** | 🟡 **code-complete, unverified** | See "M3.5 detail" below. |
| **M4** | **Auth + bootstrap** | 🟡 **code-complete, deploy unverified** | See "M4 detail" below. All 101 backend tests + 34 CDK tests green; fmt/clippy/ruff/mypy clean; `cdk synth` clean for dev + prod. The done-when gate (curl `GET /me` with a real Cognito token) needs an AWS account — blocked on B5. |
| **M5** | **Lifecycle engine + cycle CRUD** | 🟡 **code-complete, deploy unverified** | See "M5 detail" below. The manual done-when flow needs AWS (B5); an end-to-end DynamoDB Local test covers the same sequence. `shared/openapi.yaml` + contract test landed. |
| **M6** | **Responses + drafts** | 🟡 **code-complete, deploy unverified** | See "M6 detail" below. 201 backend + 41 CDK tests green. The curl save → save → publish gate needs AWS (B5); DynamoDB Local integration tests cover the same sequence. |
| **M7** | **Frontend skeleton + auth** | 🟡 **code-complete, deploy unverified** | See "M7 detail" and "Post-M7 gap pass" below. 28 frontend, 217 backend and 42 CDK tests green. typecheck, ESLint (0 problems), Prettier, `vite build`, fmt, clippy, ruff (check + format), mypy and `cdk synth` are all clean. The Google sign-in / redeem / logout gate needs a deployed stack (B5). |
| **M8** | **Media pipeline** | 🟡 **code-complete, deploy unverified** | See "M8 detail" below. 308 backend, 53 CDK and 52 frontend tests green; fmt, clippy, ruff, typecheck, ESLint, Prettier, `vite build` and `cdk synth` (dev + prod) clean. The 5 MB JPEG upload → thumbnail → reload-via-CloudFront gate needs a deployed stack (B5) and an arm64 build of `image-process` (B6). |
| **M9** | **Newsletter UI** | 🟡 **code-complete, deploy unverified** | See "M9 detail" below. 116 frontend tests green (was 52). typecheck, ESLint (0 problems), Prettier and `vite build` are clean. Backend (308) and CDK (53) are unchanged, since M9 touched only `frontend/` and `plans/`. The simulated-month gate needs a deployed stack (B5). |
| **M10** | **Engagement** | 🟡 **code-complete, deploy unverified** | See "M10 detail" below. 361 backend, 54 CDK and 163 frontend tests green. fmt, clippy (`-D warnings`), ruff, mypy, typecheck, ESLint (0 problems), Prettier, `vite build` and `cdk synth` (dev + prod) are clean. The two-users-comment-and-react gate needs a deployed stack (B5). |
| M11 | Notifications | ⬜ | |
| M12 | Admin UI | ⬜ | |
| M13 | Hardening | ⬜ | |
| M14 | Production deploy | ⬜ | |

M0 and M2 are complete; M3's `cdk synth`/`pytest infra/tests/` are verified (a real cyclic-stack-dependency bug was found and fixed — see M3 detail), but the actual `cdk deploy` gate in `12-build-order.md` still requires an AWS account and is unverified. M3.5 through M10 are code-complete and awaiting the same deploy step. Everything deploy-shaped is blocked on B5 (M1 operator tasks).

---

## Plan-set reconciliation (2026-08-09)

A full consistency pass was made over `plans/00`–`13` so the documents no longer contradict each other. If you last read the plans before this date, re-read the touched sections. Headlines:

- **Invite flow**: `PreSignUp` is a pure pass-through; there is no `PostConfirmation` trigger; `POST /invites/redeem` is the single consumption point for first signup AND additional groups. Stale contrary text removed from `01` §4.1, `03` §3.4, `04` §6/§7.8, and the `05` §2 diagram.
- **Drafts**: last-write-wins everywhere. `RESPONSE_VERSION_CONFLICT` removed from the `03` error catalog, `12` M6 deliverables, and `11` test lists; the phantom `version` increment removed from `02` §4.
- **Avatar pipeline**: buckets, CloudFront `/avatar/*` behavior, and `lambda-image-process` wiring added to `01` §5 (handler branches on source **bucket**, not key prefix).
- **Comment images**: uploaded via `POST /uploads` with `purpose: "comment"` (valid while the cycle is `published`); `ImageMedia.purpose` added to `02` §2.10.
- **Dev media auth**: signed cookies can't cross raw AWS domains — dev uses signed-URL query params built from the `/media-cookie` JSON body (`08` §4.5).
- **Upload size cap**: enforced by `lambda-image-process` (mark `failed` + delete original) — `s3:content-length-range` doesn't exist for presigned PUT (`08` §3.2).
- **Reminder offsets**: capped at 168h (`MAX_REMINDER_OFFSET_HOURS`) so the notify-tick query window always covers them (`03` §4.3, `07` §7.1).
- Smaller: error catalog gained `LAST_ADMIN`/`CANDIDATE_PROMOTED`; gradient + avatarColor slug sets enumerated in `03` §4.3/§2.3; emoji predicate unified on `09` §2.2; VAPID keys consumed via `from_base64`, no PEM (`07` §2/§10); `lambda-push` direct-invoke envelope specified (`07` §6); `ApiStack`/`NotificationsStack` creation assigned to M4/M5 in `12`; CDK-test Lambda count corrected to 8 in `11` §3; `backend-ci` uses `cargo test --features integration` (no manual DDB-local step); dev uses raw AWS endpoints in `00` §7 and `03` §1; `voteWindowOpenAt` is informational-only (`06` §4.2); dev tick route body takes `groupId` (`03` §11a.1); `sub` vs `userId` confusion fixed in code sketches (`02` §6, `08` §4.3, `09` §1.4/§4.1); CSP concretely specified as a meta tag (`04` §12.1).

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

### M2 follow-ups (from the 2026-08-09 plan reconciliation) — ✅ all folded in during M4

- ~~`ImageMedia` entity + `persistence/media.rs`: add the new `purpose: response|comment` attribute~~ — `ImagePurpose` enum added to `domain/entities.rs` and threaded onto `ImageMedia`.
- ~~`shared/config.rs`: add `MAX_REMINDER_OFFSET_HOURS = 168` and the gradient/avatarColor slug lists~~ — done, and `PATCH /groups/{g}` validates against them.
- ~~`domain/error.rs`: drop `RESPONSE_VERSION_CONFLICT`~~ — dropped. `LAST_ADMIN` and `CANDIDATE_PROMOTED` were already present.

---

## M3 detail — what landed

All paths relative to `infra/`.

- **`cdk.json`** — CDK app config with `python3 app.py` entry point and feature flags.
- **`requirements.txt`** — `aws-cdk-lib>=2.100.0`, `constructs`, `python-dotenv`, `pytest`, `ruff`, `mypy`.
- **`app.py`** — instantiates all M3 stacks; reads `--context env=<dev|prod>` and passes `EnvConfig` into each stack constructor.
- **`opennewsletter/config.py`** — `EnvConfig` frozen dataclass; `load_config(env)` reads `.env.local` (dev) or env vars (CI/prod); falls back to placeholder ARNs when M1 values are absent.
- **`opennewsletter/data_stack.py`** — `DataStack`: DDB table `OpenNewsletter-{env}`, pay-per-request, TTL on `ttl`, two GSIs (`gsi1`, `gsi2`), AWS-managed KMS, PITR in prod only, DESTROY removal in dev.
- **`opennewsletter/auth_stack.py`** — `AuthStack`: Cognito User Pool, federated IdPs (Google/Facebook; Apple dropped 2026-10-05, see B5 decision below — 3 IdPs including Apple at the time this milestone landed) via CFN dynamic references to Secrets Manager, `frontend` + `admin-bootstrap` app clients, Cognito-managed hosted UI domain. The PreSignUp (pass-through) trigger is wired in M4; there is no PostConfirmation trigger (plans reconciled 2026-08-09).
- **`opennewsletter/frontend_stack.py`** — `FrontendStack`: ACM cert (us-east-1) covering `domain`, `cdn.domain`, and `api_domain`; optional Route53 CNAME if `hosted_zone_id` is set.
- **`opennewsletter/media_persistent_stack.py`** — `MediaPersistentStack`: S3 originals + processed buckets (both private, Block-Public-Access all-on), CloudFront distribution with OAC, signed-cookie `KeyGroup` reading `infra/keys/cf-signing.pub.pem`.
- **`opennewsletter/media_pipeline_stack.py`** — `MediaPipelineStack`: `lambda-image-process` (shell stub, arm64, 1024 MB, `provided.al2023`), S3 `ObjectCreated` notification on `uploads/` prefix, least-privilege IAM.
- **`opennewsletter/monitoring_stack.py`** — `MonitoringStack` skeleton: SNS alarm topic (+ email subscription if `alarm_email` set), empty CloudWatch dashboard, AWS Budgets alarm ($10/mo). Metric widgets + alarms land in M13.
- **`infra/keys/cf-signing.pub.pem`** — dev placeholder RSA-2048 public key for CloudFront `PublicKey`. In prod, operator generates real keypair: `openssl genrsa 2048 | openssl rsa -pubout > infra/keys/cf-signing.pub.pem`, uploads private key to Secrets Manager.
- **`backend/lambda-stubs/lambda-image-process/bootstrap`** — shell stub so CDK asset hashing works at synth time; replaced by the real Rust binary in M8.
- **`infra/tests/test_stacks.py`** — 21 CDK assertion tests covering DDB keys/TTL/GSIs, Cognito user pool settings (2 clients, 3 IdPs, no self-signup), S3 Block-Public-Access, CloudFront KeyGroup attachment, Lambda arm64/1024 MB.

### M3 coverage vs. plan

- ✅ `DataStack` — DynamoDB table + 2 GSIs.
- ✅ `AuthStack` — Cognito user pool + 3 IdPs + 2 app clients + hosted UI domain.
- ✅ `MediaPersistentStack` — S3 originals + processed + CloudFront + KeyGroup.
- ✅ `MediaPipelineStack` — `lambda-image-process` stub + S3 event subscription.
- ✅ `FrontendStack` — ACM cert + optional Route53 records.
- ✅ `MonitoringStack` skeleton + AWS Budgets alarm ($10/mo).
- ✅ `infra/tests/test_stacks.py` — 21 assertion tests, all green.
- ✅ **`cdk synth --context env=dev` and `--context env=prod` both verified** (2026-07-14). `infra/.venv` recreated from `requirements.txt` (it's gitignored, not committed); `cdk` CLI run via `npx aws-cdk@2` since it isn't installed globally.
- ⚠️ **Venv gotcha**: the system Python is 3.8 and typeguard 4.x breaks jsii's runtime type-checks on it (`check_type() got an unexpected keyword argument 'argname'` — 6 AuthStack tests error at fixture setup). `typeguard~=2.13.3` is now pinned in `requirements.txt`; if a venv predates the pin, `pip install "typeguard~=2.13.3"` fixes it. (A stray duplicate `infra/venv/` existed with the broken 4.2.1 and has been fixed in place; `infra/.venv` is the canonical one.)
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

## M4 detail — what landed

### Backend

- **`shared` crate** — grew from config-only into the handler-boundary crate the repo layout always described:
  - [`http.rs`](../backend/crates/shared/src/http.rs) — `AuthClaims` extraction from the API Gateway JWT authorizer, correlation-ID echo/mint, JSON body parsing, and the RFC-7807 problem response from `03` §1.1. 4 unit tests.
  - [`telemetry.rs`](../backend/crates/shared/src/telemetry.rs) — JSON `tracing` subscriber for Lambda entry points.
  - [`config.rs`](../backend/crates/shared/src/config.rs) — the three M2 follow-ups plus `INVITE_MAX_TTL_DAYS`, cycle/window bounds, and `derive_avatar_color()`.
- **`domain`** — `ApiErrorCode` gained `as_str()`, `slug()`, `title()` (the wire fields of the problem body); `RESPONSE_VERSION_CONFLICT` removed; `ImagePurpose` added and threaded onto `ImageMedia`.
- **`persistence`**
  - [`auth.rs`](../backend/crates/persistence/src/auth.rs) — `resolve_user_id`, `require_user_id`, and `require_membership` (the tenant-isolation gate from `05` §11), each memoized in a `DashMap` for `MEMBERSHIP_CACHE_TTL_SECONDS`, plus explicit invalidation on role change / removal.
  - [`expr.rs`](../backend/crates/persistence/src/expr.rs) — `set_fields()`, which binds every patch attribute to a `#f{n}` placeholder. Added after `timezone` (and then `name`) turned out to be DynamoDB reserved words; aliasing everything removes the whole class of bug rather than the two instances found.
  - `join_via_invite_tx` now takes an optional new `User` and writes the `CognitoSubLookup` + `User` rows inside the same transaction (first-ever redemption, `05` §4.2).
  - New: `groups::update_group` (targeted patch, so a concurrent join can't have `member_count` clobbered), `groups::update_membership_role`, `users::update_profile`, `users::get_users_batch` (one `BatchGetItem` for a group's member list).
  - `invites::revoke` is now conditional on `status = pending`, so revoking a consumed invite fails instead of erasing the audit trail.
- **`lambda-groups`** (binary `groups-api`) — `GET /healthz`, `GET /config`, `GET|PATCH /me`, `GET /groups`, `GET|PATCH /groups/{g}`, `DELETE|PATCH /groups/{g}/members/{u}`. Includes the last-admin guard on both removal and demotion, and full `03` §4.3 validation (IANA timezone via `chrono-tz`, gradient/avatarColor slugs, merged-then-validated cycle settings). 10 unit tests.
- **`lambda-invites`** (binaries `invites-api` + `invites-presignup`) — `POST /admin/invites`, `GET /admin/groups/{g}/invites`, `POST /admin/invites/{code}/revoke`, `POST /invites/redeem`, plus the pass-through `PreSignUp` trigger. 12 unit tests.

### Infrastructure

- **[`api_stack.py`](../infra/opennewsletter/api_stack.py)** — HTTP API, CORS, Cognito JWT authorizer, 50/25 rps throttling, JSON access logs, the two handler Lambdas (arm64, `provided.al2023`, 256 MB, 10 s), and all 13 M4 routes. `_ROUTES` is the single list to extend as later milestones add handlers. Custom domain + API mapping in prod only.
- **[`lambda_assets.py`](../infra/opennewsletter/lambda_assets.py)** — resolves `cargo lambda build` output, falling back to the shell stub so synth works on a machine that has never run the Rust build.
- **`AuthStack`** — now owns the `PreSignUp` Lambda and its trigger, and no longer declares the `pendingInvite` custom attribute (the invite code rides in the OIDC `state` per `05` §1).
- **[`bootstrap_admin.py`](../scripts/bootstrap_admin.py)** — creates the Cognito user, adds it to an informational `admins` group, and writes User + sub-lookup + Group + admin membership in one `TransactWriteItems`. IDs are UUID5-derived from the Cognito sub, so re-running is idempotent.

### Tooling caught up to M4

Three files still assumed ApiStack didn't exist; all now fixed:

- **`Makefile`** — `deploy-dev` never built the Rust binaries. Now that `ApiStack` exists and silently substitutes the shell stub for a missing binary, that would have deployed stubs that answer every route with nothing. Added a `build-lambdas` target and made `deploy-dev` depend on it.
- **`Makefile`** — `deploy-lambda LAMBDA=<name>` assumed the cargo binary name equals the CloudFormation function name. It doesn't any more (`groups-api` → `OpenNewsletter-Groups-dev`), so a `FUNCTION_<binary>` map was added. **Add a line to it whenever a new handler lands.**
- **`write_frontend_env.py`** — dropped the `TODO-wire-in-M4` placeholder for `VITE_API_BASE_URL`; it now reads `ApiStack`'s `ApiEndpoint` output and warns when absent.

Docs corrected alongside: `00` §4 repo layout (the media-stack split and `lambda_assets.py`), `11` §3 (the "exactly 8 Lambdas in `ApiStack`" assertion is wrong until M11, and `PreSignUp` isn't in that stack at all), `13` §8 (binary-vs-function naming), and `docs/RUNBOOK.md` §M4 (verify a real binary exists before deploying).

### M4 coverage vs. plan

- ✅ All 13 routes from `03` §13 wired, each behind the JWT authorizer (asserted by a test that walks every synthesized route).
- ✅ `PreSignUp` pass-through; no `PostConfirmation` trigger.
- ✅ `admin-bootstrap` Cognito app client — already existed from M3.
- ✅ **101 backend tests** (up from 64) and **34 CDK tests** (up from 21), all green. `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `ruff check`, `mypy --strict`, and `cdk synth` for dev + prod all clean.
- 🟡 **Done-when gate unverified** — needs a deployed stack and a real Cognito token (B5).

### Decisions and deviations worth knowing

- **The bearer token is the ID token, not the access token.** `03` §1 and `05` §4.1 contradicted each other; `05` §4.2's algorithm reads `jwt.email`/`jwt.name`, which only the ID token carries. Both docs now say ID token, and `POST /invites/redeem` returns `VALIDATION_FAILED` if the `email` claim is absent.
- **Invite expiry is enforced in Rust, not in the transaction's condition expression.** `expires_at` is stored as RFC-3339 with a variable-width fractional part, so a lexicographic `<` against "now" is not reliably ordered at sub-second resolution. The condition keeps the `status = pending` guard (which is what actually prevents double-redemption); the expiry race it leaves open is microseconds wide.
- **DynamoDB attribute names are snake_case, but `02-data-model-dynamodb.md` documents them as camelCase.** The snake_case naming is baked into M2's writers and its 73 tests. The HTTP contract is camelCase as specified, so the handlers map entities → DTOs explicitly. Worth reconciling `02` to match the code at some point; not worth churning M2 for.
- **`GET /config` returns `userId: null` with an empty `memberships` array** when the JWT is valid but no `User` row exists — the pre-onboarding state `05` §6 calls for.
- **Two binaries in `lambda-invites`** required naming them (`invites-api`, `invites-presignup`) rather than the usual `bootstrap`; `lambda-groups`' binary is `groups-api` for symmetry. Other Lambda crates still use `bootstrap`.
- **Fixed in passing**: `seed_dev_data.py` wrote `avatar_color: "#4A90D9"` and a raw CSS `gradient`, neither of which is a valid slug under the reconciled `03` §2.3/§4.3 enums — both would have failed validation on the first `PATCH`.

---

## M5 detail — what landed (2026-09-26, first work on the macOS machine)

### Backend

- **`shared/src/cycle_time.rs`** — `first_day_of_next_month_local`, `cycle_id_for`, `next_cycle_schedule` (`06` §4/§5.4/§5.5). Ambiguous local midnight (DST fall-back) resolves to the earliest instant; a nonexistent midnight walks forward hour by hour. Tests cover December rollover, the `06` §11 #8 New York spring-forward case, a Southern-hemisphere zone and a fixed-offset zone.
- **`persistence`**
  - `newsletters.rs` — `find_voting_cycle`, `find_open_cycle`, `list_recent` (base64url cursor), `create_next_voting_cycle` (put-if-not-exists), `publish_cycle`, `upsert_tick_sentinel`, `rewind_active_cycle_deadline` (dev tick; picks `open` if present, else `voting`).
  - `error.rs` — `RepoError::is_lost_race()`: condition-check / transaction-cancelled failures are now distinguishable, so the tick logs lost races at INFO and moves on (`06` §8).
  - **Key-timestamp normalization** — `keys::key_timestamp()` is now the only way a timestamp enters a key attribute (fixed-width `YYYY-MM-DDTHH:MM:SSZ`). Rust writers had been using `to_rfc3339()` (`+00:00`, variable fractional part) while the Python scripts and sentinel used `Z`, so the tick's lexicographic `gsi2sk <= now` comparison was only right by luck at sub-second edges. `newsletter_gsi2sk`, `image_gsi1sk` and `comment_sk` now take `DateTime<Utc>` so a raw string can't be passed. Documented in `02` §1.
- **`lambda-cycle-tick`** (binary `cycle-tick`) — one binary serves the EventBridge schedule and `POST /admin/dev/tick/cycle` (branches on `requestContext.http`; plain `lambda_runtime`, because `lambda_http` can't deserialize a scheduled-event payload). Dev route: 404 unless `ENV=dev`, admin checks per `03` §11a.1, returns `{ "transitions": [{ groupId, cycleId, from, to }] }` (`from: null` = newly created cycle). Notification fan-out is a no-op hook in `notify.rs` until M11. Has a `lib.rs` so tests can call `tick::run_tick`.
- **`lambda-newsletters`** (binary `newsletters-api`) — `GET /groups/{g}/newsletters` (paginated, `myDraftCount`/`myPublishedCount`) and `GET /groups/{g}/newsletters/{c}` for all four statuses (`archived` → 410 `NEWSLETTER_ARCHIVED`, a new error code).
- **`lambda-questions`** (binary `questions-api`) — candidate list/create, vote cast/withdraw (idempotent, vote-cap enforced), admin `DELETE /admin/groups/{g}/candidate-questions/{q}` with vote cascade and `CANDIDATE_PROMOTED`.
- **`lambda-groups`** — `PATCH /groups/{g}` reschedules the current `voting` cycle when `responseWindowDays`/`timezone` change and the cycle has no candidates (`06` §6; if the recomputed `cycleId` would differ, the row is left alone — documented there).
- **Scripts** — `bootstrap_admin.py` writes the group's first `voting` Newsletter in the same transaction as the group (Python schedule math verified against the Rust cases). `seed_dev_data.py` had been writing a UUID-hash `cycle_id` instead of `yyyymm` and was missing `published_at`/`next_transition_at`; fixed.

### OpenAPI contract (deferred from M4)

- **`shared/openapi.yaml`** — OpenAPI 3.1 for every route through M5 (M4's 13 + M5's 7). `redocly lint`: 0 errors, 2 intentional warnings (no `info.license`, placeholder server URLs).
- **`domain/src/api.rs`** — all wire DTOs, moved out of the per-crate `dto.rs` files (deleted).
- **`domain/tests/openapi_contract.rs`** — round-trips every YAML example through its `domain::api` type via `ROUTE_TABLE`; a YAML example without a table row, or vice versa, fails. Also asserts the `gradient`/`avatarColor` enums match `shared/src/config.rs`.
- **`scripts/codegen_types.sh`** → `frontend/src/types/api.ts` (generated, committed).
- **Real bug fixed in passing**: `GroupResponse` embedded the `CycleSettings`/`NotificationSettings` *entities*, which serialize snake_case, so `GET|PATCH /groups/{g}` returned snake_case settings inside a camelCase body. Now mapped through camelCase wire types.

### Infrastructure

- **`NotificationsStack`** (new) — cycle-tick Lambda (512 MB / 60 s) on a `rate(5 minutes)` EventBridge rule. `lambda-notify-tick` joins in M11.
- **`ApiStack`** — `Newsletters` + `Questions` Lambdas and the 7 M5 routes; `POST /admin/dev/tick/cycle` → cycle-tick, **dev only** (asserted absent from prod). `ApiStack` depends on `NotificationsStack`, never the reverse; `01` §1's dependency table was backwards and is corrected.
- **Fresh-clone fix** — `.gitignore`'s `*.pem` rule had swallowed `infra/keys/cf-signing.pub.pem`, so it only ever existed on the Windows machine; a fresh clone failed `cdk synth` and 9 CDK tests. Added `!infra/keys/*.pub.pem` and committed a new placeholder public key (private half discarded; the operator still generates the real pair in M1).

### M5 coverage vs. plan

- ✅ All deliverables in `12` M5, including the `06` §11 lifecycle tests (items 1–5, 7, 8 in `persistence/tests/lifecycle.rs`; item 6 is M4's `invites.rs` member-cap test).
- ✅ **End-to-end lifecycle test** (`lambda-questions/tests/lifecycle.rs`) — candidates + votes through the real handlers → rewind → tick → `open` with top-N locked → rewind → tick → `published` → next `voting` cycle exists. Stands in for the manual done-when flow until AWS is available.
- ✅ Verified on macOS: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`, `cargo test --workspace --all-features` — **167 tests, 0 failures** (DynamoDB Local via Colima), `pytest infra/tests` (40), `cdk synth` dev + prod.
- 🟡 **Done-when manual flow unverified** — needs a deployed stack (B5).

### Decisions worth knowing

- Admin candidate delete has no `cycleId` in its path: it checks the current `voting` cycle, then the current `open` cycle's locked questions (→ `CANDIDATE_PROMOTED`). There's no "voters of question X" access pattern, so the vote cascade deletes per group member (bounded by `memberSoftCap`). Both documented in `03` §6.5.
- `GET /candidate-questions` defaults to `sort=top` and pages in memory (small groups); the `open`-status newsletter shape was unspecified and is now documented in `03` §5.
- Test pollution: `persistence::auth`'s membership cache is process-wide, so integration tests must use unique group IDs or one test's cached role leaks into another.

---

## M6 detail — what landed (2026-09-27)

- **`lambda-responses`** (binary `responses-api`) — `GET …/my-responses`, `GET|PUT …/questions/{q}/my-response` per `03` §7. Mirrors the `lambda-questions` layout (lib/router/handlers/state/validation).
- **`persistence::responses::save_response`** replaces `save_draft`/`publish_response_tx`. Every save — draft or publish — is one `TransactWriteItems`: Update the response row + ConditionCheck on the Newsletter (`status = open AND response_close_at > now`) + the `editions_answered` bump on the caller's first publish in the cycle. A lost race maps to 409 `CYCLE_NOT_OPEN`. `persistence::questions::get_locked_question` added.
- **Decisions (documented in `03` §7.3 and `02` §4 #4):**
  - Publishing is sticky: `publish=false` after a publish updates content but stays `published`, with the original `publishedAt`.
  - The deadline is enforced by `responseCloseAt` as well as by status, so the ≤5-minute window before the tick runs is closed.
  - `responseId` is stable via `if_not_exists`.
  - Kind mismatch → `VALIDATION_ERROR`.
  - Image IDs are de-duplicated before the ≤10 check.
  - Saves don't modify `ImageMedia` rows (question linkage is M8).
- **OpenAPI** — 3 routes + `ResponseDto`/`MyResponsesList`/`SaveResponseRequest` in `shared/openapi.yaml`, `ROUTE_TABLE` rows, `frontend/src/types/api.ts` regenerated.
- **Infra** — `Responses` Lambda (512 MB) + 3 JWT routes in `ApiStack`, `FUNCTION_responses-api` in the Makefile, CDK test.
- **Tests** — 22 handler integration tests + 5 validation unit tests (`lambda-responses`), 12 repo tests (`persistence/tests/responses.rs`). They cover:
  - saves and publishing:
    - draft/get
    - interleaved saves with last-write-wins and a stable ID
    - publish, with one bump only
    - sticky publish
  - cycle state:
    - past deadline → 409
    - published cycle → 409
  - access and validation:
    - non-member
    - every image rule
    - body length
    - image cap
    - poll option
    - kind mismatch
    - my-responses scoping
    - 404
- **Known limitation:** two *concurrent* first publishes by the same user on different questions can both see zero published rows and double-bump `editionsAnswered`. It is cosmetic (a stats counter) and needs same-user parallel publishes in one instant, so it is accepted for v1.

---

## M7 detail — what landed (2026-09-27)

The work was split across three parallel agents (auth/API, shell/pages, infra); the lead reviewed and verified it. The frontend was installed and built on the Mac for the first time.

### Frontend — auth and API plumbing
- **`auth/`**:
  - `userManager.ts`: one `oidc-client-ts` `UserManager` doing PKCE against Cognito, with tokens in `sessionStorage`.
  - `AuthProvider` + `useAuth()`, exposing `{ status, user, login, logout }`.
  - `RequireAuth`, the layout-route guard.
  - `inviteStash.ts`.
- **`pages/AuthCallbackPage.tsx`** — processes the code exchange exactly once (StrictMode-safe). It then goes to `/join?code=…` if an invite rode along, else to `returnTo`.
- **`pages/BootstrapLoginPage.tsx`** — `/admin/bootstrap-login`, dev only. It calls Cognito `InitiateAuth` (`USER_PASSWORD_AUTH`) with the `admin-bootstrap` client and stores the result as an OIDC `User`.
- **`api/client.ts`**:
  - `apiFetch` sends `Authorization: Bearer <ID token>` and `x-correlation-id` (a ULID from `utils/correlation.ts`).
  - A 204 returns `undefined`; problem+json becomes `ApiError { status, code, problem }`.
  - A 401 triggers one silent renew and one retry.
  - The typed `api` object covers the M7 routes.
- **`api/mockTransport.ts`** — `VITE_USE_MOCKS=true` serves the M7 routes from in-memory fixtures (`u_quinn`, `g_trail`/`g_game`/`g_meeple`; invite code `DEMO-JOIN-CODE`), so `npm run dev` still runs offline.
- **Env** — `env.ts` parses `import.meta.env`. `.env.development` is checked in with mocks on; `make fe` → `--mode dev` → the generated `.env.dev` with mocks off.
- **Tooling** (first time in this repo):
  - ESLint 9 flat config: typescript-eslint, react, react-hooks, import-x, jsx-a11y.
  - Prettier with the Tailwind plugin, applied once across the whole frontend.
  - Vitest + Testing Library + jsdom.
  - New scripts: `test`, `format`, `format:check`.
  - `tsconfig.tsbuildinfo` untracked and gitignored.

### Frontend — shell, routing, pages
- **Router** (`App.tsx`) — every `04` §3 route is registered.
  - Guards: `RequireAuth` → `RequireMembership` → `RequireGroupAdmin`.
  - `/auth/callback` renders outside the shell; `/admin/bootstrap-login` is registered only when `VITE_ENV=dev`.
- **B4 resolved as a policy** (`04` §16):
  - The M9–M12 pages (Candidates, Suggest, Newsletter, Respond, GroupAdmin) and their data layer moved to `src/mocks/` (`legacyQueries.ts`, `types.ts`).
  - They render only in mock mode. Against a real API each shows `PendingMilestonePage`.
  - Each later milestone migrates its pages and deletes the legacy code it replaces.
- **Real hooks** — `api/queries.ts` (`useConfig`, `useMe`, `useGroup`, `useNewsletters`) and `api/mutations.ts` (`useRedeemInvite`, `useUpdateProfile`, `useLeaveGroup`).
- **Components** — `AppShell` (group switcher, account menu, defaults `currentGroup` to the first membership), `BottomNav`, `GroupSwitcher`, `JoinForm`, `Spinner`.
- **`HomePage`** — per-group sections built from parallel `listNewsletters` calls. Signed-out landing; join form when there are no memberships.
- **`JoinPage`** — stash + login when signed out; redeem-once when signed in. Error copy per `ApiError.code`; lands on `/g/{g}/upcoming` per `05` §4.5.
- **`SettingsPage`** — display name (1–40 chars, per `03`), avatar colour over the 8 `AvatarColorSlug`s, Your groups (View / Manage / Leave with the `LAST_ADMIN` copy), Sign out.

### Infrastructure
- **API authorizer audience** — the JWT authorizer now also accepts the `admin-bootstrap` client's audience. Without it, tokens from `/admin/bootstrap-login` and `bootstrap_admin.py` would have been rejected, contradicting `13` §6. There's a new CDK test for it.
- **`write_frontend_env.py`** — now writes `VITE_COGNITO_HOSTED_DOMAIN` (scheme stripped), `VITE_REDIRECT_URI` and `VITE_USE_MOCKS=false`.
- **ruff** — cleared the 7 findings that were already there (`as` aliases, `RUF015`, `app.py` exec bit), so `ruff check` is clean on the Mac too.

### Bugs caught in lead review (fixed)
- **Signed-out deep links bounced home.** `/g/:groupId/*` wasn't under `RequireAuth`, so a signed-out visitor was sent to `/` instead of login. It now nests under `RequireAuth`.
- **Redeeming an invite landed the user back on Home.** `JoinPage` navigated into the new group before `/config` had refetched, so `RequireMembership` saw the stale membership list and redirected to `/`. That is exactly the M7 done-when path. Two fixes:
  - `useRedeemInvite` now awaits an `invalidateQueries(…, refetchType: "all")`.
  - The guard waits while a refetch is in flight.
  - `JoinPage.test.tsx` covers it and was checked to fail on the old code.
- **Logout could turn into a login.** `removeUser()` fires `userUnloaded`, and `RequireAuth` then started `signinRedirect()`, whose redirect could override the Cognito `/logout` redirect. Unloaded events are now ignored while a logout is in progress.
- **An expired ID token forced a full sign-in on reload.** A reload after 60 minutes idle now tries one refresh-token renew first.

### Decisions (recorded in `04` §6/§7.8/§13/§16, `05` §7/§8, `01` §6.1/§6.3, `12` M7)
- Generated types stay at `frontend/src/types/api.ts`; `12` M7's `api/generated.ts` was corrected.
- `{ invite, returnTo }` rides in `oidc-client-ts`'s client-side state data, keyed by the opaque OIDC `state` nonce, not base64'd into `state`. There is a `sessionStorage` fallback.
- The callback hands the invite to `/join?code=…`, so there is one redemption code path.
- Silent renew uses the refresh-token grant; Cognito has no `prompt=none` flow. `05` §7 was wrong.
- Logout builds the Cognito `/logout` URL by hand, because the discovery document has no `end_session_endpoint`.
- The CSP `<meta>` tag is deferred to M13, as a build-only transform: the Vite React preamble is an inline script that `script-src 'self'` would block in dev.
- Stale `01` §6.3 text claimed `POST /invites/redeem` had no authorizer. §6.5 and the code say every route does; §6.3 is fixed.

### Tests
20 Vitest tests:
- `client` (5): auth + correlation headers, problem → `ApiError`, 204, 401 renew-and-retry, 401 renew-fails
- `RequireAuth` (3)
- `inviteStash` (3)
- `correlation` (3)
- `mockTransport` (3)
- `JoinPage` (3): the post-redeem landing regression, expired-invite copy, signed-out → login with the invite

`src/test/setup.ts` shims `localStorage`/`sessionStorage`, because Node 25+'s built-in Web Storage globals shadow jsdom's.

---

## Post-M7 gap pass and bug hunt (2026-09-27 → 2026-10-04)

After M7, every known gap was worked through and two read-only review agents (backend; frontend/infra/contract) swept the implemented code. Each finding below was verified against the code before it was fixed. A backend fix agent hit a session limit partway through; the lead finished its work.

### Bugs fixed
- **Last-admin race (backend).** Removing or demoting an admin checked the admin count with a separate read, so two admins demoting each other at once could leave zero admins. The write now carries a ConditionCheck that a *witness* admin is still an admin (`02` §4 #7); losing the race returns `LAST_ADMIN`. `lambda-groups` gained a `lib.rs` (crate template) and its first integration tests (`tests/members.rs`), including the concurrent mutual-demotion case.
- **Vote-cap race (backend).** `votesPerUserPerCycle` was only a pre-read, so parallel votes on different candidates could exceed it, contradicting `02` §4 #1 and `03` §6.3. A new `VoterTally` item (`02` §2.7a) with a conditional `ADD` now guards it inside the cast/withdraw transactions, and the admin candidate-delete cascade releases voters' slots.
- **Vote race-retry was dead code.** `cast_vote_tx`/`withdraw_vote_tx` turned every SDK error into `RepoError::Dynamo`, so `is_lost_race()` never matched, and two simultaneous votes on one question gave one voter a 500. Vote transactions now return a typed `VoteTxError` built from DynamoDB's per-item cancellation reasons: a duplicate vote is idempotent, a count race retries, and a full tally returns `VOTE_CAP_REACHED`.
- **Panics on dev-tick input.** `advanceCycleClosesBy` panicked in three ways: on a multibyte last char (`split_at`), on an out-of-range amount (`Duration::days`), and when `DateTime - duration` overflowed. It now splits on chars, uses the `try_` constructors, and bounds the input to 0–366 days.
- **DB errors masked as 404** in the admin candidate delete (`load_candidate(..).is_ok()`). Real lookup failures now propagate as 500.
- **`get_users_batch` ignored `UnprocessedKeys`**, so members could silently vanish under throttling. A shared `persistence::batch::batch_get_items` now chunks at 100 keys and retries unprocessed keys with backoff.
- **Home hid the open newsletter for the whole response window, every month.** Next month's voting cycle exists alongside the open one (`06` §5.2), and Home picked only the first. Both now render (test added).
- **Guards bounced newly granted access.** `/config` never goes stale, so a membership or role granted in another tab or on another device was refused. `RequireMembership`/`RequireGroupAdmin` now refetch once before denying (`routes/useConfirmedAccess.ts`, tests added).
- **Bootstrap-login sessions were signed out at the 60-minute mark.** `automaticSilentRenew` renewed through the frontend client, failed, and the error handler removed the user. Renewal is now `auth/renewSession.ts`: the refresh-token grant for federated sessions, Cognito `REFRESH_TOKEN_AUTH` for bootstrap sessions (picked by the ID token's `aud`).
- **ID-token decoding garbled non-ASCII names** (bare `atob`). It now decodes as UTF-8.
- **Keyboard users couldn't close the account menu or group switcher.** A shared `useDismissableMenu` adds Escape plus focus management (test added).
- **The group switcher didn't navigate** when you picked a group; it now goes to `/g/:g`.

### Gaps closed
- **`MembershipSummary` gained `timezone` and `gradient`**, read with one `BatchGetItem` instead of a `GetItem` per group. Home labels cycles in the group's timezone, and the switcher shows gradient swatches (`utils/gradient.ts`).
- **`fieldErrors` is populated.** `ApiError::invalid_field(field, detail)` covers 35 field-specific validation sites, with `code: "INVALID"`. `03` §1.1 had contradicted itself on field codes and now documents this.
- **`GET /healthz` returns `version`/`buildAt`**, baked in at compile time from `BUILD_SHA`/`BUILD_AT`, which `make build-lambdas` sets.
- **Frontend cleanup:**
  - removed the legacy "Curate" tab
  - ESLint is at 0 warnings
  - `.app-container` matches `04` §4.1 widths (48rem at md, 42rem at lg)
  - Home split into one component per file (`components/home/`)
- **Python formatting:** `ruff format` applied to `infra/` and `scripts/`, and the scripts' executable bits restored.
- **Doc reconciliation:**
  - `02` reconciled to snake_case item attributes, with 5 flagged notes.
  - `02` §4's `ClientRequestToken` claim removed; it was never implemented, and a reused token breaks the vote retry.
  - `13` §16 (frontend mocking) rewritten.
  - The `04` auth file layout now matches the code.
  - CLAUDE.md gained the frontend check command.

---

## Active blockers

### B5 — M1 operator tasks outstanding

**Updated 2026-10-05.** Progress so far: domain decided (`betterloop.site`, bought on Namecheap — DNS stays on Namecheap's nameservers, no Route53 hosted zone, so ACM validation CNAMEs get added there manually when `cdk deploy` prints them); Cognito domain prefix picked; AWS CLI credentials fixed to use IAM user `bquinn17` via `aws login` (was briefly root — see note below) with `AdministratorAccess` + `SignInLocalDevelopmentAccess`; `cdk bootstrap` done (`CDKToolkit` stack `CREATE_COMPLETE`); `cargo-lambda` installed on macOS (clears part of B6); VAPID keypair generated and stored at `opennewsletter/vapid/dev` (required fixing `scripts/generate_vapid_keys.py` — the `py-vapid` API it called, `public_key_urlsafe_base64()`/`private_key_urlsafe_base64()`, doesn't exist in the installed 1.9.4; replaced with direct RFC 8292 raw-point encoding via `cryptography`); `ALARM_EMAIL` set.

Still outstanding: Google + Facebook OAuth app registrations, the real CloudFront signing keypair (`infra/keys/cf-signing.pub.pem` is still the repo's dev placeholder), and the two corresponding Secrets Manager entries (`opennewsletter/oauth/google/dev`, `opennewsletter/oauth/facebook/dev`, `opennewsletter/cdn-signing/dev`). These are non-blocking for code work but block the M3/M3.5/M4 deploy gates and any end-to-end auth verification. See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md).

**Decision — Apple IdP dropped (2026-10-05).** Sign In with Apple requires a paid Apple Developer membership ($99/yr); the owner declined. Removed `UserPoolIdentityProviderApple` from `AuthStack`, `apple_oauth_secret_arn` from `EnvConfig`, and the Apple step from `docs/milestone-1-operator-runbook.md`; updated the CDK test (`test_user_pool_two_idps`, was `_three_idps`) and every plan doc that listed three IdPs (`00`, `01`, `12`, `13`, `OpenNewsletter_spec.md`) to Google/Facebook only. `cdk synth --context env=dev` and `pytest infra/tests/` (54 passed) both verified green after the change. If Apple support is wanted later, re-add the IdP construct, the 4 config fields (`apple_client_id`/`apple_team_id`/`apple_key_id`/`apple_private_key_secret_arn` per `01` §4.1, collapsed to one `apple_oauth_secret_arn` in the actual `config.py`), and the operator-runbook step.

**Note — root-credential near-miss.** `aws login` was initially run while signed into the AWS Console as the **root** user, so it issued short-term rotating keys under the root identity rather than an IAM user. `aws login` works with any identity you authenticate as in the browser; it doesn't itself pick a non-root one. Fixed by attaching `AdministratorAccess` + `SignInLocalDevelopmentAccess` to `bquinn17` and re-running `aws login`, signing in as that IAM user. Worth calling out in `docs/milestone-1-operator-runbook.md` §"AWS account and credentials" if that section gets revised.

### B6 — Lambda release builds (Windows/WSL2 only) + `cargo-lambda` not installed anywhere

**Windows/WSL2**: `cargo build --release` (and therefore `cargo lambda build --release`) aborts in `aws-lc-sys`, which refuses to compile under gcc 9 because of [gcc bug 95189](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189). That host is Ubuntu 20.04 with only gcc 9.4 and no clang. Debug builds and the test suite are unaffected. Fix: `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain; failing both, switch rustls to the `ring` backend.

**macOS**: the gcc issue doesn't apply (Apple clang). `cargo-lambda` 1.9.2 is now installed (2026-10-05, see B5). Release artifacts exist under `backend/target/lambda/` for every binary built before M10, dated 2026-10-04 21:14. They include `image-process`, but that cross-compile has not been run through the M8 gate. **`engagement-api` has no artifact yet**: rebuild before deploying, or the stack falls back to the stub for it.

**Windows/WSL2** has never produced a Lambda artifact. `ApiStack`/`NotificationsStack` fall back to the shell stub when `backend/target/lambda/{binary}/bootstrap` is missing, which is why synth passes — deploying before this is cleared would ship stubs.

---

## Suggested next steps (in order)

1. **Finish the M1 operator tasks** to clear B5: Google/Facebook OAuth apps, the real CloudFront keypair and their Secrets Manager entries. VAPID, `cdk bootstrap` and `cargo-lambda` (macOS) are done. See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md).
2. **Rebuild the Lambda artifacts** (`cargo lambda build --release --arm64`) so `engagement-api` gets one (B6).
3. *(done 2026-10-05: CDK bootstrap)*
4. **Deploy and verify M3–M5**: `make deploy-dev && make seed && make fe` (see `docs/RUNBOOK.md`). M4 gate: `bootstrap_admin.py` then `curl -H "Authorization: Bearer $TOKEN" "$API_ENDPOINT/me"`. M5 gate: the five-step manual flow in `12` M5 using `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy`.
5. **M6 gate**: after deploy, the curl save → save → publish dance against `PUT …/my-response` (`12` M6).
6. **M7 gate**: `make fe`, then:
   - sign in with Google
   - redeem an invite made by `POST /admin/invites` and see the group on Home
   - log out
   Also try `/admin/bootstrap-login` with the `bootstrap_admin.py` credentials.
7. **M8 gate**: after deploy, `cargo lambda build --release --arm64` must succeed for `image-process`. Its `libwebp-sys` C build has only been compiled natively on macOS, never cross-compiled. Then upload a 5 MB JPEG through `ImageUploader`, see the thumbnail, reload, and confirm it renders via CloudFront. In dev that uses the signed-URL query params; check that a group-B image URL with group-A params is a 403.
8. **M9 gate**: after deploy, run a simulated month in the UI:
   - suggest and vote on candidates
   - `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy` to promote and open
   - draft with images, reload, then publish text and poll answers
   - tick again to publish, then read the edition
9. **M10 gate**: after deploy, two users comment on each other's published answers (one with an image), edit and delete a comment, and react with a ZWJ emoji (e.g. 👨‍👩‍👧‍👦) and a flag. An admin deletes the other user's comment. Check that API Gateway hands the Lambda the emoji path param decoded or encoded; the handler accepts both (`03` §8.6).
10. **Begin M11** (Notifications). See the Handoff below. Standing rule from M5: every new or changed route updates `shared/openapi.yaml` and `ROUTE_TABLE` in `domain/tests/openapi_contract.rs` in the same PR, then re-runs `scripts/codegen_types.sh`.

Workstation setup for either machine: [`13-dev-environments.md`](13-dev-environments.md) §0.

### Known gaps carried forward

- **Comment author names: no cross-request cache.** `09` §1.5 suggests a 60s per-cold-start user cache. M10 does one deduped `BatchGetItem` per request instead. Add the cache if user reads show up as hot.
- **`get_comment_by_id` walks the answer's whole `C#` range with a filter**, because there's no index on `comment_id`. That's fine at expected thread sizes; revisit if threads get long.
- **The comment composer's image upload and the `[deleted]` placeholder** are only exercised in mock mode and component tests, since there's no deploy yet (M10 gate).
- **Legacy mock query keys collide with real ones in mock mode.** `src/mocks/legacyQueries.ts` still caches `["config"]` and `["group", g]` with the legacy shapes, and `GroupAdminPage` (M12) uses them. With `VITE_USE_MOCKS=true`, opening the admin page can poison the real `/config` cache for the session. This goes away when M12 migrates the page.
- **`utils/avatar.ts#avatarClasses` uses the legacy colour slugs** (coral/grape/mint…), not the API's `AvatarColorSlug` set. New M9 code uses `utils/avatarColor.ts`. `Avatar`'s `color` prop still falls back to the legacy table, so callers should pass `colorClassName`. Remove the legacy table with the last legacy page.
- **`RespondPage` has no markdown formatting toolbar** (the legacy page had bold/italic/list buttons). The spec only requires Write/Preview. Add it if users ask.
- **No "hype check" banner** (`04` §14a `utils/hype.ts`) and no recurring-question helper text (`utils/recurring.ts`). Both are optional decoration and not built.

The post-M7 gap pass (2026-09-27, below) cleared most of the earlier list. What remains:

- **`VOTE_CAP_REACHED` lists the already-voted questions in `detail`** (free text), not a structured field. `fieldErrors` now exists but is per-field validation, not a cap conflict.
- **`lambda-cycle-tick` needs `#![recursion_limit = "256"]`** since it gained a `lib.rs`. Harmless, but if it creeps up again, box the dispatch futures.
- **`POST /admin/dev/tick/notify`** is not wired — it arrives with `lambda-notify-tick` in M11.
- **Future-milestone stub crates still name their binary `bootstrap`**: `lambda-push` and `-notify-tick` (`-engagement` was renamed `engagement-api` in M10). Every `cargo build` prints output-filename-collision warnings. Rename each to `<name>-api` (per `CLAUDE.md`) when its milestone implements it.
- **The Rust toolchain is unpinned `stable`.** Rust 1.99 (2026-09-28) added clippy's `double_must_use`, which fired through `#[async_trait]` in `lambda-image-process/src/storage.rs`. It's now `allow`ed with a reason. A new stable can break `-D warnings` without any code change; pin a version in `rust-toolchain.toml` if that keeps happening.
- **`02` carries 5 "Note (reconciled 2026-09-27)" flags** for discrepancies the doc sweep didn't resolve by renaming. Four are attributes the code writes but `02` omits: `cognito_sub` on the sub lookup, and the denormalized IDs on `CandidateVote`/`Comment`/`Reaction`. The fifth: the `NOTIFIED#…`/`TICK#NOTIFY` idempotency rows aren't written until M11.
- **No Playwright/E2E yet** (`04` §1); `11` puts it in M13, together with the build-only CSP `<meta>` tag (`04` §13).

---

## M8 detail — what landed (2026-10-04)

The lead settled the contract first: every media route and DTO went into `shared/openapi.yaml`, and `frontend/src/types/api.ts` was regenerated. Then four agents ran in parallel: lambda-media and image-process (Sonnet), infra (Haiku) and frontend (Sonnet). The lead reviewed the signing, presign, delete and processing cores and fixed two infra omissions.

### Backend — `lambda-media` (binary `media-api`)
- All nine routes: `POST /uploads`, `GET`/`PATCH`/`DELETE /uploads/{imageId}`, `POST /uploads/{imageId}/complete`, `GET /media-cookie`, `POST /avatars`, `GET`/`DELETE /avatars/{avatarId}`.
- Presigned PUTs are built offline from the S3 client, with `RequestChecksumCalculation::WhenRequired`. The response's `headers` map is every signed header. An optional `sha256` is signed as `x-amz-checksum-sha256`.
- `src/cloudfront.rs` holds the pure policy, RSA-SHA1 signing and modified-base64 code. The signing key is loaded lazily from Secrets Manager (PKCS#1 or PKCS#8 PEM) and cached for the process.
- Persistence:
  - `media::count_active_response_images` (bounded to the cycle partition)
  - `set_image_caption`
  - `mark_image_deleted` / `mark_avatar_deleted`
  - `users::clear_user_avatar_if` (conditional)
- New error code `IMAGE_IN_USE` (409).
- 54 crate tests: 23 unit and 31 DynamoDB Local.

### Backend — `lambda-image-process` (binary `image-process`)
- An S3 trigger that branches on the source bucket name, URL-decodes the key, and skips rows that aren't `pending`.
- Checks `ContentLength` before downloading. An oversized original is marked `failed`/`IMAGE_TOO_LARGE` and deleted.
- Decodes with `image` 0.25 and applies EXIF orientation. Non-GIF variants are lossy WebP via the `webp` crate (vendored libwebp); GIFs are resized frame by frame so animation survives. Avatars are center-cropped to 256×256 WebP. Nothing is upscaled.
- `persistence::media_status` adds `mark_{image,avatar}_{ready,failed}`, each conditioned on `status = pending` and returning `Updated`/`AlreadyFinal`.
- S3 sits behind an `ObjectStore` trait with an in-memory fake, so the handler tests use DynamoDB Local and no S3.
- 24 crate tests (17 unit, 7 integration) plus 7 persistence tests.

### Infrastructure
- `MediaPersistentStack`:
  - avatar originals and processed buckets
  - a `/avatar/*` CloudFront behavior with no key group
  - originals-bucket CORS `allowed_headers=["*"]`
  - the `cdn_key_pair_id` output
- `MediaPipelineStack`:
  - real `image-process` code
  - a second S3 notification on the avatars bucket (re-imported by ARN, no cycle)
  - `GetItem`/`UpdateItem` on the table, read+delete on originals, put on processed (`GetItem` added by the lead)
- `ApiStack`:
  - the `Media` Lambda and its 9 routes
  - CORS `allowCredentials`
  - `CDN_BASE_URL` computed once (the distribution domain in dev, `cdn.{domain}` in prod) and passed to the groups, newsletters, questions and media Lambdas. Newsletters and questions previously had none.
- Makefile: `FUNCTION_media-api`, `FUNCTION_image-process`.

### Frontend
- `api.media.*` in `api/client.ts`. Only `getMediaCookie` sends `credentials: "include"`. The `/uploads/{id}*` calls carry `groupId`/`cycleId` query params.
- `utils/media.ts`:
  - `ensureCookie` caches per group, refreshes 5 minutes before expiry, and shares in-flight requests
  - `withMediaAuth` adds dev signed-URL params
  - `refreshMediaAuth`
  - `pollMediaStatus`
  - `sha256Base64`
- `utils/uploadToS3.ts` does the XHR PUT with progress.
- `components/ui/CdnImage.tsx` retries a failed image once after refreshing the cookie.
- `components/responses/ImageUploader.tsx` is standalone and not wired into the legacy `RespondPage`. It covers drag-drop and picker, client validation, presign → PUT (one retry on 403) → poll, thumbnails, alt text, remove and retry.
- `SettingsPage` has avatar upload and remove.
- Stateful mocks for all nine routes.

### Decisions (recorded in `03` §1.1/§9, `08` §3–§5/§9, `02` §2.10/§2.16, `01` §5.3/§6)
- `/uploads/{imageId}*` routes take **required `groupId` + `cycleId` query params**, because the row is keyed by them. `GET`/`complete` are open to any member; `PATCH`/`DELETE` are owner-only.
- `sha256` is **standard base64** and is signed as `x-amz-checksum-sha256` (the spec had said "base64url" and `x-amz-content-sha256`).
- The 10-image cap counts within the cycle partition, not GSI1. A brief overshoot under concurrent uploads is accepted, because the response save caps at 10 anyway.
- `DELETE /uploads` soft-fails the row (`errorMessage: "DELETED"`) and refuses with `IMAGE_IN_USE` when the caller's published answer lists the image.
- `ImageMedia`/`AvatarMedia` gained `error_message` (`IMAGE_TOO_LARGE` / `IMAGE_DECODE_FAILED` / `DELETED`).
- `GET /media-cookie` returns `expiresAt` as RFC 3339. It sends `Set-Cookie` only when `MEDIA_COOKIE_DOMAIN` is set (prod), with `Path=/img/{groupId}/` and `Max-Age=3600`.
- The client hashes with `crypto.subtle.digest`, not a Web Worker.
- A PUT-403 retry calls `POST /uploads` again and gets a new ID; the abandoned `pending` row is tolerated per `08` §8.1.
- Lossy WebP needs `libwebp` (the `webp` crate), because `image`'s own WebP encoder is lossless-only.
- Processed objects carry `Cache-Control: public, max-age=31536000, immutable` (img) or `max-age=86400` (avatar).

### Tests
Backend 308 (was 217), CDK 53 (was 42), frontend 52 (was 28). Of the `08` §10 list, the backend covers:
- the presign shape (1)
- all four formats (2)
- GIF animation (3)
- EXIF orientation (4)
- the 16 MB failure and delete (5)
- SVG → 415 (6)
- group-scoped cookie policy and signature verification (7, at the policy level, not against live CloudFront)
- the 11th upload → `IMAGE_LIMIT_EXCEEDED` (8)

The live-S3 and live-CloudFront halves of 1 and 7 need a deploy.

**Testcontainers flakiness:** with every DynamoDB Local suite running at full parallelism, Colima intermittently drops container connections (`DispatchFailure … IncompleteMessage`). `cargo test --workspace -- --test-threads=4` is reliable on the Mac.

---

## M9 detail — what landed (2026-10-04)

The lead read the spec first and laid the shared groundwork, so the two agents never touched the same files:
- the `api.*` client methods
- the query and mutation hooks
- `useMembership`
- `formatInZone`
- `env.autosaveDebounceMs`
- per-feature mock-route seams
- unconditional routing of the four pages

Two Sonnet agents then ran in parallel: candidates/suggest, and newsletter/respond. Both hit a rate limit mid-read and were resumed with their context. The lead's review found and fixed two data-loss bugs in the respond flow (below).

### Shared groundwork (lead)
- `api/client.ts`: `getNewsletter`, `listCandidates`, `createCandidate`, `castVote`, `withdrawVote`, `listMyResponses`, `getMyResponse`, `saveMyResponse`.
- `api/queries.ts`:
  - `useNewsletter` and `useCandidates(groupId, sort)`
  - `useMembership(groupId)`, which reads the group's timezone and role from `/config`
  - `queryKeys.candidatesAll` as the invalidation prefix
- `api/mutations.ts`: `useCreateCandidate`, plus `useToggleVote`, which patches the vote tally into every cached sort order and then refetches.
- The mock transport is split into per-feature routes:
  - `mockShared.ts`: `fail`, `CALLER_ID`, and the `newsletters` fixture, now dated relative to the current time
  - `mockCandidates.ts`
  - `mockNewsletters.ts`
  - `mockTransport.ts` falls through to the per-feature routes.
- `App.tsx`: Candidates, Suggest, Newsletter and Respond are routed unconditionally. Only `/admin` still uses `PendingMilestonePage` against a real API.

### Candidates and Suggest
- `components/candidates/{CandidateCard,CandidateList,SuggestQuestionForm}.tsx`.
- `CandidatesPage`:
  - a countdown banner to the `voting` cycle's open date, in the group timezone
  - a link to the `open` cycle when one exists
  - a Top/Recent toggle
  - a "used X of Y votes" line and an inline "Manage votes" panel with Remove buttons
  - un-voted buttons disabled at the cap
- `CYCLE_NOT_VOTING` from the list call shows a calm "voting hasn't opened" state.
- `SuggestPage`: react-hook-form + zod (text/poll, 2–6 options, "Ask anonymously"). It mirrors the server's trimming and **case-sensitive** duplicate-label rule. Server `fieldErrors` map onto fields.

### Newsletter and Respond
- `NewsletterPage`:
  - `voting` redirects to `/upcoming`
  - `open` shows question rows with a draft preview, Not started / Draft / Published pills, a deadline and a progress count
  - `published` shows expandable questions with `AnswerCard`s and `PollWidget` tallies (the caller's vote highlighted)
  - 410 shows an "archived" state
- `components/newsletter/{QuestionPrompt,ImageGallery}.tsx` are new. `AnswerCard`/`PollWidget` were rewritten on API types, with an M10 slot for comments and reactions.
- `utils/markdown.tsx` provides `MarkdownBody`: react-markdown + remark-gfm + rehype-sanitize. `image:{id}` tokens become `CdnImage`; remote images become links.
- `components/responses/`:
  - `ResponseEditor`: Write/Preview, the uploader mounted, image tokens inserted on `ready` and stripped on remove, a word count
  - `useResponseEditor`: debounce, blur/hide/unmount flush, one save in flight with coalescing, backoff (`utils/retry.ts`), stop on `CYCLE_NOT_OPEN`, localStorage mirror and restore
  - `DraftStatusBadge`
  - `PublishButton`
- `RespondPage`:
  - text and poll answers
  - read-only published view with an Edit toggle
  - "changes update your published answer" note
- `ImageUploader` now resumes polling for `pending` initial images (the M8 gap).

### Bugs caught in lead review (fixed, with regression tests)
- **Draft images were dropped when the page opened.** `ResponseEditor` mounted before the draft's images had loaded. `ImageUploader` reads `initialImages` once, so it reported zero images and the autosave cleared `imageMediaIds`. The editor now waits for hydration (`Promise.allSettled`; an image that can't be fetched is dropped). The published read-only view does its own lookup by the current image IDs.
- **Opening a question created an empty draft.** Blur, tab-hide and unmount always PUT. They now flush only when the local draft differs from the server.
- Smaller fixes:
  - Only `CYCLE_NOT_OPEN` (not every 409) puts the editor into "closed".
  - Remote markdown images no longer render as `<img>`.
  - `<button>`-inside-`<Link>` nesting removed.
  - `aria-pressed` added to the sort toggle.
  - A cross-agent test pinned a literal cycle ID; it now uses `VOTING_CYCLE_ID`.

### Decisions (recorded in `04` §7.2–§7.5/§9/§16, `08` §7, `coding-standards.md` §3.9, `11` §4.2, `13` §16)
- **Component tests use `vi.mock` of `api/client` plus cache seeding, not MSW.** The standard was amended to match.
- Every date on group pages is in the group's timezone. The published layout takes its month label from the cycle's `NewsletterSummary`.
- Comments and reactions are deferred to M10.
- The vote-cap "tooltip" is an inline accessible panel. The ID list in `VOTE_CAP_REACHED`'s `detail` is never shown.
- Poll answers:
  - before first publish, a pick is local only
  - after it, each change saves immediately with `publish:false` (sticky)
- `react-markdown`'s `urlTransform` must allow `image:` separately from the sanitize schema.
- Leftover legacy code was pruned: 19 dead hooks removed from `src/mocks/legacyQueries.ts`. Only `GroupAdminPage` (M12) and `CommentList`/`ReactionBar` (M10) still use `src/mocks/`.

### Tests
Frontend 116 (was 52), across 22 files:
- `mockCandidates` (13) and `mockNewsletters` (9)
- `CandidatesPage` (5) and `SuggestPage` (7)
- `NewsletterPage` (4) and `RespondPage` (4)
- `useResponseEditor` (8)
- `markdown` (11) and `retry` (2)
- one more `ImageUploader` test

---

## M10 detail — what landed (2026-10-05)

The lead read the spec, settled its gaps, and wrote the engagement contract into `shared/openapi.yaml` (then regenerated `api.ts`) before any agent started. Three agents then ran in parallel:
- a Sonnet backend agent
- a Haiku infra agent
- a Sonnet frontend agent

The backend and frontend agents hit a session limit and an auth expiry, and were resumed with their context. The backend agent never delivered its final report, so the lead audited its work directly (below).

### Backend
- `lambda-engagement`, binary renamed to `engagement-api`, laid out like `lambda-responses`. It serves all seven `03` §8 routes, with the check order fixed there: membership → cycle exists → not archived → published → answer resolves.
- `domain::engagement`: `normalize_emoji` (percent-decode if needed, then NFC), `is_valid_emoji` (1–12 codepoints, with an `Extended_Pictographic` or regional-indicator codepoint), and `group_reactions` (count desc, earliest, emoji). `lambda-newsletters` now uses the same `group_reactions` (it used to sort by emoji) and the same `CommentResponse` hydration. Soft-deleted comments come back as placeholders instead of being filtered out.
- `persistence::engagement`:
  - a paginated `list_comments_page`
  - `get_comment_by_id`
  - transactional `create_comment`, `update_comment` and `soft_delete_comment`, which claim and release the comment image via `ImageMedia.attached_comment_id`
  - `persistence::cursor` for the opaque cursor
- `lambda-media` `DELETE /uploads/{id}` returns 409 `IMAGE_IN_USE` for a claimed comment image (the carried M9 gap).

### Infrastructure
- `ApiStack` gains the `Engagement` function (`engagement-api`). It gets the same `CDN_BASE_URL` environment as newsletters-api and table read/write via `_handler_lambda`.
- The seven routes are wired.
- `Makefile` gains `FUNCTION_engagement-api`.
- CDK tests: route and arm64 coverage, plus a new env test.

### Frontend
- `api.engagement.*` client methods, with the emoji `encodeURIComponent`-ed.
- Mutations: `useToggleReaction` patches the cached newsletter optimistically, takes the server's `ReactionsResponse` on success, rolls back on error, and cancels in-flight refetches first. `useAddComment`, `useEditComment` and `useDeleteComment` patch the cache, then invalidate.
- `ReactionBar` has toggle pills, quick picks (🔥🤣❤️😍👍🎉) and a `+` picker. `EmojiPickerPopover` is now a curated 64-emoji grid plus a native input, and the `emoji-picker-element` dependency was dropped (the lazy chunk went from about 150 kB to 2.6 kB).
- `CommentList`:
  - the last 3 comments, with "Show all N" above
  - a composer with a single `purpose="comment"` image
  - inline edit, and a `ConfirmDialog` before delete; admins can delete any comment
  - a `[deleted]` placeholder
  - relative times, with the absolute time in the group's timezone as a tooltip
- `MarkdownBody` gains `allowImages={false}` for comments.
- `utils/emoji.ts` mirrors the server's emoji predicate.
- `NewsletterPage` shows the reaction total in the published header (`04` §14a).
- `api/mockEngagement.ts` is registered in `mockTransport`, and the published fixture is seeded with an edited comment, a deleted comment and reactions.
- The legacy engagement hooks were removed from `src/mocks/`.

### Bugs caught in lead review (fixed)
- **An edit racing a delete could resurrect a deleted comment.** `update_comment` had no condition, so a PATCH landing just after a soft delete wrote the body back. It is now conditioned on `deleted_at` being absent or NULL, and the handler re-reads on a cancelled transaction to return 404 (deleted) or 422 (image claimed elsewhere). A regression test was added (`persistence/tests/engagement.rs`).
- Reaction optimistic updates now `cancelQueries` before patching, so a stale in-flight refetch can't overwrite them.
- The `ApiStack` docstring misattributed which milestone added which Lambda.
- **Clippy 1.99's new `double_must_use` broke `-D warnings` in `lambda-image-process`.** This wasn't an M10 change; see Known gaps.

### Decisions (recorded in `03` §8/§9.4/§13, `09` §1.1/§1.5/§2.3/§2.4, `02` §2.10)
- **Engagement routes are nested under `questions/{questionId}`.** Comment and reaction rows are keyed on `(question, answerUser)`, and `responseId` alone has no index. The answer is resolved by matching `response_id` among that question's answers, and must be a published text answer, otherwise 404.
- One `CommentResponse` (`displayName`, not `authorDisplayName`) is used both inline and on the engagement routes. Soft-deleted comments are returned with `deletedAt` set, an empty body and a null image.
- **Comment images are claimed via `ImageMedia.attached_comment_id`,** set and cleared in the same transaction as the comment write.
- **PATCH semantics:** an absent field is left unchanged, and `imageMediaId: null` removes the image. Only the author can edit; admins get 403.
- Reaction PUT and DELETE return the updated `ReactionsResponse`.
- No cross-request cache for author names (see Known gaps).

### Tests
- Backend 361 (was 308):
  - `lambda-engagement` integration tests: comments 17, reactions 6
  - engagement unit tests: 11
  - contract-table rows for all seven routes
  - the media in-use test
  - the persistence race regression
- CDK 54 (was 53): the new engagement env test. The Apple-IdP removal renamed an existing test but added none.
- Frontend 163 (was 116):
  - `emoji` 14 and `mockEngagement` 11
  - `ReactionBar` 5 and `CommentList` 6
  - `client` +7 and `markdown` +4

---

## Handoff — starting M11 (Notifications)

For the next agent picking this up cold:

1. **Orient.** Read `CLAUDE.md`, then `12` M11, `07-notifications.md`, and `03` §10 (push routes) and §11a.2 (`POST /admin/dev/tick/notify`).
2. **Know the M11 scope.**
   - `lambda-push` and `lambda-notify-tick` are still stubs with binary `bootstrap`. Rename them (`push-api`, and `notify-tick` like `cycle-tick`) and copy the `lambda-engagement` layout.
   - VAPID keys are in Secrets Manager at `opennewsletter/vapid/dev` (B5).
   - The `NOTIFIED#…`/`TICK#NOTIFY` idempotency rows (`02`) land here.
   - Wire the dev-only `POST /admin/dev/tick/notify`.
3. **Frontend:** the service worker push handler, a subscribe/unsubscribe UI and per-group preferences (`PUT /push/preferences/{groupId}`).
4. **Verify** with the commands in `CLAUDE.md`. Baselines: 361 backend, 54 CDK, 163 frontend; ESLint 0 problems. Run `ruff` from the repo root (`ruff check infra`); from inside `infra/` its import sorting misfires. Put `infra/.venv/bin` first on `PATH` for `cdk synth`.
5. **Suggested split**, same as M10:
   - the lead writes the contract first
   - a Sonnet backend agent
   - a Haiku infra agent
   - a Sonnet frontend agent

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
- add an "Mn detail — what landed" section (backend/infra/tests/decisions), refresh the snapshot date, test counts and "Suggested next steps"
- rewrite the "Handoff" section for the *next* milestone
