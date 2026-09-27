# Progress & Blockers

Snapshot as of 2026-09-26. Working document — update as milestones complete or
blockers resolve. Authoritative milestone definitions live in
[`12-build-order.md`](12-build-order.md).

---

## Milestone status

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Repo skeleton | ✅ done | Layout, READMEs, `.gitignore`, `rust-toolchain.toml`, etc. landed in prior commits. |
| M1 | Operator prerequisites | 🟡 partial | `scripts/generate_vapid_keys.py` and `infra/keys/cf-signing.pub.pem` (dev placeholder) are committed. Google/Apple/Facebook OAuth apps, Cognito domain, DNS records, real signing keypair, and `cdk bootstrap` still owed by the operator (B5). |
| **M2** | **Backend foundations (domain + persistence)** | ✅ **done** | See "M2 detail" below. All 64 tests green (`cargo test -p persistence`), fmt + clippy clean. Small follow-ups from the 2026-08-09 plan reconciliation listed under "M2 follow-ups". |
| **M3** | **Infrastructure baseline (CDK)** | 🟡 **synth + tests verified, deploy unverified** | See "M3 detail" below. |
| **M3.5** | **Dev environment online** | 🟡 **code-complete, unverified** | See "M3.5 detail" below. |
| **M4** | **Auth + bootstrap** | 🟡 **code-complete, deploy unverified** | See "M4 detail" below. All 101 backend tests + 34 CDK tests green; fmt/clippy/ruff/mypy clean; `cdk synth` clean for dev + prod. The done-when gate (curl `GET /me` with a real Cognito token) needs an AWS account — blocked on B5. |
| **M5** | **Lifecycle engine + cycle CRUD** | 🟡 **code-complete, deploy unverified** | See "M5 detail" below. The manual done-when flow needs AWS (B5); an end-to-end DynamoDB Local test covers the same sequence. `shared/openapi.yaml` + contract test landed. |
| M6 | Responses + drafts | ⬜ | Drafts are last-write-wins — no version-conflict handling (`03-api-contract.md` §7.3). |
| M7 | Frontend skeleton + auth | ⬜ (existing mock UI predates real API) | Commit `91188ba` shipped a rich mock-only UI; will need rework against real endpoints in M7. |
| M8 | Media pipeline | ⬜ | Scope grew on 2026-08-09: avatar buckets + `/avatar/*` CloudFront behavior (`01` §5), size-cap enforcement in `lambda-image-process` (`08` §3.2), dev signed-URL query-param mode (`08` §4.5). |
| M9 | Newsletter UI | ⬜ | |
| M10 | Engagement | ⬜ | |
| M11 | Notifications | ⬜ | |
| M12 | Admin UI | ⬜ | |
| M13 | Hardening | ⬜ | |
| M14 | Production deploy | ⬜ | |

M0 and M2 are complete; M3's `cdk synth`/`pytest infra/tests/` are verified (a real cyclic-stack-dependency bug was found and fixed — see M3 detail), but the actual `cdk deploy` gate in `12-build-order.md` still requires an AWS account and is unverified. M3.5, M4 and M5 are code-complete and awaiting the same deploy step. Everything deploy-shaped is blocked on B5 (M1 operator tasks).

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
- **`opennewsletter/auth_stack.py`** — `AuthStack`: Cognito User Pool, 3 federated IdPs (Google/Apple/Facebook) via CFN dynamic references to Secrets Manager, `frontend` + `admin-bootstrap` app clients, Cognito-managed hosted UI domain. The PreSignUp (pass-through) trigger is wired in M4; there is no PostConfirmation trigger (plans reconciled 2026-08-09).
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

## Active blockers

### B4 — Pre-existing mock-only frontend will need replacement in M7

Commit `91188ba` shipped a rich UI built against in-memory mocks. M7's deliverables (Vite + Tailwind + React Router skeleton wired to the real API + Cognito) overlap heavily with what's already there; expect that work to be partial rewrite, not greenfield.

### B5 — M1 operator tasks outstanding

OAuth app registrations (Google / Apple / Facebook), Cognito hosted-UI domain, DNS records, the real CloudFront signing keypair, VAPID keys in Secrets Manager, and `cdk bootstrap` of the dev account are still owed by the human operator. These are non-blocking for code work but block the M3/M3.5/M4 deploy gates and any end-to-end auth verification. See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md).

### B6 — Lambda release builds (Windows/WSL2 only) + `cargo-lambda` not installed anywhere

**Windows/WSL2**: `cargo build --release` (and therefore `cargo lambda build --release`) aborts in `aws-lc-sys`, which refuses to compile under gcc 9 because of [gcc bug 95189](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189). That host is Ubuntu 20.04 with only gcc 9.4 and no clang. Debug builds and the test suite are unaffected. Fix: `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain; failing both, switch rustls to the `ring` backend.

**macOS**: the gcc issue doesn't apply (Apple clang). `cargo-lambda` is not installed because Homebrew refuses its third-party tap until the user trusts it — an operator decision: `brew tap cargo-lambda/cargo-lambda && brew trust cargo-lambda/cargo-lambda && brew install cargo-lambda`.

On both machines no Lambda artifact has ever been produced. `ApiStack`/`NotificationsStack` fall back to the shell stub when `backend/target/lambda/{binary}/bootstrap` is missing, which is why synth passes — deploying before this is cleared would ship stubs.

---

## Suggested next steps (in order)

1. **Complete M1 operator tasks** (OAuth apps, VAPID keys, DNS, real CloudFront keypair) to clear B5. See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md).
2. **Install `cargo-lambda`** (B6) on whichever machine will deploy.
3. **Bootstrap CDK** in the dev AWS account:
   ```bash
   source infra/.venv/bin/activate && npx aws-cdk@2 bootstrap aws://ACCOUNT_ID/us-east-1
   ```
4. **Deploy and verify M3–M5**: `make deploy-dev && make seed && make fe` (see `docs/RUNBOOK.md`). M4 gate: `bootstrap_admin.py` then `curl -H "Authorization: Bearer $TOKEN" "$API_ENDPOINT/me"`. M5 gate: the five-step manual flow in `12` M5 using `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy`.
5. **Begin M6** (Responses + drafts): `lambda-responses` per `03` §7, last-write-wins drafts. Standing rule from M5: every new/changed route updates `shared/openapi.yaml` + `ROUTE_TABLE` in `domain/tests/openapi_contract.rs` in the same PR, then re-run `scripts/codegen_types.sh`.

Workstation setup for either machine: [`13-dev-environments.md`](13-dev-environments.md) §0.

### Known gaps carried forward

- **`GET /healthz` returns only `{"status":"ok"}`** — `03` documents `version`/`buildAt` too. The YAML and `HealthResponse` describe what ships today.
- **Problem responses never populate `fieldErrors`** — `shared::http::problem_response` has a fixed key set; `ProblemDetails.field_errors` exists in `domain::api` per `03` §1.1 but nothing fills it. `VOTE_CAP_REACHED` puts its already-voted list in `detail` for the same reason.
- **`02-data-model-dynamodb.md` documents attributes as camelCase; the code writes snake_case** (carried from M4).
- **`ruff format` has never been applied repo-wide** — would reformat 4 infra files untouched since M3. `ruff check` is clean.
- **`GET /config`'s membership list does one `GetItem` per group.** Switch to the `BatchGetItem` helper if group counts grow.
- **`lambda-cycle-tick` needs `#![recursion_limit = "256"]`** since it gained a `lib.rs`; harmless, but if it creeps up again, box the dispatch futures.
- **`POST /admin/dev/tick/notify`** is not wired — it arrives with `lambda-notify-tick` in M11.

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
