# Progress log, M0–M13.5 (archived)

History moved out of [`../PROGRESS.md`](../PROGRESS.md) on 2026-10-11 so the live file stays short. Everything below is kept verbatim from PROGRESS.md as of that date. Sections are in chronological order. Section links such as "see M13 detail" and "Known gaps" refer to headings in this file or to the live PROGRESS.md.

The current design is in [`docs/`](../../docs/README.md), which grew out of the plan docs this log talks about; where this log and the docs disagree, the docs win.

*Paths updated 2026-10-12: the numbered plan docs moved to [`docs/`](../../docs/README.md) under descriptive names (e.g. `plans/06-newsletter-lifecycle.md` is now [`docs/design/lifecycle.md`](../../docs/design/lifecycle.md)), and the links below were rewritten to match. Section numbers are unchanged, so every § reference still points at the same content. The text is otherwise as written.* Use this file for *why* something is the way it is: what each milestone landed, the bugs found in review, and the decisions taken along the way.

Contents: M2 · M3 · M3.5 · M4 · Plan-set reconciliation (2026-08-09) · M5 · M6 · M7 · Post-M7 gap pass · M8 · M9 · M10 · M11 · M12 · M13 · Plan review, bug hunt and code reviews (2026-10-07 → 10-11) · M13.5 · B5/B6 blocker history · Known gaps as of 2026-10-11 · Design review (2026-10-08, landed 10-12)

---

## M2 detail — what landed

All paths relative to `backend/`.

- **Workspace** — `Cargo.toml` wires all 14 crates with pinned workspace dependencies.
- **Lambda stubs** — every `crates/lambda-*` has a minimal `Cargo.toml` + `src/main.rs` so the workspace builds. Real handlers land in M4+.
- **`shared` crate** — [`config.rs`](../../backend/crates/shared/src/config.rs): every tunable named in the plans (cycle sizes, vote caps, image/comment limits, invite TTL, vote-count pad width) in one file.
- **`domain` crate**
  - [`ids.rs`](../../backend/crates/domain/src/ids.rs) — `UserId`, `GroupId`, `CycleId`, `QuestionId`, `ResponseId`, `CommentId`, `ImageId`, `AvatarId`, `PollOptionId`, `SubscriptionId`, `CognitoSub`, `InviteCode` as newtypes. Implement `Display`, `FromStr`, `AsRef<str>`, serde transparent. `UserId::generate()` etc. mint UUIDv7.
  - [`entities.rs`](../../backend/crates/domain/src/entities.rs) — every entity from [`data-model.md` §2](../../docs/design/data-model.md#2-entity-catalog): `User`, `CognitoSubLookup`, `GroupMembership`, `Group` (+ `CycleSettings`, `NotificationSettings`), `Invite`, `Newsletter`, `CandidateQuestion`, `CandidateVote`, `LockedQuestion`, `PollOption`, `Response`, `ImageMedia`, `AvatarMedia`, `Comment`, `Reaction`, `PushSubscription`, `NotificationPref`. Enums: `Role`, `NewsletterStatus`, `ResponseStatus`, `InviteStatus`, `MediaStatus`, `QuestionKind`, `ImageMimeType`.
  - [`error.rs`](../../backend/crates/domain/src/error.rs) — `ApiError` + `ApiErrorCode` with HTTP-status mapping mirroring [`api.md` §1.1](../../docs/design/api.md#11-error-format).
- **`persistence` crate**
  - [`keys.rs`](../../backend/crates/persistence/src/keys.rs) — single source of truth for every `pk` / `sk` / `gsi1pk` / `gsi1sk` / `gsi2pk` / `gsi2sk` shape. Centralised `attr::*` and `index::*` constants. **Comprehensive unit tests** covering padding, lexicographic ordering, and every entity family.
  - [`repo.rs`](../../backend/crates/persistence/src/repo.rs) — `Repo { client, table }` handle.
  - [`error.rs`](../../backend/crates/persistence/src/error.rs) — `RepoError` enum, with `From` impls for `aws_sdk_dynamodb::error::SdkError` and `serde_dynamo::Error`.
  - Per-entity-family modules with the access-pattern functions named after the AP they serve:
    - [`users.rs`](../../backend/crates/persistence/src/users.rs) — AP1, Cognito-sub lookup, profile upsert
    - [`groups.rs`](../../backend/crates/persistence/src/groups.rs) — AP2, AP3, AP4 + transaction §4 #6 (leave group)
    - [`invites.rs`](../../backend/crates/persistence/src/invites.rs) — AP5, AP6, revoke + transaction §4 #5 (join via invite)
    - [`newsletters.rs`](../../backend/crates/persistence/src/newsletters.rs) — AP7, AP8, AP9 + `write_status_transition` (the GSI2-consistent writer required by §9)
    - [`questions.rs`](../../backend/crates/persistence/src/questions.rs) — AP10, AP11, AP12, AP13 + transactions §4 #1 (cast vote), #2 (withdraw vote), #3 (promote candidates)
    - [`responses.rs`](../../backend/crates/persistence/src/responses.rs) — AP14, AP15, AP16, last-write-wins `save_draft`, transaction §4 #4 (publish response with first-publish editions_answered bump)
    - [`engagement.rs`](../../backend/crates/persistence/src/engagement.rs) — AP18, AP19, comment soft-delete, reaction toggle
    - [`media.rs`](../../backend/crates/persistence/src/media.rs) — AP17, image + avatar read/write
    - [`push.rs`](../../backend/crates/persistence/src/push.rs) — AP20, AP21, subscribe/unsubscribe
  - [`test_factories.rs`](../../backend/crates/persistence/src/test_factories.rs) behind the `test-utils` feature.

### M2 coverage vs. plan

- ✅ Every access pattern AP1–AP21 has a named function.
- ✅ Every transaction in [`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants) (#1–#6) implemented with `TransactWriteItems`.
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
- **`opennewsletter/frontend_stack.py`** — `FrontendStack`: ACM cert (us-east-1) covering `domain` and `api_domain`; optional Route53 CNAME if `hosted_zone_id` is set.
- **`opennewsletter/media_persistent_stack.py`** — `MediaPersistentStack`: S3 originals + processed buckets (both private, Block-Public-Access all-on).
- **`opennewsletter/media_pipeline_stack.py`** — `MediaPipelineStack`: `lambda-image-process` (shell stub, arm64, 1024 MB, `provided.al2023`), S3 `ObjectCreated` notification on `uploads/` prefix, least-privilege IAM.
- **`opennewsletter/monitoring_stack.py`** — `MonitoringStack` skeleton: SNS alarm topic (+ email subscription if `alarm_email` set), empty CloudWatch dashboard, AWS Budgets alarm ($10/mo). Metric widgets + alarms land in M13.
- **`backend/lambda-stubs/lambda-image-process/bootstrap`** — shell stub so CDK asset hashing works at synth time; replaced by the real Rust binary in M8.
- **`infra/tests/test_stacks.py`** — 21 CDK assertion tests covering DDB keys/TTL/GSIs, Cognito user pool settings (2 clients, 3 IdPs, no self-signup), S3 Block-Public-Access, Lambda arm64/1024 MB.

### M3 coverage vs. plan

- ✅ `DataStack` — DynamoDB table + 2 GSIs.
- ✅ `AuthStack` — Cognito user pool + 3 IdPs + 2 app clients + hosted UI domain.
- ✅ `MediaPersistentStack` — S3 originals + processed buckets.
- ✅ `MediaPipelineStack` — `lambda-image-process` stub + S3 event subscription.
- ✅ `FrontendStack` — ACM cert + optional Route53 records.
- ✅ `MonitoringStack` skeleton + AWS Budgets alarm ($10/mo).
- ✅ `infra/tests/test_stacks.py` — 21 assertion tests, all green.
- ✅ **`cdk synth --context env=dev` and `--context env=prod` both verified** (2026-07-14). `infra/.venv` recreated from `requirements.txt` (it's gitignored, not committed); `cdk` CLI run via `npx aws-cdk@2` since it isn't installed globally.
- ⚠️ **Venv gotcha**: the system Python is 3.8 and typeguard 4.x breaks jsii's runtime type-checks on it (`check_type() got an unexpected keyword argument 'argname'` — 6 AuthStack tests error at fixture setup). `typeguard~=2.13.3` is now pinned in `requirements.txt`; if a venv predates the pin, `pip install "typeguard~=2.13.3"` fixes it. (A stray duplicate `infra/venv/` existed with the broken 4.2.1 and has been fixed in place; `infra/.venv` is the canonical one.)
- 🟡 **`cdk deploy` still unverified** — needs an AWS account + `cdk bootstrap`. Blocked on B5 (M1 operator tasks) for a full end-to-end check; see "Suggested next steps."

**Bug found + fixed during verification**: `MediaPersistentStack` and `MediaPipelineStack` had a real circular dependency, not just an ordering issue. `MediaPipelineStack.add_event_notification()` was called directly on the `originals_bucket` object passed in from `MediaPersistentStack`; CDK attaches the `BucketNotifications` custom resource (and its singleton handler Lambda) to the *bucket's own* stack, so that resource ended up needing `MediaPipelineStack`'s Lambda ARN from `MediaPersistentStack` — while the Lambda's IAM grants (`grant_read`, `grant_put`) already needed `MediaPersistentStack`'s bucket ARNs the other way. `cdk synth` failed with `RuntimeError: ... would create a cyclic reference`.

  Fix ([`media_pipeline_stack.py`](../../infra/opennewsletter/media_pipeline_stack.py)): inside `MediaPipelineStack`, re-import the originals bucket via `s3.Bucket.from_bucket_attributes(self, ..., bucket_arn=originals_bucket.bucket_arn)` and call `add_event_notification` on that imported reference instead of the real bucket object. This scopes the notification custom resource (and its singleton handler) to `MediaPipelineStack` — matching the stated design intent that redeploying the volatile pipeline stack should never require touching the stable persistent stack. Direct S3→Lambda notifications (not EventBridge) were kept per [`media.md`](../../docs/design/media.md)'s explicit "no batching configurable on direct S3→Lambda notifications" requirement.

  This added a second `AWS::Lambda::Function` (CDK's own `BucketNotificationsHandler` singleton) to `MediaPipelineStack`'s template, so `test_image_process_lambda_exists` in `infra/tests/test_stacks.py` was updated to match on `FunctionName` rather than asserting a raw count of 1.

**Other cleanup during verification** (pre-existing issues, not introduced by the above): `ruff check .` found 3 unused imports (`aws_route53_targets` in `frontend_stack.py`, `aws_iam` in `media_pipeline_stack.py`, `MonitoringStack` in `test_stacks.py`) — auto-fixed. `mypy` found 43 errors, all from the same pattern: every stack's `**kwargs: object` doesn't satisfy `cdk.Stack.__init__`'s keyword-only parameter types; changed to `**kwargs: Any` across all 6 stack files. Also fixed a real `Optional[str]`-vs-`str` type error in `config.py`'s `get()` helper. `ruff` and `mypy` are both clean now.

---

## M3.5 detail — what landed

- **[`Makefile`](../../Makefile)** — six targets at repo root:
  - `make deploy-dev` — `cdk deploy --all --outputs-file cdk.out/dev-outputs.json` + writes `frontend/.env.dev`
  - `make seed` — runs `scripts/seed_dev_data.py --env dev`
  - `make redeploy-volatile` — destroy + redeploy `DataStack-dev`, `MediaPipelineStack-dev`, `MonitoringStack-dev`
  - `make reset-all` — interactive prompt then `cdk destroy --all --force` (warns about Cognito loss)
  - `make deploy-lambda LAMBDA=<name>` — `cargo lambda build --release --arm64` + `aws lambda update-function-code` (~5s path)
  - `make fe` — `cd frontend && npm run dev -- --mode dev`
- **[`scripts/seed_dev_data.py`](../../scripts/seed_dev_data.py)** — idempotent + destructive:
  - Reads `infra/cdk.out/dev-outputs.json` for table name, bucket names, user pool ID.
  - Scan + batch-deletes all DynamoDB items; deletes all S3 objects from originals + processed buckets.
  - Creates or finds the bootstrap admin Cognito user (idempotent; skips creation if already exists).
  - Writes 5 DynamoDB fixture items: CognitoSubLookup, User, Group, GroupMembership (admin), Newsletter (voting).
  - IDs are derived deterministically from the Cognito sub so re-seeding gives the same user/group IDs.
  - Accepts `--cycle-close-in DUR` (e.g. `5m`, `2h`, `4d`) to set the response-window deadline relative to now.
  - Prints sign-in credentials and the `admin-initiate-auth` CLI command for inner-loop testing.
- **[`scripts/write_frontend_env.py`](../../scripts/write_frontend_env.py)** — reads `infra/cdk.out/{env}-outputs.json`, writes `frontend/.env.{env}` with all `VITE_*` vars. `VITE_API_BASE_URL` falls back to a TODO placeholder until `ApiStack` lands in M4.
- **Tick Lambda stubs upgraded** — `lambda-cycle-tick` and `lambda-notify-tick` replaced `fn main() {}` with proper `lambda_runtime = "1"` handlers (accept any event, return stub JSON). Added `lambda_runtime = "1"` to workspace deps. `cargo check --workspace` clean.
- **AWS Budgets alarm** (in `MonitoringStack`) — `CfnBudget` at $10/mo; alerts at 80% actual and 100% forecasted to `config.alarm_email`.

### M3.5 coverage vs. plan

- ✅ `Makefile` — all 6 targets per [`environments.md` §8](../../docs/development/environments.md#8-fast-lambda-iteration).
- ✅ `scripts/seed_dev_data.py` — idempotent, destructive, `--cycle-close-in` supported.
- ✅ `scripts/write_frontend_env.py` — writes `frontend/.env.{env}` from CDK outputs.
- ✅ Dev removal-policy overrides — already applied in M3 (DynamoDB DESTROY, S3 auto_delete, log groups DESTROY).
- ✅ `MediaStack` split — already done in M3 (`MediaPersistentStack` + `MediaPipelineStack`).
- ✅ Tick Lambda stubs — `lambda-cycle-tick` and `lambda-notify-tick` are proper `lambda_runtime` binaries.
- ✅ AWS Budgets alarm — $10/mo in `MonitoringStack`.
- 🟡 **`POST /admin/dev/tick/{cycle|notify}` HTTP routes** — Lambda code ready; API Gateway routes wired in M5 when `ApiStack` is created.
- 🟡 **End-to-end verification blocked** — `infra/.venv` is ready; blocked on M1 operator tasks + CDK bootstrap + AWS account for `cdk deploy`. See [`runbook.md`](../../docs/operations/runbook.md).

---

## M4 detail — what landed

### Backend

- **`shared` crate** — grew from config-only into the handler-boundary crate the repo layout always described:
  - [`http.rs`](../../backend/crates/shared/src/http.rs) — `AuthClaims` extraction from the API Gateway JWT authorizer, correlation-ID echo/mint, JSON body parsing, and the RFC-7807 problem response from [`api.md` §1.1](../../docs/design/api.md#11-error-format). 4 unit tests.
  - [`telemetry.rs`](../../backend/crates/shared/src/telemetry.rs) — JSON `tracing` subscriber for Lambda entry points.
  - [`config.rs`](../../backend/crates/shared/src/config.rs) — the three M2 follow-ups plus `INVITE_MAX_TTL_DAYS`, cycle/window bounds, and `derive_avatar_color()`.
- **`domain`** — `ApiErrorCode` gained `as_str()`, `slug()`, `title()` (the wire fields of the problem body); `RESPONSE_VERSION_CONFLICT` removed; `ImagePurpose` added and threaded onto `ImageMedia`.
- **`persistence`**
  - [`auth.rs`](../../backend/crates/persistence/src/auth.rs) — `resolve_user_id`, `require_user_id`, and `require_membership` (the tenant-isolation gate from [`auth.md` §11](../../docs/design/auth.md#11-tenant-authorization-helper)), each memoized in a `DashMap` for `MEMBERSHIP_CACHE_TTL_SECONDS`, plus explicit invalidation on role change / removal.
  - [`expr.rs`](../../backend/crates/persistence/src/expr.rs) — `set_fields()`, which binds every patch attribute to a `#f{n}` placeholder. Added after `timezone` (and then `name`) turned out to be DynamoDB reserved words; aliasing everything removes the whole class of bug rather than the two instances found.
  - `join_via_invite_tx` now takes an optional new `User` and writes the `CognitoSubLookup` + `User` rows inside the same transaction (first-ever redemption, [`auth.md` §4.2](../../docs/design/auth.md#42-algorithm)).
  - New: `groups::update_group` (targeted patch, so a concurrent join can't have `member_count` clobbered), `groups::update_membership_role`, `users::update_profile`, `users::get_users_batch` (one `BatchGetItem` for a group's member list).
  - `invites::revoke` is now conditional on `status = pending`, so revoking a consumed invite fails instead of erasing the audit trail.
- **`lambda-groups`** (binary `groups-api`) — `GET /healthz`, `GET /config`, `GET|PATCH /me`, `GET|PATCH /groups/{g}`, `DELETE|PATCH /groups/{g}/members/{u}`. Includes the last-admin guard on both removal and demotion, and full [`api.md` §4.3](../../docs/design/api.md#43-patch-groupsgroupid) validation (IANA timezone via `chrono-tz`, gradient/avatarColor slugs, merged-then-validated cycle settings). 10 unit tests.
- **`lambda-invites`** (binaries `invites-api` + `invites-presignup`) — `POST /admin/invites`, `GET /admin/groups/{g}/invites`, `POST /admin/invites/{code}/revoke`, `POST /invites/redeem`, plus the pass-through `PreSignUp` trigger. 12 unit tests.

### Infrastructure

- **[`api_stack.py`](../../infra/opennewsletter/api_stack.py)** — HTTP API, CORS, Cognito JWT authorizer, 50/25 rps throttling, JSON access logs, the two handler Lambdas (arm64, `provided.al2023`, 256 MB, 10 s), and all 13 M4 routes. `_ROUTES` is the single list to extend as later milestones add handlers. Custom domain + API mapping in prod only.
- **[`lambda_assets.py`](../../infra/opennewsletter/lambda_assets.py)** — resolves `cargo lambda build` output, falling back to the shell stub so synth works on a machine that has never run the Rust build.
- **`AuthStack`** — now owns the `PreSignUp` Lambda and its trigger, and no longer declares the `pendingInvite` custom attribute (the invite code rides in the OIDC `state` per [`auth.md` §1](../../docs/design/auth.md#1-cognito-user-pool-configuration-recap)).
- **[`bootstrap_admin.py`](../../scripts/bootstrap_admin.py)** — creates the Cognito user, adds it to an informational `admins` group, and writes User + sub-lookup + Group + admin membership in one `TransactWriteItems`. IDs are UUID5-derived from the Cognito sub, so re-running is idempotent.

### Tooling caught up to M4

Three files still assumed ApiStack didn't exist; all now fixed:

- **`Makefile`** — `deploy-dev` never built the Rust binaries. Now that `ApiStack` exists and silently substitutes the shell stub for a missing binary, that would have deployed stubs that answer every route with nothing. Added a `build-lambdas` target and made `deploy-dev` depend on it.
- **`Makefile`** — `deploy-lambda LAMBDA=<name>` assumed the cargo binary name equals the CloudFormation function name. It doesn't any more (`groups-api` → `OpenNewsletter-Groups-dev`), so a `FUNCTION_<binary>` map was added. **Add a line to it whenever a new handler lands.**
- **`write_frontend_env.py`** — dropped the `TODO-wire-in-M4` placeholder for `VITE_API_BASE_URL`; it now reads `ApiStack`'s `ApiEndpoint` output and warns when absent.

Docs corrected alongside: [`overview.md` §4](../../docs/overview.md#4-repo-layout) repo layout (the media-stack split and `lambda_assets.py`), [`testing-and-ci.md` §3](../../docs/development/testing-and-ci.md#3-cdk-tests) (the "exactly 8 Lambdas in `ApiStack`" assertion is wrong until M11, and `PreSignUp` isn't in that stack at all), [`environments.md` §8](../../docs/development/environments.md#8-fast-lambda-iteration) (binary-vs-function naming), and [`runbook.md`](../../docs/operations/runbook.md) §M4 (verify a real binary exists before deploying).

### M4 coverage vs. plan

- ✅ All 13 routes from [`api.md` §13](../../docs/design/api.md#13-per-route-authrole-summary-table) wired, each behind the JWT authorizer (asserted by a test that walks every synthesized route).
- ✅ `PreSignUp` pass-through; no `PostConfirmation` trigger.
- ✅ `admin-bootstrap` Cognito app client — already existed from M3.
- ✅ **101 backend tests** (up from 64) and **34 CDK tests** (up from 21), all green. `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `ruff check`, `mypy --strict`, and `cdk synth` for dev + prod all clean.
- 🟡 **Done-when gate unverified** — needs a deployed stack and a real Cognito token (B5).

### Decisions and deviations worth knowing

- **The bearer token is the ID token, not the access token.** [`api.md` §1](../../docs/design/api.md#1-general-conventions) and [`auth.md` §4.1](../../docs/design/auth.md#41-inputs) contradicted each other; [`auth.md` §4.2](../../docs/design/auth.md#42-algorithm)'s algorithm reads `jwt.email`/`jwt.name`, which only the ID token carries. Both docs now say ID token, and `POST /invites/redeem` returns `VALIDATION_FAILED` if the `email` claim is absent.
- **Invite expiry is enforced in Rust, not in the transaction's condition expression.** `expires_at` is stored as RFC-3339 with a variable-width fractional part, so a lexicographic `<` against "now" is not reliably ordered at sub-second resolution. The condition keeps the `status = pending` guard (which is what actually prevents double-redemption); the expiry race it leaves open is microseconds wide.
- **DynamoDB attribute names are snake_case, but [`data-model.md`](../../docs/design/data-model.md) documents them as camelCase.** The snake_case naming is baked into M2's writers and its 73 tests. The HTTP contract is camelCase as specified, so the handlers map entities → DTOs explicitly. Worth reconciling [`data-model.md`](../../docs/design/data-model.md) to match the code at some point; not worth churning M2 for.
- **`GET /config` returns `userId: null` with an empty `memberships` array** when the JWT is valid but no `User` row exists — the pre-onboarding state [`auth.md` §6](../../docs/design/auth.md#6-jwt-verification-on-the-backend) calls for.
- **Two binaries in `lambda-invites`** required naming them (`invites-api`, `invites-presignup`) rather than the usual `bootstrap`; `lambda-groups`' binary is `groups-api` for symmetry. Other Lambda crates still use `bootstrap`.
- **Fixed in passing**: `seed_dev_data.py` wrote `avatar_color: "#4A90D9"` and a raw CSS `gradient`, neither of which is a valid slug under the reconciled [`api.md` §2.3](../../docs/design/api.md#23-patch-me)/§4.3 enums — both would have failed validation on the first `PATCH`.

---

## Plan-set reconciliation (2026-08-09)

A full consistency pass was made over plan docs `00`–`13` (now in `docs/`) so the documents no longer contradict each other. If you last read the plans before this date, re-read the touched sections. Headlines:

- **Invite flow**: `PreSignUp` is a pure pass-through; there is no `PostConfirmation` trigger; `POST /invites/redeem` is the single consumption point for first signup AND additional groups. Stale contrary text removed from [`infrastructure.md` §4.1](../../docs/design/infrastructure.md#41-cognito-user-pool), [`api.md` §3.4](../../docs/design/api.md#34-post-invitesredeem), [`frontend.md` §6](../../docs/design/frontend.md#6-auth-in-the-spa)/§7.8, and the [`auth.md` §2](../../docs/design/auth.md#2-end-to-end-first-signup-flow) diagram.
- **Drafts**: last-write-wins everywhere. `RESPONSE_VERSION_CONFLICT` removed from the [`api.md`](../../docs/design/api.md) error catalog, [`build-order.md`](../build-order.md) M6 deliverables, and [`testing-and-ci.md`](../../docs/development/testing-and-ci.md) test lists; the phantom `version` increment removed from [`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants).
- **Avatar pipeline**: buckets and `lambda-image-process` wiring added to [`infrastructure.md` §5](../../docs/design/infrastructure.md#5-mediapersistentstack-and-mediapipelinestack) (handler branches on source **bucket**, not key prefix).
- **Comment images**: uploaded via `POST /uploads` with `purpose: "comment"` (valid while the cycle is `published`); `ImageMedia.purpose` added to [`data-model.md` §2.10](../../docs/design/data-model.md#210-imagemedia).
- **Upload size cap**: enforced by `lambda-image-process` (mark `failed` + delete original) — `s3:content-length-range` doesn't exist for presigned PUT ([`media.md` §3.2](../../docs/design/media.md#32-presign)).
- **Reminder offsets**: capped at 168h (`MAX_REMINDER_OFFSET_HOURS`) so the notify-tick query window always covers them ([`api.md` §4.3](../../docs/design/api.md#43-patch-groupsgroupid), [`notifications.md` §7.1](../../docs/design/notifications.md#71-algorithm)).
- Smaller: error catalog gained `LAST_ADMIN`/`CANDIDATE_PROMOTED`; gradient + avatarColor slug sets enumerated in [`api.md` §4.3](../../docs/design/api.md#43-patch-groupsgroupid)/§2.3; emoji predicate unified on [`engagement.md` §2.2](../../docs/design/engagement.md#22-validation); VAPID keys consumed via `from_base64`, no PEM ([`notifications.md` §2](../../docs/design/notifications.md#2-vapid-keys)/§10); `lambda-push` direct-invoke envelope specified ([`notifications.md` §6](../../docs/design/notifications.md#6-cycle-open-fan-out)); `ApiStack`/`NotificationsStack` creation assigned to M4/M5 in [`build-order.md`](../build-order.md); CDK-test Lambda count corrected to 8 in [`testing-and-ci.md` §3](../../docs/development/testing-and-ci.md#3-cdk-tests); `backend-ci` uses `cargo test --features integration` (no manual DDB-local step; *superseded M13: the flag was never built, and CI runs plain `cargo test --workspace`*); dev uses raw AWS endpoints in [`overview.md` §7](../../docs/overview.md#7-environments) and [`api.md` §1](../../docs/design/api.md#1-general-conventions); `voteWindowOpenAt` is informational-only ([`lifecycle.md` §4.2](../../docs/design/lifecycle.md#42-default-schedule-per-group)); dev tick route body takes `groupId` ([`api.md` §11a.1](../../docs/design/api.md#11a1-post-admindevtickcycle)); `sub` vs `userId` confusion fixed in code sketches ([`data-model.md` §6](../../docs/design/data-model.md#6-tenant-isolation-rule), [`media.md` §4.3](../../docs/design/media.md#43-the-media-urls-crate), [`engagement.md` §1.4](../../docs/design/engagement.md#14-endpoints)/§4.1); CSP concretely specified as a meta tag ([`frontend.md` §12.1](../../docs/design/frontend.md#121-content-security-policy)).

---

## M5 detail — what landed (2026-09-26, first work on the macOS machine)

### Backend

- **`shared/src/cycle_time.rs`** — `first_day_of_next_month_local`, `cycle_id_for`, `next_cycle_schedule` ([`lifecycle.md` §4](../../docs/design/lifecycle.md#4-cycle-creation-and-the-calendar)/§5.4/§5.5). Ambiguous local midnight (DST fall-back) resolves to the earliest instant; a nonexistent midnight walks forward hour by hour. Tests cover December rollover, the [`lifecycle.md` §11](../../docs/design/lifecycle.md#11-tests) #8 New York spring-forward case, a Southern-hemisphere zone and a fixed-offset zone.
- **`persistence`**
  - `newsletters.rs` — `find_voting_cycle`, `find_open_cycle`, `list_recent` (base64url cursor), `create_next_voting_cycle` (put-if-not-exists), `publish_cycle`, `upsert_tick_sentinel`, `rewind_active_cycle_deadline` (dev tick; picks `open` if present, else `voting`).
  - `error.rs` — `RepoError::is_lost_race()`: condition-check / transaction-cancelled failures are now distinguishable, so the tick logs lost races at INFO and moves on ([`lifecycle.md` §8](../../docs/design/lifecycle.md#8-race-conditions-and-idempotency)).
  - **Key-timestamp normalization** — `keys::key_timestamp()` is now the only way a timestamp enters a key attribute (fixed-width `YYYY-MM-DDTHH:MM:SSZ`). Rust writers had been using `to_rfc3339()` (`+00:00`, variable fractional part) while the Python scripts and sentinel used `Z`, so the tick's lexicographic `gsi2sk <= now` comparison was only right by luck at sub-second edges. `newsletter_gsi2sk`, `image_gsi1sk` and `comment_sk` now take `DateTime<Utc>` so a raw string can't be passed. Documented in [`data-model.md` §1](../../docs/design/data-model.md#1-table-shape-recap).
- **`lambda-cycle-tick`** (binary `cycle-tick`) — one binary serves the EventBridge schedule and `POST /admin/dev/tick/cycle` (branches on `requestContext.http`; plain `lambda_runtime`, because `lambda_http` can't deserialize a scheduled-event payload). Dev route: 404 unless `ENV=dev`, admin checks per [`api.md` §11a.1](../../docs/design/api.md#11a1-post-admindevtickcycle), returns `{ "transitions": [{ groupId, cycleId, from, to }] }` (`from: null` = newly created cycle). Notification fan-out is a no-op hook in `notify.rs` until M11. Has a `lib.rs` so tests can call `tick::run_tick`.
- **`lambda-newsletters`** (binary `newsletters-api`) — `GET /groups/{g}/newsletters` (paginated, `myDraftCount`/`myPublishedCount`) and `GET /groups/{g}/newsletters/{c}` for all four statuses (`archived` → 410 `NEWSLETTER_ARCHIVED`, a new error code).
- **`lambda-questions`** (binary `questions-api`) — candidate list/create, vote cast/withdraw (idempotent, vote-cap enforced), admin `DELETE /admin/groups/{g}/candidate-questions/{q}` with vote cascade and `CANDIDATE_PROMOTED`.
- **`lambda-groups`** — `PATCH /groups/{g}` reschedules the current `voting` cycle when `responseWindowDays`/`timezone` change and the cycle has no candidates ([`lifecycle.md` §6](../../docs/design/lifecycle.md#6-settings-changes-mid-cycle); if the recomputed `cycleId` would differ, the row is left alone — documented there).
- **Scripts** — `bootstrap_admin.py` writes the group's first `voting` Newsletter in the same transaction as the group (Python schedule math verified against the Rust cases). `seed_dev_data.py` had been writing a UUID-hash `cycle_id` instead of `yyyymm` and was missing `published_at`/`next_transition_at`; fixed.

### OpenAPI contract (deferred from M4)

- **`shared/openapi.yaml`** — OpenAPI 3.1 for every route through M5 (M4's 13 + M5's 7). `redocly lint`: 0 errors, 2 intentional warnings (no `info.license`, placeholder server URLs).
- **`domain/src/api.rs`** — all wire DTOs, moved out of the per-crate `dto.rs` files (deleted).
- **`domain/tests/openapi_contract.rs`** — round-trips every YAML example through its `domain::api` type via `ROUTE_TABLE`; a YAML example without a table row, or vice versa, fails. Also asserts the `gradient`/`avatarColor` enums match `shared/src/config.rs`.
- **`scripts/codegen_types.sh`** → `frontend/src/types/api.ts` (generated, committed).
- **Real bug fixed in passing**: `GroupResponse` embedded the `CycleSettings`/`NotificationSettings` *entities*, which serialize snake_case, so `GET|PATCH /groups/{g}` returned snake_case settings inside a camelCase body. Now mapped through camelCase wire types.

### Infrastructure

- **`NotificationsStack`** (new) — cycle-tick Lambda (512 MB / 60 s) on a `rate(5 minutes)` EventBridge rule. `lambda-notify-tick` joins in M11.
- **`ApiStack`** — `Newsletters` + `Questions` Lambdas and the 7 M5 routes; `POST /admin/dev/tick/cycle` → cycle-tick, **dev only** (asserted absent from prod). `ApiStack` depends on `NotificationsStack`, never the reverse; [`infrastructure.md` §1](../../docs/design/infrastructure.md#1-stack-inventory)'s dependency table was backwards and is corrected.

### M5 coverage vs. plan

- ✅ All deliverables in [`build-order.md`](../build-order.md) M5, including the [`lifecycle.md` §11](../../docs/design/lifecycle.md#11-tests) lifecycle tests (items 1–5, 7, 8 in `persistence/tests/lifecycle.rs`; item 6 is M4's `invites.rs` member-cap test).
- ✅ **End-to-end lifecycle test** (`lambda-questions/tests/lifecycle.rs`) — candidates + votes through the real handlers → rewind → tick → `open` with top-N locked → rewind → tick → `published` → next `voting` cycle exists. Stands in for the manual done-when flow until AWS is available.
- ✅ Verified on macOS: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -D warnings`, `cargo test --workspace --all-features` — **167 tests, 0 failures** (DynamoDB Local via Colima), `pytest infra/tests` (40), `cdk synth` dev + prod.
- 🟡 **Done-when manual flow unverified** — needs a deployed stack (B5).

### Decisions worth knowing

- Admin candidate delete has no `cycleId` in its path: it checks the current `voting` cycle, then the current `open` cycle's locked questions (→ `CANDIDATE_PROMOTED`). There's no "voters of question X" access pattern, so the vote cascade deletes per group member (bounded by `memberSoftCap`). Both documented in [`api.md` §6.5](../../docs/design/api.md#65-admin-moderation-routes).
- `GET /candidate-questions` defaults to `sort=top` and pages in memory (small groups); the `open`-status newsletter shape was unspecified and is now documented in [`api.md` §5](../../docs/design/api.md#5-newsletter-routes).
- Test pollution: `persistence::auth`'s membership cache is process-wide, so integration tests must use unique group IDs or one test's cached role leaks into another.

---

## M6 detail — what landed (2026-09-27)

- **`lambda-responses`** (binary `responses-api`) — `PUT …/questions/{q}/my-response` per [`api.md` §7](../../docs/design/api.md#7-response-routes-drafts--publish). Mirrors the `lambda-questions` layout (lib/router/handlers/state/validation).
- **`persistence::responses::save_response`** replaces `save_draft`/`publish_response_tx`. Every save — draft or publish — is one `TransactWriteItems`: Update the response row + ConditionCheck on the Newsletter (`status = open AND response_close_at > now`) + the `editions_answered` bump on the caller's first publish in the cycle. A lost race maps to 409 `CYCLE_NOT_OPEN`. `persistence::questions::get_locked_question` added.
- **Decisions (documented in [`api.md` §7.3](../../docs/design/api.md#73-put-groupsgroupidnewsletterscycleidquestionsquestionidmy-response) and [`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants) #4):**
  - Publishing is sticky: `publish=false` after a publish updates content but stays `published`, with the original `publishedAt`.
  - The deadline is enforced by `responseCloseAt` as well as by status, so the ≤5-minute window before the tick runs is closed.
  - `responseId` is stable via `if_not_exists`.
  - Kind mismatch → `VALIDATION_FAILED`.
  - Image IDs are de-duplicated before the ≤10 check.
  - Saves don't modify `ImageMedia` rows (question linkage is M8).
- **OpenAPI** — the route + `ResponseDto`/`SaveResponseRequest` in `shared/openapi.yaml`, `ROUTE_TABLE` rows, `frontend/src/types/api.ts` regenerated.
- **Infra** — `Responses` Lambda (512 MB) + its JWT route in `ApiStack`, `FUNCTION_responses-api` in the Makefile, CDK test.
- **Tests** — 20 handler integration tests + 5 validation unit tests (`lambda-responses`), 12 repo tests (`persistence/tests/responses.rs`). They cover:
  - saves and publishing:
    - draft save
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
- **Router** (`App.tsx`) — every [`frontend.md` §3](../../docs/design/frontend.md#3-routing) route is registered.
  - Guards: `RequireAuth` → `RequireMembership` → `RequireGroupAdmin`.
  - `/auth/callback` renders outside the shell; `/admin/bootstrap-login` is registered only when `VITE_ENV=dev`.
- **B4 resolved as a policy** ([`frontend.md` §16](../../docs/design/frontend.md#16-legacy-mock-pages-historical)):
  - The M9–M12 pages (Candidates, Suggest, Newsletter, Respond, GroupAdmin) and their data layer moved to `src/mocks/` (`legacyQueries.ts`, `types.ts`).
  - They render only in mock mode. Against a real API each shows `PendingMilestonePage`.
  - Each later milestone migrates its pages and deletes the legacy code it replaces.
- **Real hooks** — `api/queries.ts` (`useConfig`, `useMe`, `useGroup`, `useNewsletters`) and `api/mutations.ts` (`useRedeemInvite`, `useUpdateProfile`, `useLeaveGroup`).
- **Components** — `AppShell` (group switcher, account menu, defaults `currentGroup` to the first membership), `BottomNav`, `GroupSwitcher`, `JoinForm`, `Spinner`.
- **`HomePage`** — per-group sections built from parallel `listNewsletters` calls. Signed-out landing; join form when there are no memberships.
- **`JoinPage`** — stash + login when signed out; redeem-once when signed in. Error copy per `ApiError.code`; lands on `/g/{g}/upcoming` per [`auth.md` §4.5](../../docs/design/auth.md#45-spa-wrapper).
- **`SettingsPage`** — display name (1–40 chars, per [`api.md`](../../docs/design/api.md)), avatar colour over the 8 `AvatarColorSlug`s, Your groups (View / Manage / Leave with the `LAST_ADMIN` copy), Sign out.

### Infrastructure
- **API authorizer audience** — the JWT authorizer now also accepts the `admin-bootstrap` client's audience. Without it, tokens from `/admin/bootstrap-login` and `bootstrap_admin.py` would have been rejected, contradicting [`environments.md` §6](../../docs/development/environments.md#6-cognito-strategy-in-dev). There's a new CDK test for it.
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

### Decisions (recorded in [`frontend.md` §6](../../docs/design/frontend.md#6-auth-in-the-spa)/§7.8/§13/§16, [`auth.md` §7](../../docs/design/auth.md#7-token-lifetime-and-refresh)/§8, [`infrastructure.md` §6.1](../../docs/design/infrastructure.md#61-http-api-apigatewayv2)/§6.3, [`build-order.md`](../build-order.md) M7)
- Generated types stay at `frontend/src/types/api.ts`; [`build-order.md`](../build-order.md) M7's `api/generated.ts` was corrected.
- `{ invite, returnTo }` rides in `oidc-client-ts`'s client-side state data, keyed by the opaque OIDC `state` nonce, not base64'd into `state`. There is a `sessionStorage` fallback.
- The callback hands the invite to `/join?code=…`, so there is one redemption code path.
- Silent renew uses the refresh-token grant; Cognito has no `prompt=none` flow. [`auth.md` §7](../../docs/design/auth.md#7-token-lifetime-and-refresh) was wrong.
- Logout builds the Cognito `/logout` URL by hand, because the discovery document has no `end_session_endpoint`.
- The CSP `<meta>` tag is deferred to M13, as a build-only transform: the Vite React preamble is an inline script that `script-src 'self'` would block in dev.
- Stale [`infrastructure.md` §6.3](../../docs/design/infrastructure.md#63-routes-and-integrations) text claimed `POST /invites/redeem` had no authorizer. §6.5 and the code say every route does; §6.3 is fixed.

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
- **Last-admin race (backend).** Removing or demoting an admin checked the admin count with a separate read, so two admins demoting each other at once could leave zero admins. The write now carries a ConditionCheck that a *witness* admin is still an admin ([`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants) #7); losing the race returns `LAST_ADMIN`. `lambda-groups` gained a `lib.rs` (crate template) and its first integration tests (`tests/members.rs`), including the concurrent mutual-demotion case.
- **Vote-cap race (backend).** `votesPerUserPerCycle` was only a pre-read, so parallel votes on different candidates could exceed it, contradicting [`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants) #1 and [`api.md` §6.3](../../docs/design/api.md#63-post-groupsgroupidcandidate-questionsquestionidvotes). A new `VoterTally` item ([`data-model.md` §2.7a](../../docs/design/data-model.md#27a-voter-tally)) with a conditional `ADD` now guards it inside the cast/withdraw transactions, and the admin candidate-delete cascade releases voters' slots.
- **Vote race-retry was dead code.** `cast_vote_tx`/`withdraw_vote_tx` turned every SDK error into `RepoError::Dynamo`, so `is_lost_race()` never matched, and two simultaneous votes on one question gave one voter a 500. Vote transactions now return a typed `VoteTxError` built from DynamoDB's per-item cancellation reasons: a duplicate vote is idempotent, a count race retries, and a full tally returns `VOTE_CAP_REACHED`.
- **Panics on dev-tick input.** `advanceCycleClosesBy` panicked in three ways: on a multibyte last char (`split_at`), on an out-of-range amount (`Duration::days`), and when `DateTime - duration` overflowed. It now splits on chars, uses the `try_` constructors, and bounds the input to 0–366 days.
- **DB errors masked as 404** in the admin candidate delete (`load_candidate(..).is_ok()`). Real lookup failures now propagate as 500.
- **`get_users_batch` ignored `UnprocessedKeys`**, so members could silently vanish under throttling. A shared `persistence::batch::batch_get_items` now chunks at 100 keys and retries unprocessed keys with backoff.
- **Home hid the open newsletter for the whole response window, every month.** Next month's voting cycle exists alongside the open one ([`lifecycle.md` §5.2](../../docs/design/lifecycle.md#52-promote_and_opennl)), and Home picked only the first. Both now render (test added).
- **Guards bounced newly granted access.** `/config` never goes stale, so a membership or role granted in another tab or on another device was refused. `RequireMembership`/`RequireGroupAdmin` now refetch once before denying (`routes/useConfirmedAccess.ts`, tests added).
- **Bootstrap-login sessions were signed out at the 60-minute mark.** `automaticSilentRenew` renewed through the frontend client, failed, and the error handler removed the user. Renewal is now `auth/renewSession.ts`: the refresh-token grant for federated sessions, Cognito `REFRESH_TOKEN_AUTH` for bootstrap sessions (picked by the ID token's `aud`).
- **ID-token decoding garbled non-ASCII names** (bare `atob`). It now decodes as UTF-8.
- **Keyboard users couldn't close the account menu or group switcher.** A shared `useDismissableMenu` adds Escape plus focus management (test added).
- **The group switcher didn't navigate** when you picked a group; it now goes to `/g/:g`.

### Gaps closed
- **`MembershipSummary` gained `timezone` and `gradient`**, read with one `BatchGetItem` instead of a `GetItem` per group. Home labels cycles in the group's timezone, and the switcher shows gradient swatches (`utils/gradient.ts`).
- **`fieldErrors` is populated.** `ApiError::invalid_field(field, detail)` covers 35 field-specific validation sites, with `code: "INVALID"`. [`api.md` §1.1](../../docs/design/api.md#11-error-format) had contradicted itself on field codes and now documents this.
- **`GET /healthz` returns `version`/`buildAt`**, baked in at compile time from `BUILD_SHA`/`BUILD_AT`, which `make build-lambdas` sets.
- **Frontend cleanup:**
  - removed the legacy "Curate" tab
  - ESLint is at 0 warnings
  - `.app-container` matches [`frontend.md` §4.1](../../docs/design/frontend.md#41-shell) widths (48rem at md, 42rem at lg)
  - Home split into one component per file (`components/home/`)
- **Python formatting:** `ruff format` applied to `infra/` and `scripts/`, and the scripts' executable bits restored.
- **Doc reconciliation:**
  - [`data-model.md`](../../docs/design/data-model.md) reconciled to snake_case item attributes, with 5 flagged notes.
  - [`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants)'s `ClientRequestToken` claim removed; it was never implemented, and a reused token breaks the vote retry.
  - [`environments.md` §16](../../docs/development/environments.md#16-frontend-mocking) (frontend mocking) rewritten.
  - The [`frontend.md`](../../docs/design/frontend.md) auth file layout now matches the code.
  - CLAUDE.md gained the frontend check command.

---

## M8 detail — what landed (2026-10-04)

The lead settled the contract first: every media route and DTO went into `shared/openapi.yaml`, and `frontend/src/types/api.ts` was regenerated. Then four agents ran in parallel: lambda-media and image-process (Sonnet), infra (Haiku) and frontend (Sonnet). The lead reviewed the presign, delete and processing cores and fixed two infra omissions.

### Backend — `lambda-media` (binary `media-api`)
- All eight routes: `POST /uploads`, `GET`/`PATCH`/`DELETE /uploads/{imageId}`, `POST /uploads/{imageId}/complete`, `POST /avatars`, `GET`/`DELETE /avatars/{avatarId}`.
- Presigned PUTs are built offline from the S3 client, with `RequestChecksumCalculation::WhenRequired`. The response's `headers` map is every signed header. An optional `sha256` is signed as `x-amz-checksum-sha256`.
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
  - the avatar originals bucket
  - originals-bucket CORS `allowed_headers=["*"]`
- `MediaPipelineStack`:
  - real `image-process` code
  - a second S3 notification on the avatars bucket (re-imported by ARN, no cycle)
  - `GetItem`/`UpdateItem` on the table, read+delete on originals, put on processed (`GetItem` added by the lead)
- `ApiStack`:
  - the `Media` Lambda and its 8 routes
- Makefile: `FUNCTION_media-api`, `FUNCTION_image-process`.

### Frontend
- `api.media.*` in `api/client.ts`. The `/uploads/{id}*` calls carry `groupId`/`cycleId` query params.
- `utils/media.ts`:
  - `pollMediaStatus`
  - `sha256Base64`
- `utils/uploadToS3.ts` does the XHR PUT with progress.
- `components/responses/ImageUploader.tsx` is standalone and not wired into the legacy `RespondPage`. It covers drag-drop and picker, client validation, presign → PUT (one retry on 403) → poll, thumbnails, alt text, remove and retry.
- `SettingsPage` has avatar upload and remove.
- Stateful mocks for all eight routes.

### Decisions (recorded in [`api.md` §1.1](../../docs/design/api.md#11-error-format)/§9, [`media.md` §3](../../docs/design/media.md#3-upload-flow-browser--s3)–§5/§9, [`data-model.md` §2.10](../../docs/design/data-model.md#210-imagemedia)/§2.16, [`infrastructure.md` §5.3](../../docs/design/infrastructure.md#53-lambda-image-process-rust-mediapipelinestack)/§6)
- `/uploads/{imageId}*` routes take **required `groupId` + `cycleId` query params**, because the row is keyed by them. `GET`/`complete` are open to any member; `PATCH`/`DELETE` are owner-only.
- `sha256` is **standard base64** and is signed as `x-amz-checksum-sha256` (the spec had said "base64url" and `x-amz-content-sha256`).
- The 10-image cap counts within the cycle partition, not GSI1. A brief overshoot under concurrent uploads is accepted, because the response save caps at 10 anyway.
- `DELETE /uploads` soft-fails the row (`errorMessage: "DELETED"`) and refuses with `IMAGE_IN_USE` when the caller's published answer lists the image.
- `ImageMedia`/`AvatarMedia` gained `error_message` (`IMAGE_TOO_LARGE` / `IMAGE_DECODE_FAILED` / `DELETED`).
- The client hashes with `crypto.subtle.digest`, not a Web Worker.
- A PUT-403 retry calls `POST /uploads` again and gets a new ID; the abandoned `pending` row is tolerated per [`media.md` §8.1](../../docs/design/media.md#81-orphan-originals).
- Lossy WebP needs `libwebp` (the `webp` crate), because `image`'s own WebP encoder is lossless-only.
- Processed objects carry `Cache-Control: public, max-age=31536000, immutable` (img; cut to 60 days on 2026-10-11) or `max-age=86400` (avatar).

### Tests
Backend 308 (was 217), CDK 53 (was 42), frontend 52 (was 28). Of the [`media.md` §10](../../docs/design/media.md#10-tests) list, the backend covers:
- the presign shape (1)
- all four formats (2)
- GIF animation (3)
- EXIF orientation (4)
- the 16 MB failure and delete (5)
- SVG → 415 (6)
- the 11th upload → `IMAGE_LIMIT_EXCEEDED` (8)

The live-S3 half of 1 needs a deploy.

**Testcontainers flakiness (fixed 2026-10-09):** with every DynamoDB Local suite running at full parallelism, Colima intermittently dropped container connections (`DispatchFailure … IncompleteMessage`). `make_test_repo` now waits for the server, sets request timeouts and throttles start-ups (see "Code review follow-ups"), so `--test-threads=4` is no longer needed.

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
- `api/client.ts`: `getNewsletter`, `listCandidates`, `createCandidate`, `castVote`, `withdrawVote`, `saveMyResponse`.
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
- `SuggestPage`: react-hook-form + zod (text/poll, 2–6 options, "Ask anonymously"). It mirrors the server's trimming and duplicate-label rule (case-sensitive at M9; case-insensitive since the 2026-10-07 plan review). Server `fieldErrors` map onto fields.

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

### Decisions (recorded in [`frontend.md` §7.2](../../docs/design/frontend.md#72-newsletterpage-ggnc)–§7.5/§9/§16, [`media.md` §7](../../docs/design/media.md#7-image-references-in-markdown), [`coding-standards.md` §3.9](../../docs/development/coding-standards.md#39-tests), [`testing-and-ci.md` §4.2](../../docs/development/testing-and-ci.md#42-api-mocking-for-component-tests), [`environments.md` §16](../../docs/development/environments.md#16-frontend-mocking))
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
- `lambda-engagement`, binary renamed to `engagement-api`, laid out like `lambda-responses`. It serves all seven [`api.md` §8](../../docs/design/api.md#8-engagement-routes-comments--reactions) routes, with the check order fixed there: membership → cycle exists → not archived → published → answer resolves.
- `domain::engagement`: `normalize_emoji` (percent-decode if needed, then NFC), `is_valid_emoji` (1–12 codepoints, with an `Extended_Pictographic` or regional-indicator codepoint), and `group_reactions` (count desc, earliest, emoji). `lambda-newsletters` now uses the same `group_reactions` (it used to sort by emoji) and the same `CommentResponse` hydration. Soft-deleted comments come back as placeholders instead of being filtered out.
- `persistence::engagement`:
  - a paginated `list_comments_page`
  - `get_comment_by_id`
  - transactional `create_comment`, `update_comment` and `soft_delete_comment`, which claim and release the comment image via `ImageMedia.attached_comment_id`
  - `persistence::cursor` for the opaque cursor
- `lambda-media` `DELETE /uploads/{id}` returns 409 `IMAGE_IN_USE` for a claimed comment image (the carried M9 gap).

### Infrastructure
- `ApiStack` gains the `Engagement` function (`engagement-api`). It gets table read/write via `_handler_lambda`, like newsletters-api.
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
- `NewsletterPage` shows the reaction total in the published header ([`frontend.md` §14a](../../docs/design/frontend.md#14a-client-derived-display-data-not-api-fields)).
- `api/mockEngagement.ts` is registered in `mockTransport`, and the published fixture is seeded with an edited comment, a deleted comment and reactions.
- The legacy engagement hooks were removed from `src/mocks/`.

### Bugs caught in lead review (fixed)
- **An edit racing a delete could resurrect a deleted comment.** `update_comment` had no condition, so a PATCH landing just after a soft delete wrote the body back. It is now conditioned on `deleted_at` being absent or NULL, and the handler re-reads on a cancelled transaction to return 404 (deleted) or 422 (image claimed elsewhere). A regression test was added (`persistence/tests/engagement.rs`).
- Reaction optimistic updates now `cancelQueries` before patching, so a stale in-flight refetch can't overwrite them.
- The `ApiStack` docstring misattributed which milestone added which Lambda.
- **Clippy 1.99's new `double_must_use` broke `-D warnings` in `lambda-image-process`.** This wasn't an M10 change. It fired through `#[async_trait]` and is now `allow`ed with a reason. In response, `rust-toolchain.toml` is pinned to `1.99.0` (2026-10-06), so later stable releases can't break CI unannounced.

### Bugs caught in post-M10 code review (fixed, with regression tests)
- **Replacing a comment's image sent `imageMediaId: null`.** In the edit form, "Remove image" followed by a new upload dropped the image, or got a 422 if the text was empty. A freshly uploaded image now wins over the removal flag (`CommentList.tsx`).
- **The comment composer's uploader kept the posted image.** Its single slot stayed full, removing the image got `IMAGE_IN_USE`, and the next comment could re-send the image and get a 422. The uploader is now remounted (fresh `key`) after a successful post.
- **A delete racing an image change could strand an image.** A delete that read image A before an edit swapped in B released A and left B claimed by a `[deleted]` row. Both `soft_delete_comment` and `update_comment` are now conditioned on the comment still holding the image that was read; the handlers re-read and retry (`COMMENT_WRITE_ATTEMPTS`).
- **A well-formed but foreign cursor returned 500.** A cursor from another answer, or a hand-crafted one, reached DynamoDB and came back as a `ValidationException`. `persistence::cursor::decode_scoped` now checks the key is in the listing's own partition and `C#` range, so it gets a 422 on `cursor`.

### Decisions (recorded in [`api.md` §8](../../docs/design/api.md#8-engagement-routes-comments--reactions)/§9.4/§13, [`engagement.md` §1.1](../../docs/design/engagement.md#11-shape)/§1.5/§2.3/§2.4, [`data-model.md` §2.10](../../docs/design/data-model.md#210-imagemedia))
- **Engagement routes are nested under `questions/{questionId}`.** Comment and reaction rows are keyed on `(question, answerUser)`, and `responseId` alone has no index. The answer is resolved by matching `response_id` among that question's answers, and must be a published text answer, otherwise 404.
- One `CommentResponse` (`displayName`, not `authorDisplayName`) is used both inline and on the engagement routes. Soft-deleted comments are returned with `deletedAt` set, an empty body and a null image.
- **Comment images are claimed via `ImageMedia.attached_comment_id`,** set and cleared in the same transaction as the comment write.
- **PATCH semantics:** an absent field is left unchanged, and `imageMediaId: null` removes the image. Only the author can edit; admins get 403.
- Reaction PUT and DELETE return the updated `ReactionsResponse`.
- No cross-request cache for author names (see Known gaps).
- **Comment writes are conditioned on the image they read,** and retried up to 3 times on a lost race ([`engagement.md` §1.5](../../docs/design/engagement.md#15-author-display-name-resolution)).
- **Out-of-scope pagination cursors are a 422,** the same as malformed ones ([`api.md` §1](../../docs/design/api.md#1-general-conventions)).

### Tests
- Backend 361 (was 308):
  - `lambda-engagement` integration tests: comments 17, reactions 6
  - engagement unit tests: 11
  - contract-table rows for all seven routes
  - the media in-use test
  - the persistence race regression
- Post-review fixes: backend +4 (`cursor` scope 2, persistence stale-image races 2; the handler cursor test was extended), frontend +3 (`CommentList`: image replace, explicit-null removal, composer reset). Totals: backend 365, frontend 166.
- CDK 54 (was 53): the new engagement env test. The Apple-IdP removal renamed an existing test but added none.
- Frontend 163 (was 116):
  - `emoji` 14 and `mockEngagement` 11
  - `ReactionBar` 5 and `CommentList` 6
  - `client` +7 and `markdown` +4

---

## M11 detail — what landed (2026-10-06)

The lead settled the spec's gaps (decisions D1–D11, folded into the plan docs) and wrote the push contract into `shared/openapi.yaml` (then regenerated `api.ts`) before any agent started. Three agents ran in parallel: a Sonnet backend agent, a Haiku infra agent and a Sonnet frontend agent. The backend agent hit the session limit twice and never finished; the lead completed the backend directly (persistence tests, the `cycle-tick` invoke) and audited the rest.

### Backend
- **`lambda-push`** (binary `push-api`, lib `push`), laid out like `lambda-engagement` plus:
  - `webpush.rs`: RFC 8291 `aes128gcm` encryption and RFC 8292 ES256 VAPID JWTs in pure Rust (`p256`, `hkdf`, `aes-gcm`, `sha2`). **Not** the `web-push` crate, whose OpenSSL `ece` and isahc/libcurl dependencies put the arm64 cross-compile at risk (B6). `VapidKeys::parse` checks the private key derives the public key.
  - `sender.rs` (a `PushSender` trait, reqwest/rustls impl), `delivery.rs` (send one + [`notifications.md` §3.2](../../docs/design/notifications.md#32-pruning-failures) bookkeeping), `payload.rs` (payload + copy, ≤3,000 bytes), `fanout.rs` (cycle-open and publication), `metrics.rs` (EMF).
  - One binary serves the six `/push/*` routes and the internal fan-out invoke. It takes the raw event and checks `requestContext` **first**, so an HTTP caller can never forge an `{internal: …}` event.
- **`lambda-notify-tick`** (binary `notify-tick`, lib `notify_tick`): every 15 minutes, finds `open` cycles closing within `MAX_REMINDER_OFFSET_HOURS` (`newsletters::list_open_cycles_closing_within`, GSI2) and sends deadline reminders in-process through the `push` lib. Also serves the dev-only `POST /admin/dev/tick/notify` (404 outside dev, 403 for non-admins).
- **`lambda-cycle-tick`** now async-invokes `lambda-push` (`InvocationType::Event`, function name from `PUSH_FUNCTION_NAME`) after each `voting → open` and `open → published`, through a `Notifier` trait. `run_tick(&repo)` keeps a no-op notifier; `run_tick_with` takes one. An unset env var logs a WARN and disables fan-out; an invoke failure is logged and never fails the tick.
- **`persistence::push`**: `upsert_subscription` (keeps `createdAt`/`lastSuccessAt`, resets `failureCount`, enforces the 10-sub cap by evicting the least recently successful), `mark_delivered`/`record_failure` conditioned on the row existing (never resurrect a deleted row), at-most-once `claim_*_notified_marker`, and the `TICK/NOTIFY` sentinel.
- `shared::config`: `MAX_PUSH_FAILURES`, `MAX_PUSH_SUBSCRIPTIONS_PER_USER`, `PUSH_TTL_SECONDS`, `PUSH_SEND_TIMEOUT_SECS`, `MAX_PUSH_PAYLOAD_BYTES`.

### Infrastructure
- `ApiStack`: the `Push` function (`push-api`, 60s timeout, `VAPID_SECRET_ARN` + `GetSecretValue`), the six push routes, and `POST /admin/dev/tick/notify` → notify-tick (new `notify_tick_fn` constructor param). `_handler_lambda` gained `timeout_secs`.
- `NotificationsStack`: `NotifyTickFn` (120s, 512 MB, 15-minute rule, VAPID env + grant, own log group). `cycle_tick_fn` gets `PUSH_FUNCTION_NAME` and `lambda:InvokeFunction` on an ARN built with `Stack.format_arn`. Passing the `Push` construct would cycle, since `ApiStack` depends on this stack.
- `MonitoringStack`: `PushFailureRateAlarm` (metric math `PushFailed / (PushSent + PushFailed) > 0.2` over 30 min, missing data not breaching), plus NotifyTick and CycleTick error alarms.
- `Makefile`: `FUNCTION_push-api`, `FUNCTION_notify-tick`.

### Frontend
- `vite-plugin-pwa` (`injectManifest`) builds `src/pwa/sw.ts`: precache, `CacheFirst` for media images, `push` and `notificationclick` (pure helpers in `swHelpers.ts`). Own `tsconfig.sw.json`; `npm run typecheck` checks both.
- `src/pwa/pushSetup.ts`: `isPushSupported`, `ensurePushSubscription(key, {prompt})`, `disablePush`, `getLocalPushState`. `App.tsx` refreshes the subscription on every authenticated load (`prompt: false`). `AuthProvider.logout` unsubscribes best-effort, raced against a 2s timeout.
- Settings "Notifications" section: master toggle, "Send test push", "My devices" (with a "This device" badge), and per-group "Cycle open"/"Deadline reminders" toggles (optimistic). Unsupported-browser and iOS-not-installed states per [`notifications.md` §14](../../docs/design/notifications.md#14-failure-modes--ux).
- Install prompt: `beforeinstallprompt` capture, plus a dismissible iOS "Add to Home Screen" banner on Home and Settings.
- Manifest, placeholder icons (`scripts/generate-icons.mjs`), `npm run dev:https` (basic-ssl) for testing push locally.
- `api/mockPush.ts` in `mockTransport`; the legacy push mocks were removed from `src/mocks/`.

### Bugs caught in lead review (fixed, with regression tests where testable)
- **Pre-existing, cross-group: AP15 ignored the group.** `list_my_responses_in_cycle` queries GSI1 `USER#{u}#NL#{cycleId}`, and cycle IDs are calendar months shared by every group. A user in two groups got both groups' responses: the newsletter list/detail counts over-counted, and publishing in group B after group A in the same month **skipped B's `editions_answered` bump**. It now takes `group_id` and filters. Regressions: `persistence/tests/responses.rs` and `lambda-responses` `first_publish_in_a_second_group_same_month_still_bumps_editions_answered`. M11's reminder "N of total" count would have inherited the same bug.
- **The RFC 8291 test only round-tripped** through a mirror decryptor sharing the encoder's derivation, which can't catch a spec-level mistake. It now also asserts the exact ciphertext RFC 8291 §5 publishes (it matches).
- **Padded base64url push keys were rejected.** `keys.p256dh`/`keys.auth` are now accepted with trailing `=` and stored unpadded.
- **The push failure-rate alarm evaluated over 5 minutes, not 30.** The `MathExpression` overrode its metrics' periods to 300s; `period` is now set on the expression, and a CDK test asserts 1800s.
- **The mock-mode VAPID key wasn't a valid P-256 point**, so `PushManager.subscribe` threw in mock mode. Replaced with a real throwaway key.
- The cycle-tick invoke was never wired by the backend agent (only the `aws-sdk-lambda` dependency landed); the lead wrote it.

### Bugs found in post-M11 code review (fixed 2026-10-06)
- **Fan-outs sent to subscriptions one at a time under a fixed time limit.** Each send could take up to 10s. At 60s, 6 hung endpoints were enough to kill a cycle-open/publication fan-out partway through. Its marker was already claimed, so retries skipped it and the remaining members silently got nothing. The same applied to notify-tick's 120s. `POST /push/test` (up to 10 devices × 10s) could pass API Gateway's 30s limit and 503. Now every fan-out sends concurrently (16 in flight) with a send cutoff under each limit. Skipped deliveries are logged at ERROR and counted as `PushSkipped`, and notify-tick stops claiming new cycles after 60s ([`notifications.md` §6.2](../../docs/design/notifications.md#62-concurrency)). Tests: `lambda-push/tests/delivery.rs` (concurrency, cutoff) and a `metrics` unit test.
- **The push failure-rate alarm couldn't see a total outage.** With every send failing, `PushSent` has no data, so `failed / (sent + failed)` was missing, which counts as not breaching. Now `FILL(sent, 0)`; the CDK test asserts it.

### Decisions (recorded in [`notifications.md` §2](../../docs/design/notifications.md#2-vapid-keys)/§5/§6/§7/§8/§10/§11/§13, [`api.md` §10](../../docs/design/api.md#10-push-routes)/§11a.2, [`data-model.md` §2.15](../../docs/design/data-model.md#215-counters-and-idempotency-markers)/AP15, [`frontend.md` §5.2](../../docs/design/frontend.md#52-service-worker--caching-strategy), [`infrastructure.md` §7.2](../../docs/design/infrastructure.md#72-lambda-cycle-tick)/§7.3)
- Own Web Push implementation, not the `web-push` crate (above). The VAPID secret holds raw base64url scalars.
- **Markers are claimed before fanning out** (`attribute_not_exists`): at-most-once, so a crash mid-fan-out loses the rest rather than risking duplicates. New `NOTIFIED#PUBLISH` marker. `Newsletter.notified_offsets_hours`/`notified_on_open` stay vestigial.
- **Reminders:** an offset landing at or before `responseOpenAt` is moot (marked, never sent). Of the due offsets, only the smallest is sent and all are marked, so a late tick or a dev rewind doesn't burst. Members who've published every answer are skipped at the group's smallest offset.
- The payload `url` is root-relative (`/g/{g}/n/{c}`), resolved by the service worker against its own origin, so no frontend-origin env var is needed.
- Subscribe keeps `createdAt`/`lastSuccessAt`; the cap is 10 per user with LRU eviction; unsubscribe is idempotent and also serves "My devices" removal.
- New `GET /push/preferences` (one entry per membership, defaults both true).
- **No API runtime caching in the service worker** (deviation from [`frontend.md` §5.2](../../docs/design/frontend.md#52-service-worker--caching-strategy)): an authenticated response in Cache Storage would outlive logout on a shared device.
- notify-tick runs reminders in-process (it has the VAPID secret) and doesn't need `InvokeFunction`; 120s timeout per [`infrastructure.md` §7.3](../../docs/design/infrastructure.md#73-lambda-notify-tick).
- EMF dimension sets `[["kind"], []]`, so the alarm can use the dimensionless totals.

### Tests
- Backend 427 (was 365): `lambda-push` unit tests (webpush incl. the RFC 8291 vector, validation, payload copy, dispatch) and DynamoDB Local tests (`push_routes.rs`, `fanout.rs`); `lambda-notify-tick` tests (`notify_tick.rs`); `persistence/tests/push.rs` +5 (re-subscribe, cap eviction, failure limit, no resurrection, markers); `cycle-tick` envelope test; the lifecycle test now asserts both fan-outs fire; the AP15 regressions; six push contract-table routes.
- CDK 68 (was 54): 14 new (push function and routes, notify-tick function and schedule, invoke grant, alarms incl. the 1800s period).
- Frontend 220 (was 166): `pushSetup` 16, `swHelpers`, `installPrompt`, `NotificationsSection`, `mockPush`, `client` additions.

---

## M12 detail — what landed (2026-10-06)

Frontend-only apart from one backend validation fix: every admin route already existed in the backend and in `shared/openapi.yaml`. The lead settled the semantics up front (recorded in [`frontend.md` §7.7](../../docs/design/frontend.md#77-groupadminpage)/§16), a Sonnet agent built it, and the lead reviewed it and re-ran every check.

### Frontend
- **`pages/GroupAdminPage`** is on the real API and routed unconditionally under `RequireGroupAdmin`. It has three accessible tabs (`tablist`/`tab`/`tabpanel`):
  - **Members**: avatar, role pill and the "N of cap" line. A row menu offers make admin, make member and remove; remove goes through `ConfirmDialog`. The caller's own row has no menu.
  - **Invites**: create with role + TTL (1/3/7/14/30/90 days), then show the copyable URL. The list shows active, expired (derived client-side), revoked and consumed invites, with "used by {name}" or "a former member". Revoke is a direct action.
  - **Settings**: `components/admin/GroupSettingsForm` (react-hook-form + zod) covers name, gradient swatches, timezone (`Intl.supportedValuesOf`), cycle settings, reminder offsets, `onCycleOpen` and `memberSoftCap`. Its client validation mirrors `lambda-groups/src/validation.rs`. It sends only the changed fields and renders the server's `fieldErrors` inline.
- **Data layer**:
  - `api/client.ts` gained `createInvite`, `listInvites`, `revokeInvite`, `patchMember` and `patchGroup`.
  - `queries.ts` gained `useInvites` and `queryKeys.invites`.
  - `mutations.ts` gained `useCreateInvite`, `useRevokeInvite`, `usePatchMember`, `useKickMember` and `usePatchGroup`. `usePatchGroup` invalidates `/config` and the group's newsletter list.
- **`utils/inviteUrl.ts`** builds the join link from `window.location.origin` + `BASE_URL`. This replaces the legacy hardcoded `opennewsletter.example.com`.
- **Mock mode**:
  - `api/mockAdmin.ts` serves the five admin routes.
  - The mock `groups` fixture moved to `mockShared.ts` so admin edits show up in `GET /groups/{g}`.
  - Mock `/config` now derives membership name/gradient/timezone from the live fixture.
- **Legacy removal ([`frontend.md` §16](../../docs/design/frontend.md#16-legacy-mock-pages-historical) complete)**: deleted `src/mocks/` and `PendingMilestonePage`, plus the legacy colour table in `utils/avatar.ts` and `Avatar`'s `color` prop. This clears two Known gaps: the `["config"]`/`["group", g]` mock-cache collision and the legacy avatar slugs.

### Bugs found (fixed)
- **Mock `removeMember` hardcoded `LAST_ADMIN` for `g_trail`.** Every mock kick in that group would have failed. It now mirrors the backend's `admin_witness` rule: only a self-removal or self-demotion by the sole admin gets 409. The lead checked this against `group_routes.rs`.
- **Group-name 422s were reported against `displayName`.** `group_patch` reused the `/me` validator. It is now split into `name_field(raw, field)`, so the field is `name`, with a regression test (`blank_group_name_is_reported_against_name`). The form still aliases `displayName` → `name` as well, which is harmless.
- **Save stayed enabled after a reverted edit.** The settings form compared raw string inputs with numeric baselines. Both sides are now normalised, with a test.

### Tests
- **Frontend: 267 (was 220; first recorded as 265, recounted in M13).** +15 `GroupAdminPage`, +20 `mockAdmin`, +6 client path/body, +5 `inviteUrl`. Some legacy-only coverage went with the deleted files.
- **Backend: 431** (+1 for the validation regression test). The full `--no-fail-fast` run had one DynamoDB Local "create table" flake (`promotion_is_idempotent_across_rapid_reruns`, under load), and it passed on a solo re-run. That is the known overload flake, not a regression.
- **CDK: 68**, unchanged.

---

## M13 detail — what landed (2026-10-07)

The lead settled the semantics first and recorded them in [`testing-and-ci.md` §2.1](../../docs/development/testing-and-ci.md#21-unit-in-crate)/§2.4/§4.3/§5.2/§6, [`environments.md` §10](../../docs/development/environments.md#10-local-testing--what-runs-where), [`notifications.md` §3.1](../../docs/design/notifications.md#31-create), [`frontend.md` §12.1](../../docs/design/frontend.md#121-content-security-policy)/§13 and [`data-model.md` §2.2](../../docs/design/data-model.md#22-groupmembership-also-serves-as-group--user-index-via-gsi1)/§4 #4. Three Sonnet agents then worked in parallel, split by directory into backend, frontend and CI/infra. Two of them hit an API rate limit mid-task and were resumed. The lead reviewed every diff, fixed what's listed under "Bugs", and re-ran every check.

### CI/CD (`.github/workflows/`)
- **Six workflows**, each with a header comment:
  - `frontend-ci`: typecheck, ESLint, Prettier, `test:coverage` (artifact uploaded) and build.
  - `backend-ci`: fmt, clippy, `cargo deny`, `cargo test --workspace` (DynamoDB Local via the runner's Docker) and `cargo lambda build --release --arm64`.
  - `infra-ci`: ruff, `mypy --strict`, pytest, and synth for dev and prod. A PR-only `cdk diff` against prod is posted as a comment.
  - `frontend-deploy`: builds, writes `CNAME`, copies `404.html` ([`frontend.md` §14.5](../../docs/design/frontend.md#145-github-pages-spa-fallback)) and publishes to `gh-pages`.
  - `backend-deploy`: `cargo lambda` build, then `cdk deploy --all` via OIDC.
  - `e2e`: nightly Playwright run.
- **Deploy-shaped steps are off until repo variables switch them on** (`FRONTEND_DEPLOY_ENABLED`, `BACKEND_DEPLOY_ENABLED`, `AWS_DIFF_ROLE_ARN`, `E2E_BASE_URL`), so `main` stays green without AWS. `.github/workflows/README.md` lists every variable and secret.
- **Node comes from `.nvmrc`** (now 24) in every workflow.

### Infrastructure
- **`CiStack`** (account-wide, unsuffixed):
  - a GitHub OIDC provider
  - `GithubDeployRole`: trusts only `repo:…:ref:refs/heads/main`, and may only `sts:AssumeRole` onto `cdk-hnb659fds-*`
  - `GithubDiffRole`: trusts `pull_request` only; lookup role plus read-only CloudFormation on `*-prod` stacks
  - The repo slug lives in `config.py` (`github_repo`).
- **`MonitoringStack` implements [`infrastructure.md` §9.1](../../docs/design/infrastructure.md#91-dashboard)/§9.2.** It has 17 alarms, all on the SNS topic:
  - per-Lambda error rate >5%
  - API 5xx >1%
  - DynamoDB throttles
  - the two tick-error alarms
  - the existing push-failure alarm

  The dashboard has real widgets for API, Lambdas, DynamoDB, S3 storage and the push EMF metrics.
  - **No exports.** Metrics are built from fixed names. The HTTP API id isn't deterministic, so `ApiStack` publishes it to SSM (`ssm_refs.py`), and `MonitoringStack` reads it with `{{resolve:ssm:…}}` dynamic references.
- **CDK: 86 tests** (+9 `test_ci_stack.py`, +9 `test_monitoring_stack.py`).

### Backend
- **Test-gap audit against [`testing-and-ci.md` §2](../../docs/development/testing-and-ci.md#2-rust-backend-tests).** Every key builder, AP1–AP21, every §4 transaction, the 8-item lifecycle suite and the image-pipeline suite were already covered.
  - **New concurrency tests** use `tokio::join!` on the three named high-stakes transactions: vote (same question; same voter at the cap), redeem-invite (at the member cap) and publish-response.
  - **`shared::http::error_type_uri`** got pure-core tests.
  - **Doc correction:** [`testing-and-ci.md` §2.1](../../docs/development/testing-and-ci.md#21-unit-in-crate)'s `JwtVerifier` bullet named a type that doesn't exist (API Gateway verifies JWTs), so it was removed.
- **Push-endpoint host allowlist** ([`notifications.md` §3.1](../../docs/design/notifications.md#31-create)). `shared::config::PUSH_ENDPOINT_HOST_SUFFIXES` covers FCM, Mozilla, Apple and WNS. The check is against the host parsed by the `url` crate, so `fcm.googleapis.com.evil.com` and `https://fcm.googleapis.com@evil.com/` are rejected. This clears the Known gap.
- **`cargo deny`** (`backend/deny.toml`):
  - Permissive-license allowlist: `Unicode-3.0` (via `idna`), `CDLA-Permissive-2.0` (`webpki-roots`) and `NCSA` (fuzzing-only).
  - Sources limited to crates.io.
  - `cargo update` moved `h2`/`rustls`/`rustls-webpki` past fixed advisories.
  - The remaining ignores each carry a reason. See Known gaps.

### Frontend
- **CSP `<meta>`** (`src/csp/csp.ts` + `cspPlugin.ts`, build-only). Origins come from `loadEnv`, which includes `VITE_*` from the process env, as the deploy workflow passes them. `worker-src`/`manifest-src` were added.
- **Playwright** (`npm run e2e`):
  - The `mock` project builds `--mode e2e` (`.env.e2e`) and serves with `vite preview`. It runs 7 tests: two CSP assertions, a no-violations walk, and four golden paths (suggest, upvote, react toggle, comment) plus draft recovery.
  - The `dev` project runs only with `E2E_BASE_URL`. All ten [`testing-and-ci.md` §4.3](../../docs/development/testing-and-ci.md#43-playwright-e2e) paths are scaffolded as `fixme` until B5 clears.
  - The viewport is 700px, the only width where `BottomNav` and its labels are both in the accessibility tree.
- **Vitest §4.1 audit: 293 tests (+26).** New `dates.test.ts` (12 tests; `utils/dates.ts` had none) and `PollWidget.test.tsx`, plus gap tests for:
  - flushing a dirty draft on `visibilitychange`
  - a sticky poll-vote change after publish
  - turning a reaction off successfully
  - markdown headings
- **New dev dependencies:**
  - `@playwright/test` (E2E runner)
  - `@vitest/coverage-v8` (`test:coverage`)
  - `@types/node` (types `process.env` in the Playwright config)

### Docs
- **[`smoke-test.md`](../../docs/operations/smoke-test.md)**: the [`testing-and-ci.md` §13](../../docs/development/testing-and-ci.md#13-manual-smoke-runbook) checklist covering dev (with `advanceCycleClosesBy`) and prod (direct `aws lambda invoke`).
- **README**: real Status, Local development and Contributing sections (the M13 done-when: clone → run → PR).
- **Docs found out of date and fixed:**
  - [`runbook.md`](../../docs/operations/runbook.md):
    - the IAM step now warns about root vs IAM-user `aws login`, and the "least-privilege is an M13 task" note is gone (CI uses the `CiStack` roles)
    - the Route53 step now says a hosted zone is optional (`betterloop.site` is on Namecheap DNS)
    - new M13 steps for wiring GitHub Actions and checking the 17 alarms
  - [`testing-and-ci.md` §2.1](../../docs/development/testing-and-ci.md#21-unit-in-crate) dropped the nonexistent `JwtVerifier`; §2.4/§6.3 dropped `--features integration`; §5.2 dropped stylelint; §6.1 uses `.nvmrc` instead of Node 20.
  - [`environments.md` §10](../../docs/development/environments.md#10-local-testing--what-runs-where) test matrix: integration tests aren't opt-in, and Playwright runs locally.
  - [`frontend.md` §12.1](../../docs/design/frontend.md#121-content-security-policy)/§13 document the CSP as built.
  - [`data-model.md` §2.2](../../docs/design/data-model.md#22-groupmembership-also-serves-as-group--user-index-via-gsi1)/§4 #4 document `last_answered_cycle`.
  - [`notifications.md` §3.1](../../docs/design/notifications.md#31-create) documents the push host allowlist.
  - The "Plan-set reconciliation" line about `--features integration` is marked superseded.
  - The M12 frontend test count is corrected from 265 to 267.
- **Owner change, same session:** macOS Node moved to 24 via nvm (`.nvmrc`), and `CLAUDE.md` gained the nvm loader prefix plus an `infra/bin/cdk` wrapper for `npx aws-cdk@2`. The workflows follow `.nvmrc`. `@types/node` is pinned at ^20, which is harmless; bump it with the next dependency sweep.

### Bugs found (fixed)
- **`editions_answered` double-counted under concurrency** (found by the new concurrency test). The "first publish this cycle" check was only a handler pre-read, so two concurrent first publishes both bumped.
  - The bump now also writes `last_answered_cycle`, conditioned on it not already being this cycle.
  - `save_response` reads the cancellation reasons. A failed marker condition means retry without the bump; a `TransactionConflict` means retry as-is (3 attempts); anything else is still `CYCLE_NOT_OPEN`.
  - Covered by a regression test and a stale-pre-read test ([`data-model.md` §4](../../docs/design/data-model.md#4-transactional-invariants) #4).
- **`MonitoringStack` had no deploy order relative to the SSM parameters it resolves.** A fresh `--all` deploy could fail. Fixed with an ordering-only `add_dependency` on `ApiStack` and `MediaPersistentStack`, which creates no exports.
- **`backend-deploy.yml` ran `cargo lambda build` without installing `cargo-lambda`.** Both backend workflows also ran `pip install` on `ubuntu-latest`'s system Python, which PEP 668 refuses. Fixed with `setup-python` and an explicit install.
- **The CSP blocked the Google Fonts stylesheet** (caught by the Playwright violations walk). Fixed by allowing the two Google Fonts origins ([`frontend.md` §12.1](../../docs/design/frontend.md#121-content-security-policy)).
- **Files the root `.gitignore` silently excluded** (post-M13 code review), so a fresh clone or CI couldn't build:
  - `build/` matched `frontend/src/build/`, which held the CSP plugin. It was renamed to `frontend/src/csp/`.
  - `git ls-files --others --ignored --exclude-standard` now lists only build artifacts and `.env.local`.
- **`infra-ci`'s PR `cdk diff` ran without the prod config variables**, so every comment would have shown placeholder domains and ARNs replacing the real ones. It now gets the same `env:` block as `backend-deploy`.

---

## Plan review (2026-10-07)

A read-through of plan docs `00`–`13` (now in `docs/`) after M13. Stale text was corrected to match the code. Decisions and bugs are listed below and are still open.

**Stale text fixed:**
- [`auth.md`](../../docs/design/auth.md) dropped its leftover Apple IdP text (line 3, §9.2, the §10 checklist), which the M1 sweep had missed. It also gained the M7 invite-state mechanism (`oidc-client-ts` state data, not base64 in `state`). Its `JwtVerifier`/`run_local.sh` paragraph is gone, and the bootstrap transaction now lists the sub-lookup row and the first voting cycle.
- [`infrastructure.md`](../../docs/design/infrastructure.md): marker keys, CORS headers (`*`), Query rather than Scan, no Route53 cost, and the cycle-tick → push grant.
- [`data-model.md`](../../docs/design/data-model.md): the §2.15 note now says M11 writes the `NOTIFIED#…` and `TICK/NOTIFY` rows.
- [`api.md`](../../docs/design/api.md): status banner, `VALIDATION_ERROR` → `VALIDATION_FAILED`, the 20000-char example, the §7.4 title, `askedBy` examples, the §13 enforcement pointer, and the media-deletion claim.
- [`frontend.md`](../../docs/design/frontend.md): layout tree rebuilt from the real `frontend/`, plus Node 24, no Stylelint, no route loaders or `date-fns-tz`, the emoji picker, "PWA done", stores, and axe not wired.
- [`lifecycle.md`](../../docs/design/lifecycle.md), [`notifications.md`](../../docs/design/notifications.md), [`engagement.md`](../../docs/design/engagement.md), [`archival.md`](../../docs/design/archival.md), [`testing-and-ci.md`](../../docs/development/testing-and-ci.md), [`build-order.md`](../build-order.md), [`environments.md`](../../docs/development/environments.md) and [`coding-standards.md`](../../docs/development/coding-standards.md): references to things renamed, dropped or never built (`ci-pr-*` stacks, Google sign-in in E2E, `src/mocks/`, Lambda function names, `py_vapid`, `authorDisplayName`). Also prod smoke steps that can't advance a cycle.
- [`auth.md` §4.2](../../docs/design/auth.md#42-algorithm) now returns 400 for a revoked invite, matching the catalog. [`auth.md` §12](../../docs/design/auth.md#12-threat-model-considerations) no longer claims a per-user redeem rate limit (HTTP APIs can't do one).
- [`lifecycle.md` §9](../../docs/design/lifecycle.md#9-empty-cycle-edge-cases) no longer says poll questions can be commented on. Engagement targets published text answers only, as [`api.md` §8](../../docs/design/api.md#8-engagement-routes-comments--reactions) and [`engagement.md` §3](../../docs/design/engagement.md#3-polls) say.
- Left alone: the `opennewsletter.example.com` placeholders. They match `config.py`'s defaults, and the real prod hostnames are M14's call.

**Owner decisions (2026-10-07).** These are written into the plan docs as current design, marked "decided 2026-10-07; lands in M13.5". **All twelve landed in M13.5 (2026-10-08)**, except the operator steps listed under it in "Suggested next steps". See "M13.5 detail".
1. **Tokens go in `localStorage`**, so an installed PWA stays signed in. Logout revokes the refresh token ([`frontend.md` §6](../../docs/design/frontend.md#6-auth-in-the-spa), [`auth.md` §8](../../docs/design/auth.md#8-logout)/§12).
2. **Non-empty drafts auto-publish at close.** The tick does this before flipping the status. The RespondPage warns while an answer is an unpublished draft. A poll pick saves as a draft immediately. Reminder counts include non-empty drafts ([`lifecycle.md` §5.3](../../docs/design/lifecycle.md#53-publishnl), [`notifications.md` §7.2](../../docs/design/notifications.md#72-per-recipient-body-customization), [`frontend.md` §7.3](../../docs/design/frontend.md#73-respondpage-ggncrespondq)).
3. **The response window is 7 days by default**, and the `[96, 48, 24]` reminders all fire now. No further concerns: existing dev groups keep `4` until patched or reseeded, and the 1–28 day validation is unchanged ([`lifecycle.md` §4.2](../../docs/design/lifecycle.md#42-default-schedule-per-group)).
4. **Promotion order is votes desc, then the oldest suggestion.** 0-vote candidates still fill slots. Promotion reads the pool and sorts it in memory ([`lifecycle.md` §5.2](../../docs/design/lifecycle.md#52-promote_and_opennl), [`api.md` §6.1](../../docs/design/api.md#61-get-groupsgroupidcandidate-questions)).
5. **The prod bootstrap admin is an email-bound admin invite.** `BOOTSTRAP_ADMIN_EMAIL` → `bootstrap_admin.py --env prod` creates the group, the first cycle and an admin invite that only that verified email can redeem (via Google). The `admin-bootstrap` client becomes dev-only, so prod has no password auth ([`auth.md` §9](../../docs/design/auth.md#9-bootstrap-admin--first-group)).
6. **Manual promotion gate.** One `deploy.yml`: `main` auto-deploys dev, then `promote-prod` runs behind the `production` environment and deploys the backend, then the frontend, from the same SHA. API changes are expand → contract, and the service worker updates promptly ([`testing-and-ci.md` §6](../../docs/development/testing-and-ci.md#6-github-actions-workflows), `CLAUDE.md`, [`coding-standards.md` §8](../../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
7. **Cost target ≈ $1–2/mo** (it was ≈ $8–9 as built): one secret per env, prod-only alarms within the free 10, one account-wide $5 budget in `CiStack`, and DynamoDB on an AWS-owned key ([`infrastructure.md` §12](../../docs/design/infrastructure.md#12-cost-expectations)).
8. **Media deletes are real**: S3 objects, then the row. No bucket versioning. Permanent-delete warnings in the UI ([`media.md` §8](../../docs/design/media.md#8-cleanup-paths), [`api.md` §9.4](../../docs/design/api.md#94-delete-uploadsimageid)).
9. **Shared-device privacy is accepted as-is.**
10. **Settings changes apply to the current cycle where possible.** The voting cycle is rescheduled regardless of candidates. An open cycle's close moves if it's still in the future, otherwise 422 ([`lifecycle.md` §6](../../docs/design/lifecycle.md#6-settings-changes-mid-cycle)).
11. **Flat comments, Web Push and the admin UI are confirmed.** Threaded comments and poll comments/reactions go to [`future-ideas.md`](../../docs/future-ideas.md).
12. **Per-PR `ci-pr-*` stacks** move to [`future-ideas.md` §3](../../docs/future-ideas.md#3-ephemeral-per-pr-ci-stacks).

**Fixed in code:**
- **Skipped month.** With `responseWindowDays = 28` in a non-leap February, the window closes Mar 1 00:00. `lambda-cycle-tick` anchored the next cycle on `response_close_at`, so it created April and skipped March. It now anchors on the current cycle's `cycleId` (`shared::cycle_time::cycle_month_start`, [`lifecycle.md` §5.4](../../docs/design/lifecycle.md#54-create_next_voting_cyclegroup-after)). `response_open_at` was the suggested fix, but the dev tick's `advanceCycleClosesBy` rewinds a voting cycle's `response_open_at` by any amount, which could give the "next" cycle the current cycle's own id. The put would then fail as a lost race, and the group would have no voting cycle. Regression test: `cycle_time::a_full_length_february_window_does_not_skip_march`. `lambda-questions/tests/lifecycle.rs` now derives its cycle id from its dates instead of hardcoding `202606`, and asserts that the seeded cycle is the following month.
- **Poll labels.** Duplicates are now rejected case-insensitively (Unicode lowercase of the trimmed label), as the original [`engagement.md` §3.3](../../docs/design/engagement.md#33-validation) said. Both the backend and the frontend compared case-sensitively, and the doc had been changed to match the code. "Yes" and "yes" in one poll is confusing, so the code changed instead. Changed in `lambda-questions` validation, `SuggestQuestionForm` and the mock API, with tests in each.
- Verified: 448 backend tests (+4) and 293 frontend Vitest tests pass. fmt, clippy (`-D warnings`), typecheck, ESLint, Prettier and `vite build` are clean.

**Bug hunt (2026-10-08), fixed in code** (alongside the in-flight M13.5 work, in separate edits):
- **The lifecycle could stop.** If `create_next_voting_cycle` failed after promotion committed, nothing retried it, so the group never got another cycle. Each tick now repairs a missing successor for every `open` cycle ([`lifecycle.md` §5.4](../../docs/design/lifecycle.md#54-create_next_voting_cyclegroup-after)). Regression test in `lambda-questions/tests/lifecycle.rs`.
- **Image decode bomb.** `lambda-image-process` had no decode limits, and GIFs collected every frame at full canvas size. A small crafted upload could run it out of memory and leave the row `pending` forever. It now has a header-checked budget and streamed GIF frames, and treats an over-budget image as `IMAGE_TOO_LARGE` ([`media.md` §9](../../docs/design/media.md#9-failure-modes), new `shared::config` constants). Unit tests in `process/tests.rs`.
- **Orphan vote on candidate delete.** The admin delete cascaded the votes before deleting the candidate, so a vote cast in between survived and held its voter's tally slot. It now sweeps the votes, deletes the candidate, then sweeps again (`delete_candidate_and_votes`). The first sweep keeps a failed delete retryable, because the candidate still exists for the retry to find. The second catches a vote cast between the first sweep and the delete. After the delete, a late vote fails its `vote_count` condition.
- **Push hardening.** `resolveNotificationUrl` now falls back to the app origin for any off-origin URL. The push-endpoint allowlist rejects non-443 ports.
- **Owner decisions on the open items (2026-10-11), all resolved:**
  - **PR code could read user data.** The PR `cdk diff` job runs the branch's own code, and `GithubDiffRole` could assume the CDK lookup role (`ReadOnlyAccess`: `dynamodb:Scan`, `s3:GetObject`; on the AWS-owned table key its `kms:Decrypt` deny no longer protects the table). The diff role now assumes no bootstrap role. It keeps only `DescribeStacks`/`GetTemplate` on the prod stacks and `CiStack`; the app makes no context lookups. `infra-ci` runs `cdk diff --no-change-set`, and `test_diff_role_policy_only_reads_stack_templates` pins it ([`testing-and-ci.md` §6](../../docs/development/testing-and-ci.md#6-github-actions-workflows)).
  - **Actions are SHA-pinned.** Every `uses:` across the five workflows is pinned to a full commit SHA with a `# vX.Y.Z` comment, at the release its major tag pointed to on 2026-10-11. `.github/dependabot.yml` bumps them in one grouped weekly PR. The convention is in `.github/workflows/README.md`.
  - **The per-container membership cache is accepted.** Up to `MEMBERSHIP_CACHE_TTL_SECONDS` (60s) of stale access after removal or demotion is fine.

**Code review (2026-10-08), fixed in code** (`/code-review` of the branch; regression tests where noted):
- **A bound invite could be redeemed through Facebook.** PreSignUp set `autoVerifyEmail` for every federated signup, so Facebook users also had `email_verified=true`. It's now always false, and `email_verified` comes only from Google's attribute mapping ([`auth.md` §3.3](../../docs/design/auth.md#33-presignup-logic)). Nothing had been deployed yet, so no existing users carry the old value.
- **A settings save could undo a promotion.** The voting-cycle reschedule was an unconditional put of a copy read earlier. It's now conditioned on `status = voting` ([`lifecycle.md` §6](../../docs/design/lifecycle.md#6-settings-changes-mid-cycle)). Tests: `persistence/tests/newsletters.rs::reschedule_voting_cycle_*`.
- **A failed candidate-delete cascade stranded votes.** This reverses the bug-hunt fix above, keeping its race guarantee: sweep votes, delete the candidate, sweep again ([`api.md` §6.5](../../docs/design/api.md#65-admin-moderation-routes) note).
- **Drafts listing only deleted images auto-published empty.** The close sweep now publishes a text draft with no body only if one of its images still exists and hasn't failed processing. Test: `auto_publish.rs` (u5).
- **An image deleted mid-processing left processed objects behind.** When `mark_*_ready` fails because the row is gone, image-process deletes the variants it just wrote. Test: `lambda-image-process/tests/handler.rs::an_image_deleted_mid_processing_leaves_no_processed_objects`. A narrower window remains: if the deleter reads the row while it's pending and image-process marks it ready before the row delete lands, the variants are still orphaned.
- **The successor repair read every open group's newsletter partition each tick.** It's now one GSI2 query for all `voting` cycles ([`lifecycle.md` §5.4](../../docs/design/lifecycle.md#54-create_next_voting_cyclegroup-after)).
- **Concurrent avatar replacements could orphan one.** `update_profile` returns the replaced `avatar_media_id` from the write itself (`UPDATED_OLD`), and `PATCH /me` deletes that one.

**Code review follow-ups (2026-10-09), owner decisions applied:**
- **Broken drafts are retried, then skipped.** The sweep carries on past a failing draft, counts the failure on the cycle row and keeps the cycle `open`. After `AUTO_PUBLISH_MAX_FAILED_SWEEPS` (3) such ticks, it publishes without those drafts ([`lifecycle.md` §5.3](../../docs/design/lifecycle.md#53-publishnl)). An unreadable answer row no longer fails its whole question (`responses::list_answers_lenient`). Giving up emits the `AutoPublishAbandoned` EMF metric. The cycle-tick alarm is now `FILL(errors,0) + FILL(abandoned,0)`, which uses all 10 free alarm metrics ([`infrastructure.md` §9.2](../../docs/design/infrastructure.md#92-alarms)). Test: `auto_publish.rs::a_broken_draft_holds_the_cycle_for_a_few_ticks_then_is_skipped`.
- **Clearing the avatar deletes it.** `PATCH /me` with `avatarMediaId: null` deletes the old avatar, the same as replacing it ([`api.md` §2.3](../../docs/design/api.md#23-patch-me)/§9.7). The SPA's follow-up `DELETE /avatars` is kept for older backends and is now a 204 no-op. Tests in `avatar_replacement.rs`.
- **Field errors: one name per field, the reason in `code`.** The D10 422 is now `field: "responseWindowDays"`, `code: "CLOSES_IN_PAST"`. `fieldErrors[].code` is a `FieldErrorCode` enum in `openapi.yaml`, mirrored by `domain::FieldErrorCode` and checked by the contract test. The settings form switches on the code instead of a special dotted field name ([`api.md` §1.1](../../docs/design/api.md#11-error-format)). This is safe without expand→contract because the dotted name never shipped.
- **Flaky DynamoDB Local start-up fixed.** With many containers starting at once, the first request often died with `DispatchFailure`/`IncompleteMessage`. The image's wait condition fires before the server listens, and Colima's port forwarder accepts connections early. `test_factories::make_test_repo` now:
  - polls `ListTables` until the server answers (up to 60s)
  - sets SDK connect/attempt timeouts so a dead connection can't hang a test
  - starts at most 4 containers at a time per test process

  `persistence`'s own tests now use the same factory (a dev-dependency on itself with `test-utils`), so the duplicate setup is gone. A 6-binary concurrent stress run that failed every round before is clean while the machine stays awake.

**Code review follow-ups (2026-10-11):**
- **A `responseWindowDays` change could reopen a closed cycle.** A cycle past its close but not yet published is no longer moved; the setting applies from the next cycle. `reschedule_open_cycle_close` is also conditioned on `response_close_at > now` ([`lifecycle.md` §6](../../docs/design/lifecycle.md#6-settings-changes-mid-cycle)). Tests: `group_settings.rs::a_cycle_past_its_close_is_not_reopened_by_a_longer_window`, `::rescheduling_refuses_a_cycle_whose_close_has_passed`.
- **The narrower mid-processing delete window (above) is closed.** The media row delete is conditioned on the status the deleter read; on a lost race it rereads and deletes the new variants ([`media.md` §8.0](../../docs/design/media.md#80-explicit-deletion)). Tests: `media-delete/tests/delete.rs::an_image_that_finishes_processing_mid_delete_loses_its_processed_objects_too`, `persistence/tests/media.rs::an_image_row_delete_refuses_a_row_whose_status_changed`.
- **Promotion read only the first page of candidates.** `list_candidates` now follows `LastEvaluatedKey`, and the tick skips an unreadable candidate instead of failing the promotion ([`lifecycle.md` §5.2](../../docs/design/lifecycle.md#52-promote_and_opennl)).
- **A failed poll save left the unsaved pick showing.** `RespondPage` puts the selection back on the last saved option. Test: `RespondPage.test.tsx` "puts the pick back on the saved option when a save fails".
- **The emoji picker was the only lazy chunk**, and D6's prompt activation drops the old build's precache, so an open tab couldn't load it after a deploy. It's now bundled eagerly, and ESLint (`no-restricted-syntax` on `ImportExpression`) forbids lazy chunks in `src/`.
- Cleanups: the successor repair's two GSI2 queries run concurrently and the `voting` one projects only `group_id`. `MediaDeleter::from_env` replaces three copies of the deleter setup. `list_top_candidates_by_votes` (AP11) is removed. `Response::has_text_body` is the one "non-empty body" rule.

---

## M13.5 detail — what landed (2026-10-08)

The lead read the decided sections first. Before spawning agents, they settled three gaps and recorded each one: the media-delete crate location, the string-typed `email_verified` claim and the backend↔infra env-var contract. Three agents ran in parallel: backend, infra and frontend. The first Sonnet launches all died on repeated API 500s before writing anything, so they were relaunched on Opus, and a session rate limit then paused them mid-task. The lead reviewed the risky diffs, fixed the frontend fixtures the codegen broke, and re-ran every check.

### Backend
- **D2, auto-publish.**
  - `Response::is_non_empty` is the single predicate, shared by the sweep and notify-tick's reminder `N`.
  - `persistence::responses::auto_publish_draft` runs one transaction per draft (`status = draft`), with the M13 `last_answered_cycle` bump guard. A refused bump is retried without it, and a lost race is retried as-is.
  - `publish()` sweeps before the status flip; any error skips the flip, so the next tick redoes the sweep.
- **D3:** `DEFAULT_RESPONSE_WINDOW_DAYS = 7`.
- **D4:** `CandidateQuestion::promotion_order` is the one comparator (votes desc, oldest first, then id), used by both promotion and `sort=top`. Promotion reads the whole pool.
- **D5, bound invites.**
  - `Invite.bound_email` (`#[serde(default)]`), plus `boundEmail` on create and list.
  - 403 `INVITE_EMAIL_MISMATCH`.
  - `shared::http::is_true_claim` parses the claim.
- **D7:** `APP_SECRET_ARN` is one JSON secret, read by push, notify-tick and media. Unknown keys are ignored.
- **D8, real deletion.**
  - **New lib crate `media-delete`:**
    - the `MediaObjectStore` trait
    - an S3 implementation
    - a recording fake
    - `delete_image_media`/`delete_avatar_media`
  - `persistence::media` deletes the rows; the `"DELETED"` soft mark is gone.
  - **Callers:**
    - `DELETE /uploads` and `DELETE /avatars`: 204 when the row is already gone
    - `PATCH /me` avatar replacement and comment-image release: best effort
- **D10:**
  - The "no candidates" gate is gone.
  - New `newsletters::reschedule_open_cycle_close`, guarded by `status = open`.
  - A 422 on `responseWindowDays` (code `CLOSES_IN_PAST` since the 2026-10-08 review) is validated before any write.
- **Contract:** `shared/openapi.yaml`, `ROUTE_TABLE` and the regenerated `frontend/src/types/api.ts`.

### Infrastructure
- **D7, one secret:** `config.app_secret_arn` replaces the four secret ARNs, and `secret_ref()` builds the dynamic references.
- **D7, cost:**
  - DynamoDB uses `TableEncryption.DEFAULT`.
  - Prod-only alarms: **9 alarm metrics**. Dev keeps the dashboard, with no alarms and no topic.
  - One $5 budget in `CiStack` (80% actual, 100% forecasted).
  - `config.get()` now treats an empty env value as unset, because GitHub renders a missing `vars.X` as `""`.
- **D5:**
  - The Google `email_verified` mapping.
  - `admin-bootstrap` exists in dev only, and the prod authorizer audience is the frontend client alone.
  - `bootstrap_admin.py --env prod`:
    - writes the group, the first cycle and a bound admin invite in one conditional transaction
    - `created_by` is the sentinel `"bootstrap"`
    - exits without `BOOTSTRAP_ADMIN_EMAIL`
- **D8:**
  - `s3:DeleteObject` scoped to the [`media.md`](../../docs/design/media.md) key prefixes.
  - No bucket versioning anywhere.
- **D6, `deploy.yml`:**
  - It replaces `frontend-deploy.yml`/`backend-deploy.yml`.
  - Lambdas are built once, tarred to keep the exec bit, and promoted unchanged.
  - Dev reads `DEV_*` variables, and prod reads the unprefixed ones.
  - New `scripts/check_lambda_binaries.sh` blocks a stub deploy.
  - `GithubDeployRole` also trusts `repo:…:environment:production`, which is what an environment-scoped job's OIDC subject is.

### Frontend
- **D1, sessions:**
  - `userStore` moved to `localStorage`.
  - `logout()` follows [`auth.md` §8](../../docs/design/auth.md#8-logout), including a best-effort, 3-second `/oauth2/revoke` against the issuing client.
  - `env.ts` treats `VITE_COGNITO_BOOTSTRAP_CLIENT_ID` as optional and never set in prod. `/admin/bootstrap-login` is routed only when it's enabled.
- **D2:**
  - Every poll pick saves as a draft.
  - The auto-publish note and badge copy.
  - `cacheSavedResponse` fixes a poll pick missing from the cached detail.
- **D8:** permanent-delete confirms for the response editor, comment images and avatars. The response editor runs strip → save → `DELETE`.
- **D10:** "applies now" copy, and the inline 422 on the window field.
- **D6:** `activatePromptly` (`skipWaiting` + `clients.claim`), not `autoUpdate`, which reloads the page mid-session.
- **D5:** the `INVITE_EMAIL_MISMATCH` message on the Join page.

### Decisions (recorded in [`infrastructure.md` §6.2](../../docs/design/infrastructure.md#62-lambda-functions-request-handlers), [`api.md` §9.7](../../docs/design/api.md#97-avatar-routes), [`frontend.md` §7.3](../../docs/design/frontend.md#73-respondpage-ggncrespondq)/§14.2, [`auth.md` §4.2](../../docs/design/auth.md#42-algorithm), [`lifecycle.md` §5.3](../../docs/design/lifecycle.md#53-publishnl), [`media.md` §8.0](../../docs/design/media.md#80-explicit-deletion), [`engagement.md` §4.3](../../docs/design/engagement.md#43-commentlisttsx), [`testing-and-ci.md` §3](../../docs/development/testing-and-ci.md#3-cdk-tests)/§6.4)
- The media-delete helper is its own crate. `shared` can't depend on `persistence`.
- `email_verified` counts as true only for JSON `true` or the string `"true"`.
- The bound-email check runs after the "already a member" short-circuit.
- A departed member's draft auto-publishes without a bump.
- A missing avatar row returns 204.
- The comment-image confirm fires at Save.
- The `production` environment **must** restrict deployment branches to `main`. Its OIDC subject carries no branch.

### Lead review fixes
- Codegen made `boundEmail` required, which broke typecheck in `mockAdmin.ts` and `GroupAdminPage.test.tsx`. The lead added it to the fixtures.
- `.github/workflows/README.md` called the `main`-only branch rule optional ("while you're there"). It's now required, with the reason.
- [`runbook.md`](../../docs/operations/runbook.md)'s prod bootstrap command dropped the flags the script ignores in prod.

### Tests
- **Backend:** 486, up from 448. New suites:
  - `lambda-cycle-tick/tests/{auto_publish,promotion}.rs`
  - `lambda-invites/tests/bound_invites.rs` (the crate's first integration tests)
  - `lambda-groups/tests/{group_settings,avatar_replacement}.rs`
  - `media-delete/tests`
  - the lambda-media delete tests
  - parser unit tests

  A few of the 486 belong to the parallel bug-hunt session. The first full run hit DynamoDB Local connection drops at container start, which were not test bugs. That start-up flakiness has since been fixed in `test_factories::make_test_repo` (see "Code review follow-ups"), so a plain `cargo test --workspace` is reliable. Verified 2026-10-11: two back-to-back runs at default parallelism (CLAUDE.md's command, as `backend-ci` runs it) passed 493/493.
- **Code-review follow-ups (2026-10-08/09):** backend 493 (+7), CDK 156 (+1). Vitest stays at 331; existing tests were updated for the field-error change.
- **CDK:** 155, up from 86. New `test_config.py` and `test_bootstrap_admin.py`. `test_monitoring_stack.py` was rewritten.
- **Frontend:** 331 Vitest (up from 293), and 8 Playwright (up from 7; the new test covers a poll draft plus the auto-publish note).

---

## B5/B6 blocker history (as of 2026-10-11)

### B5 — M1 operator tasks outstanding

**Updated 2026-10-05.** Progress so far: domain decided (`betterloop.site`, bought on Namecheap — DNS stays on Namecheap's nameservers, no Route53 hosted zone, so ACM validation CNAMEs get added there manually when `cdk deploy` prints them); Cognito domain prefix picked; AWS CLI credentials fixed to use IAM user `bquinn17` via `aws login` (was briefly root — see note below) with `AdministratorAccess` + `SignInLocalDevelopmentAccess`; `cdk bootstrap` done (`CDKToolkit` stack `CREATE_COMPLETE`); `cargo-lambda` installed on macOS (clears part of B6); VAPID keypair generated and stored at `opennewsletter/vapid/dev` (required fixing `scripts/generate_vapid_keys.py` — the `py-vapid` API it called, `public_key_urlsafe_base64()`/`private_key_urlsafe_base64()`, doesn't exist in the installed 1.9.4; replaced with direct RFC 8292 raw-point encoding via `cryptography`); `ALARM_EMAIL` set.

**Re-assessed 2026-10-07.** All three secret ARNs in `.env.local` (`opennewsletter/oauth/google/dev`, `…/oauth/facebook/dev`, `…/vapid/dev`) now carry the random suffix AWS adds when a secret is created, so the secrets have been created. Their contents couldn't be checked: the `aws login` session had expired. What remains:
- ~~**Combined dev secret (M13.5, D7).**~~ **Done 2026-10-11.** `opennewsletter/dev` was built from the three old dev secrets: OAuth `clientId`/`clientSecret` → `google_*`/`facebook_app_*`, and VAPID `publicKey`/`privateKey`/`subject` → `vapid_*`. The migration refused to run on any missing or empty field; the secret was read back and compared. `.env.local` now holds only `APP_SECRET_ARN` (the three old `*_SECRET_ARN` lines are gone). `cdk synth --context env=dev` resolves all 11 secret references to `opennewsletter/dev`, and `pytest infra/tests` passes (157). The three old secrets are **scheduled for deletion with a 7-day recovery window (until 2026-10-18)**. If the first dev deploy shows a problem with the new secret, restore one with `aws secretsmanager restore-secret --secret-id <name>`.
- **`.env.local` also applies to prod, and it overrides environment variables** (`load_config`). A prod synth or deploy from a workstation would therefore pick up the *dev* `APP_SECRET_ARN`. CI has no `.env.local`, so it isn't affected. Before a local prod bring-up, make `.env.local` dev-only or override it.
- **Confirm the OAuth secrets hold real client IDs and secrets.** The migration showed the fields are present and non-empty, but not that the values are valid. Also confirm that each IdP app lists the Cognito hosted-UI redirect `https://<prefix>.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`.
- ~~Re-run `aws login`~~ (done 2026-10-07 as `bquinn17`; these sessions expire, so re-check with `aws sts get-caller-identity` before deploying).

These don't block code work. They block every deploy gate (M3–M13).

**Decision — Apple IdP dropped (2026-10-05).** Sign In with Apple requires a paid Apple Developer membership ($99/yr); the owner declined. Removed `UserPoolIdentityProviderApple` from `AuthStack`, `apple_oauth_secret_arn` from `EnvConfig`, and the Apple step from [`operator-setup.md`](../../docs/operations/operator-setup.md); updated the CDK test (`test_user_pool_two_idps`, was `_three_idps`) and every plan doc that listed three IdPs ([`overview.md`](../../docs/overview.md), [`infrastructure.md`](../../docs/design/infrastructure.md), [`build-order.md`](../build-order.md), [`environments.md`](../../docs/development/environments.md), [`original-spec.md`](original-spec.md)) to Google/Facebook only. `cdk synth --context env=dev` and `pytest infra/tests/` (54 passed) both verified green after the change. If Apple support is wanted later, re-add the IdP construct, the 4 config fields (`apple_client_id`/`apple_team_id`/`apple_key_id`/`apple_private_key_secret_arn` per [`infrastructure.md` §4.1](../../docs/design/infrastructure.md#41-cognito-user-pool), collapsed to one `apple_oauth_secret_arn` in the actual `config.py`), and the operator-runbook step.

**Note — root-credential near-miss.** `aws login` was initially run while signed into the AWS Console as the **root** user, so it issued short-term rotating keys under the root identity rather than an IAM user. `aws login` works with any identity you authenticate as in the browser; it doesn't itself pick a non-root one. Fixed by attaching `AdministratorAccess` + `SignInLocalDevelopmentAccess` to `bquinn17` and re-running `aws login`, signing in as that IAM user. Worth calling out in [`operator-setup.md`](../../docs/operations/operator-setup.md) §"AWS account and credentials" if that section gets revised.

### B6 — Lambda release builds: stale artifacts on macOS, no release build on Windows/WSL2

**Windows/WSL2**: `cargo build --release` (and therefore `cargo lambda build --release`) aborts in `aws-lc-sys`, which refuses to compile under gcc 9 because of [gcc bug 95189](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189). That host is Ubuntu 20.04 with only gcc 9.4 and no clang. Debug builds and the test suite are unaffected. Fix: `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain; failing both, switch rustls to the `ring` backend.

**macOS**: the gcc issue doesn't apply (Apple clang), and `cargo-lambda` 1.9.2 is installed. Artifacts in `backend/target/lambda/` (re-checked 2026-10-07) are stale:
- `cycle-tick`, `notify-tick` and `push-api` date from 2026-10-06 19:18. `push-api` predates the M13 endpoint allowlist.
- The rest date from 2026-10-04, before M10–M13. `responses-api` lacks the M13 double-count fix.
- **`engagement-api` still has no artifact**, so a deploy now would ship the stub for it.

Run `make build-lambdas` immediately before any deploy (`make deploy-dev` already does). **New in M13:** `backend-ci` cross-builds every Lambda on Linux. Its first run is also the first Linux build of `image-process` (`libwebp-sys`).

**Windows/WSL2** has never produced a Lambda artifact. `ApiStack`/`NotificationsStack` fall back to the shell stub when `backend/target/lambda/{binary}/bootstrap` is missing, which is why synth passes — deploying before this is cleared would ship stubs.

---

## Known gaps and next steps as of 2026-10-11

Snapshot of the live list when this log was archived; the live, pruned list is in `../PROGRESS.md`.

### Suggested next steps (snapshot)

1. **Finish the M1 operator tasks** to clear B5:
   - confirm the Google/Facebook secrets hold real client credentials and that both IdP apps list the Cognito `/oauth2/idpresponse` redirect
   - ~~(M13.5, D7) fold the three dev secrets into `opennewsletter/dev`~~ (done 2026-10-11, B5). Deploy dev before 2026-10-18, when the old secrets' recovery window closes.
   - **(M13.5, D5) set `BOOTSTRAP_ADMIN_EMAIL`** for prod
   - **(M13.5, D6) GitHub settings** per `.github/workflows/README.md`:
     - repo variables: `DEPLOY_ENABLED`, `DEV_APP_SECRET_ARN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, `FRONTEND_DOMAIN`, and optionally the `DEV_*` domain variables
     - the `production` environment, with a required reviewer and a `main`-only deployment-branch rule
     - delete the old `*_SECRET_ARN` variables, `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED`, and the `VITE_COGNITO_BOOTSTRAP_CLIENT_ID` secret

   `opennewsletter/dev`, VAPID, `cdk bootstrap` and `cargo-lambda` (macOS) are done. See [`docs/operations/runbook.md`](../../docs/operations/runbook.md).
2. **Rebuild every Lambda artifact** with `make build-lambdas` (`make deploy-dev` does it first). `push-api`, `notify-tick` and `cycle-tick` were built on 2026-10-06 and the rest on 2026-10-04, so all of them predate M13. `engagement-api` has never been built (B6).
3. *(done 2026-10-05: CDK bootstrap)*
4. **Deploy and verify M3–M5**: `make deploy-dev && make seed && make fe` (see [`runbook.md`](../../docs/operations/runbook.md)). M4 gate: `bootstrap_admin.py` then `curl -H "Authorization: Bearer $TOKEN" "$API_ENDPOINT/me"`. M5 gate: the five-step manual flow in [`build-order.md`](../build-order.md) M5 using `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy`.
5. **M6 gate**: after deploy, the curl save → save → publish dance against `PUT …/my-response` ([`build-order.md`](../build-order.md) M6).
6. **M7 gate**: `make fe`, then:
   - sign in with Google
   - redeem an invite made by `POST /admin/invites` and see the group on Home
   - log out
   Also try `/admin/bootstrap-login` with the `bootstrap_admin.py` credentials.
7. **M8 gate**: after deploy, `cargo lambda build --release --arm64` must succeed for `image-process`. Its `libwebp-sys` C build cross-compiled on macOS on 2026-10-04. Its first Linux build is `backend-ci`'s first run. Then upload a 5 MB JPEG through `ImageUploader`, see the thumbnail, reload, and confirm it renders via a presigned S3 URL.
8. **M9 gate**: after deploy, run a simulated month in the UI:
   - suggest and vote on candidates
   - `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy` to promote and open
   - draft with images, reload, then publish text and poll answers
   - tick again to publish, then read the edition
9. **M10 gate**: after deploy, two users comment on each other's published answers (one with an image), edit and delete a comment, and react with a ZWJ emoji (e.g. 👨‍👩‍👧‍👦) and a flag. An admin deletes the other user's comment. Check that API Gateway hands the Lambda the emoji path param decoded or encoded; the handler accepts both ([`api.md` §8.6](../../docs/design/api.md#86-put-reactionsemoji)).
10. **M11 gate**: after deploy, `cargo lambda build --release --arm64` for `push-api`, `notify-tick` and `cycle-tick` — all three cross-compiled cleanly on macOS 2026-10-06, but they predate M13's push allowlist, so rebuild them (step 2). Then:
   - enable notifications in Settings on a desktop Chrome and an installed iOS PWA; "Send test push" reaches both, and "My devices" lists both
   - `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy` to open a cycle → the cycle-open push arrives once (retick: no duplicate)
   - rewind into the reminder window and `POST /admin/dev/tick/notify` → one reminder (the smallest due offset), not a burst
   - tick to publish → the publication push arrives; tapping it opens the edition
   - turn a group's "Deadline reminders" off and confirm that member is skipped
   - check the `PushFailureRate` alarm and the `PushSent`/`PushFailed` EMF metrics exist in CloudWatch
11. **M12 gate**: after deploy, as a group admin open Settings → Your groups → Manage:
   - Invites: create a member invite, copy the URL, and open it in another browser signed in as a second Google account → the new member appears on the Members tab, and the invite shows "used by {name}"
   - make them admin, then member again; remove them through the confirm dialog
   - Settings: rename the group and change its gradient → Home and the group switcher pick it up without a reload
12. **M13 gates**:
   - Push a branch and confirm `frontend-ci`, `backend-ci` and `infra-ci` go green on GitHub. They've only been checked locally and with `actionlint`. The first real run of `cargo lambda build --release --arm64` on Linux, including `libwebp-sys`, happens in `backend-ci`.
   - After deploy, deploy `CiStack` (it's in `--all`), then paste its two role-ARN outputs into the repo variables listed in `.github/workflows/README.md`.
   - Run [`smoke-test.md`](../../docs/operations/smoke-test.md) against dev.
   - Confirm the SNS email subscription (prod only since M13.5). Dev has no alarms; prod has 10 alarm metrics (the free-tier limit, since the 2026-10-09 cycle-tick change), which should sit in `OK`. Confirm the `CiStack` budget email too.
   - Fill in the `e2e/dev/` Playwright bodies once a Cognito test user exists (`E2E_USERNAME`/`E2E_PASSWORD`).
13. ~~**M13.5**~~ (code-complete 2026-10-08; see "M13.5 detail"). Its deploy-shaped checks join the M14 gate: a bound-invite redeem via Google, an auto-publish at a real close, a media delete that 403s immediately afterwards, and the first `promote-prod`. The code-review follow-ups add two more ([`runbook.md`](../../docs/operations/runbook.md) M13/M14): a Facebook sign-up gets `email_verified=false`, and the `AutoPublishAbandoned` EMF metric reaches CloudWatch and is read by the `CycleTick-Errors` alarm.
14. **M14** (production deploy).

Workstation setup for either machine: [`environments.md` §0](../../docs/development/environments.md#0-workstation-setup).

### Known gaps carried forward (snapshot)

**From M13.5 (2026-10-08):**
- **The fixed `BottomNav` covers RespondPage's bottom bar** ("Save & exit", "Publish") at 700px. Both are `fixed bottom-3 z-40`. The new e2e test works around it with `goBack()`. Pre-existing; fix the stacking.
- **Prompt SW activation can strand an old tab's lazy chunk.** The new SW's precache cleanup may remove the previous build's `EmojiPickerPopover` chunk while an old tab is open, so that tab's emoji picker fails until it reloads ([`frontend.md` §14.2](../../docs/design/frontend.md#142-service-worker)).
- **A poll pick can't be cleared**, since `pollOptionId` is required. So a poll pick always auto-publishes at close. Decide whether polls need a "clear" action ([`frontend.md` §7.3](../../docs/design/frontend.md#73-respondpage-ggncrespondq)).
- **The D10 open-cycle close move is best-effort.** It runs after the group update, and a write error is logged at ERROR rather than returned, the same as the voting reschedule. The 422 check runs before any write.
- **The account-level Lambda error alarm also counts dev's errors**, since both envs share one account ([`infrastructure.md` §9.2](../../docs/design/infrastructure.md#92-alarms) as written). Its description says so.
- **Existing `sessionStorage` sessions aren't migrated** to `localStorage`. Every user signs in once more after M13.5 ships.

- **Five more `cargo deny` advisory ignores have no upstream fix.** Three are `rustls-webpki` 0.101 issues, pinned by `aws-smithy-http-client`'s legacy rustls 0.21 branch; they're reachable only via a mis-issued certificate. The other two are test-only (`h2` 0.3, `tokio-tar`). `deny.toml` also ignores two unmaintained-crate notices (`paste`, `rustls-pemfile`), which aren't vulnerabilities. Re-check after AWS SDK bumps.
- **Fonts come from Google Fonts**, so the CSP allows `fonts.googleapis.com`/`fonts.gstatic.com` and the fonts aren't available offline. Self-host them if that matters ([`frontend.md` §12.1](../../docs/design/frontend.md#121-content-security-policy)).
- **Mock-mode E2E can't cover reload-persistence of saved drafts.** The mock transport is in-memory, so the test checks the localStorage crash-recovery mirror instead. The real save → reload path is a `dev`-project test.
- **`CiStack` deploys with every `--all`** (dev or prod). It's account-wide and idempotent, but `make deploy-dev` touches it too.
- **A crash mid-fan-out loses the rest of that fan-out** (markers are claimed first, at-most-once — [`notifications.md` §7.1](../../docs/design/notifications.md#71-algorithm)). Accepted for now; the alternative risks duplicate pushes.
- **Fan-outs are sequential.** Fine for small groups (60s push timeout, 120s notify-tick); parallelise with a bounded `join_all` if groups grow.
- **PWA icons are generated placeholders** (`frontend/scripts/generate-icons.mjs`). Swap in designed artwork.
- **Comment author names: no cross-request cache.** [`engagement.md` §1.5](../../docs/design/engagement.md#15-author-display-name-resolution) suggests a 60s per-cold-start user cache. M10 does one deduped `BatchGetItem` per request instead. Add the cache if user reads show up as hot.
- **`get_comment_by_id` walks the answer's whole `C#` range with a filter**, because there's no index on `comment_id`. That's fine at expected thread sizes; revisit if threads get long.
- **The comment composer's image upload and the `[deleted]` placeholder** are only exercised in mock mode and component tests, since there's no deploy yet (M10 gate).
- **No invite pagination.** `GET /admin/groups/{g}/invites` takes no cursor and always returns `nextCursor: null`. Every invite comes back, including consumed/revoked ones until their DynamoDB TTL (expiry + 30 days) removes them. Fine at expected volumes.
- **`RespondPage` has no markdown formatting toolbar** (the legacy page had bold/italic/list buttons). The spec only requires Write/Preview. Add it if users ask.
- **No "hype check" banner** ([`frontend.md` §14a](../../docs/design/frontend.md#14a-client-derived-display-data-not-api-fields) `utils/hype.ts`) and no recurring-question helper text (`utils/recurring.ts`). Both are optional decoration and not built.

The post-M7 gap pass (2026-09-27, above) cleared most of the earlier list. What remains:

- **`VOTE_CAP_REACHED` lists the already-voted questions in `detail`** (free text), not a structured field. `fieldErrors` now exists but is per-field validation, not a cap conflict.
- **`lambda-cycle-tick` needs `#![recursion_limit = "256"]`** since it gained a `lib.rs`. Harmless, but if it creeps up again, box the dispatch futures.
- **[`data-model.md`](../../docs/design/data-model.md) carries 5 "Note (reconciled 2026-09-27)" flags** for discrepancies the doc sweep didn't resolve by renaming. Four are attributes the code writes but [`data-model.md`](../../docs/design/data-model.md) omits: `cognito_sub` on the sub lookup, and the denormalized IDs on `CandidateVote`/`Comment`/`Reaction`. The fifth (the `NOTIFIED#…`/`TICK#NOTIFY` rows) is resolved: M11 writes them.

---

## Design review detail — what landed (2026-10-08, merged 2026-10-12)

A whole-system review for performance, cost and security ahead of prod. Overall it rated **B+**: sound, cheap architecture and strong transactional correctness. The weak spots were request paths whose latency grew with group size, list queries that silently truncated at DynamoDB's 1 MB page, and the fact that nothing had been deployed yet. Built on `bquinn/design-review-perf` from `d0d81b5` and rebased onto `4698ebd` on 2026-10-12.

**Fixed in code** (decisions recorded in [`infrastructure.md` §5.1](../../docs/design/infrastructure.md#51-s3-buckets-mediapersistentstack)/§6.4/§12, [`data-model.md` §3](../../docs/design/data-model.md#3-access-patterns--index-map), [`api.md` §5.2](../../docs/design/api.md#52-get-groupsgroupidnewsletterscycleid)):
- **Published-edition read was a serial N+1.** It made about six sequential DynamoDB calls per answer, which at the 50-member cap with image-heavy answers would have exceeded the 10 s Lambda timeout. It now reads in four concurrent waves ([`api.md` §5.2](../../docs/design/api.md#52-get-groupsgroupidnewsletterscycleid)), using the new `engagement::list_engagement` and `media::get_images_batch`. A new handler test pins the output: answer order, draft exclusion, image order with non-`ready` images skipped, comment images, `[deleted]` placeholders and poll tallies.
- **List queries didn't paginate.** `persistence::query::query_all` now follows `LastEvaluatedKey` for every whole-set Query ([`data-model.md` §3](../../docs/design/data-model.md#3-access-patterns--index-map)). Regression test: 60 answers at the body cap (about 1.2 MB) all come back. M13.5 had independently added page loops to `list_candidates`, `list_answers` and the GSI2 status queries; the rebase folded those into `query_all`, keeping M13.5's lenient-listing variants.
- **`find_voting_cycle`/`find_open_cycle` read the group's whole history** on every candidate request. They now read the 3 newest `NL#` rows ([`data-model.md` §3](../../docs/design/data-model.md#3-access-patterns--index-map) invariant), with a test behind nine published cycles.
- **Smaller latency wins:**
  - `GET …/newsletters` runs its per-cycle AP15 reads concurrently.
  - `PUT …/my-response` batches its image checks into one `BatchGetItem` and overlaps its three pre-reads. Error precedence is unchanged.
  - Push and notify-tick fan-outs read each member's pref, responses and subscriptions concurrently (16 in flight, `PUSH_MEMBER_READ_CONCURRENCY`). The notify tick keeps M13.5's D2 rule that a non-empty draft counts as answered.
- **`CONFIG_JSON` removed.** `groupDefaults` on `GET /config` now comes from `shared::config::default_group_settings`. The hand-copied env var in `api_stack.py` had drifted once already (`responseWindowDays`). The M13.5 CDK test for the 7-day default moved to a `shared::config` unit test. New `AUTOSAVE_DEBOUNCE_MS`.
- **No Intelligent-Tiering on the processed buckets.** The objects are under the 128 KB floor, so each transition fee bought nothing ([`infrastructure.md` §5.1](../../docs/design/infrastructure.md#51-s3-buckets-mediapersistentstack)). Originals buckets unchanged. CDK tests pin both.

**Verified after the rebase:** backend 505 (498 + 7 new) and CDK 158 (157 − 1 moved + 2 new) pass. fmt, clippy (`-D warnings`), ruff (check + format), `mypy --strict .` and `cdk synth` (dev + prod) are clean. Frontend untouched.

**Checked:** the Lambda account concurrency limit is **50** (dev and prod share it).

**Open owner questions** are listed in PROGRESS.md. A fifth, trimming PROGRESS.md, was done separately on 2026-10-11.

---

## Media serving via presigned S3 — what landed (2026-10-14/15)

Media is served directly from S3 via presigned GET URLs, signed by each Lambda's own IAM role ([`decisions.md`](../../docs/decisions.md#presigned-s3-media)), with originals deleted once processed ([`decisions.md`](../../docs/decisions.md#originals-deleted-on-success)). Built with parallel agents (backend, infra, frontend, docs). The lead finished the backend after its agent hit a rate limit.

**Backend.**
- New `media-urls` crate: `MediaUrls::{image_url, avatar_url}` presign GETs for the one processed bucket, with the start time floored to `MEDIA_URL_SIGN_WINDOW_SECONDS` (5 min) and `MEDIA_URL_TTL_SECONDS` (1 h). Media, newsletters, engagement, groups and questions each hold one `MediaUrls` in their `AppState`. Unit tests pin `X-Amz-SignedHeaders=host` (no `x-amz-checksum-mode`, which would 403 every browser GET) and identical URLs within a window.
- `media-delete` deletes the S3 objects, then the row.
- lambda-image-process writes image and avatar variants to the one `PROCESSED_BUCKET`. It deletes the original once the ready update wins (best effort, ERROR log) and on decode failure, and writes `Cache-Control: private`.

**Infra.**
- Three buckets: processed CORS is GET/HEAD for the app origin, and both originals buckets expire `uploads/` after 7 days.
- New `MediaBaseUrl`/`UploadOrigins` outputs, built as `{bucket}.s3.{region}.amazonaws.com` so they match the SDK's presigned host and the CSP.
- `s3:GetObject` is prefix-scoped per signing Lambda.
- The monitoring dashboard has a free S3 storage widget.
- `write_frontend_env.py` writes `VITE_MEDIA_BASE_URL`/`VITE_UPLOAD_ORIGINS`; `deploy.yml` reads them as secrets, like the other prod `VITE_*` values.

**Frontend.**
- `MediaImage` (`crossorigin="anonymous"`, lazy, width/height). On the first error per object path it refreshes once (`refreshMediaUrls` invalidates active queries, throttled to once per 10 s; `ImageUploader` re-GETs its upload), then shows a placeholder. `Avatar` falls back to initials the same way.
- The SW cache `media-images` is keyed by origin + path, 200-only. The CSP's `connect-src` gains the upload origins, which fixes a latent bug where the prod CSP would have blocked presigned upload PUTs.

**Bugs caught in lead review.**
- The Newsletters Lambda was granted `s3:GetObject` on `img/*` only, though it signs asker and comment-author avatars too; every avatar in a published edition would have 403'd.
- `media-urls` compiled only with its `test-utils` feature on (imports used only by the test constructor), which the workspace build hid through feature unification.
- The S3 monitoring widget labelled both originals buckets "originals".
- Three requested CDK tests were missing; they were added: processed CORS per env, `GetObject` scoping, and the outputs.

**Spec decisions.** Avatars are signed too, at no extra cost, so no bucket needs a public path ([`media.md` §11.3](../../docs/design/media.md#113-serving)). The media secrets stay GitHub *secrets*, matching the convention for the other prod `VITE_*` values.

**Code-review fixes (2026-10-18).**
- Inline answer images and comment images were sized by the original's dimensions (say 4032×3024) while showing the 400px thumb, so they stretched to the container. Thumbs now get `thumbSize`'d dimensions.
- `MarkdownBody` rebuilt its `components` every render, so React remounted every inline `MediaImage` and reset its retry state. They're now module-level, with the data passed through a context.
- `MediaImage`'s failed placeholder was a span that ignored `width`/`height` and could collapse to a dot. It's now an `<img>` of a transparent `data:` SVG, so it keeps the image's box.
- `useMediaRetry` replaces the copy of the retry state machine in `Avatar`. `media-urls` gained `ready_image_urls` and `optional_avatar_url`, which replace 3 and 5 copies.
- lambda-image-process skips an original that's already gone (a duplicate S3 delivery after another invocation finished) instead of erroring into a retry ([`media.md` §5.1](../../docs/design/media.md#51-trigger-configuration)). The failure-path original delete is best effort, like the ready path.
- `promote-prod` checks the prod `VITE_*` secrets after `cdk deploy` and before the frontend build.
- After these: backend 498 (+2), Vitest 343 (+6).

Owner follow-ups on the review's other findings, same day:
- `refreshMediaUrls` now refetches only queries flagged `MEDIA_URLS_META` (`me`, group, newsletter, candidates and RespondPage's image query), not every query, and does nothing while offline. One missing image offline used to raise an error toast for every query on screen.
- Avatars are cached for 90 days ([`decisions.md`](../../docs/decisions.md#avatars-cached-90-days)): a separate `media-avatars` SW cache, and `Cache-Control: max-age=7776000` on the processed object.
- Gap found while doing that: after an avatar change, `useUpdateProfile` refreshed only `me` and `config`, so the members list and candidates showed the old avatar until their `staleTime` ran out. A photo or colour change now invalidates the media-URL queries.
- Left as is, by owner call: presign errors stay 500s; per-mention avatar presigning stays; lambda-media keeps its two S3 clients. `ResponseEditor`'s duplicate preview refresh is a known gap in [`PROGRESS.md`](../PROGRESS.md).
- Vitest 346 (+3).

**Tests.** Backend 496, CDK 151, Vitest 337, Playwright 8. fmt, clippy `-D warnings` (workspace and `media-urls` standalone), ruff, `mypy --strict`, typecheck, ESLint, Prettier, `vite build`, `actionlint` and `cdk synth` (dev + prod) are clean.

**Found, not fixed:** `cargo deny check advisories` now fails on new upstream rustls advisories RUSTSEC-2026-0346/0347/0348/0349 (rustls 0.21.12 and 0.23.45). Lockfile versions are unchanged, so this predates the change.

## `cargo deny`: rustls advisories cleared (2026-10-18)

RUSTSEC-2026-0346 to 0349 (published 2026-10-14) failed `cargo deny check` on two rustls versions.
- **rustls 0.23.45** (the stack every Lambda actually uses) moved to the patched 0.23.46 with `cargo update -p rustls@0.23.45`.
- **rustls 0.21.12** had no patched release. It came only from the AWS SDK crates' default `rustls` feature, which compiles in the legacy hyper 0.14 client. Every `load_defaults` passes `BehaviorVersion::latest()` (2026-01-12), and at that version `aws-smithy-runtime` picks `default-https-client` (hyper 1 / rustls 0.23) whenever it's enabled, so the legacy client was dead code. The workspace now declares `aws-sdk-dynamodb`, `-s3`, `-lambda` and `-secretsmanager` (the last moved into `[workspace.dependencies]`) with `default-features = false` and the SDK's other defaults kept. That removed rustls 0.21, hyper 0.14, hyper-rustls 0.24, `rustls-webpki` 0.101, `sct` and `h2` 0.3 from the lockfile (13 packages).
- With those crates gone, four `deny.toml` ignores no longer matched anything and were removed: RUSTSEC-2026-0098, -0099, -0104 (`rustls-webpki` 0.101) and RUSTSEC-2026-0258 (`h2` 0.3).

**Checks.** `cargo deny check` passes (advisories, bans, licenses, sources). `cargo fmt --check`, clippy `--all-targets --all-features -D warnings` and `cargo test --workspace` (498 passed) are clean.

## Dev deploy — what landed (2026-10-18, follow-ups 2026-10-26)

First `make deploy-dev`; all nine stacks are up (last deployed 2026-10-19 02:03 UTC). M3's deploy and M7 are confirmed against dev; the other gates are still to run.

**Bugs found bringing dev up.**
- **`ApiStack-dev` failed to create.** The access-log format had `correlationId: $context.requestHeaderOverride.header.x-correlation-id`. HTTP APIs don't allow header variables in access logs at all, and only the deploy rejects it: `cdk synth` passes. The field is gone, and a CDK test fails if the format reads a request header. The Lambdas still log `correlation_id`, which is what the Logs Insights query searches.
- **`make seed` had no Python dependencies.** It ran the system `python3`, which has no `boto3` and which Homebrew won't `pip install` into. New `scripts/requirements.txt` (`boto3[crt]`, `py-vapid`) is installed into `infra/.venv` next to `infra/requirements.txt`, and `make seed` activates that venv. `[crt]` is needed because botocore can't load `aws login` credentials without it. The setup docs and the scripts' "not installed" messages give the install command.
- **Every request from the seeded admin 500'd** ("failed to resolve caller"). `seed_dev_data.py` wrote the `CognitoSubLookup` item without `cognito_sub`, which the Rust struct requires. `bootstrap_admin.py` already wrote it.
- **`AuthCallbackPage` swallowed its error**, so a failed first Google sign-in couldn't be diagnosed. It now logs it with `console.error`. The failure didn't recur, so it was most likely a reload of a used callback URL or a stale tab.
- **Settings 401'd for a user who hadn't redeemed an invite.** `GET /me` refuses callers with no User row, by design, and the client treated each 401 as an expired token and renewed (so the 401s came in pairs).

**Decisions.**
- Spec gap: before an invite is redeemed, Settings redirects to Home (which shows the join form), and `useMe` runs only once `/config` has a `userId` ([`frontend.md` §7.6](../../docs/design/frontend.md#76-settingspage)). The client's 401-means-expired assumption is a known gap in [`PROGRESS.md`](../PROGRESS.md).
- `make seed` gives the bootstrap admin a fresh random 20-character password on every run (meeting the pool policy: lowercase, uppercase, digit, symbol from `-_.%`), unless `--admin-password` is given. The password is set on every run, but the user is never recreated, so its sub and the rows derived from it stay stable. Consequence: the `dev` E2E user can't be the seeded admin ([`testing-and-ci.md` §4.3](../../docs/development/testing-and-ci.md#43-playwright-e2e)).
- 2026-10-26: `seed_dev_data.py` and `bootstrap_admin.py` run on Python 3.8 again (the WSL2 venv). `UTC` is `timezone.utc` rather than `datetime.UTC` (3.11+), and `zoneinfo` falls back to `backports.zoneinfo` below 3.9 (in `scripts/requirements.txt` with a version marker). Re-run the WSL2 venv's `pip install` to pick it up. Checked with `ast` 3.8 syntax parsing and an import under Python 3.9; not yet run on a real 3.8.

**Tests.** CDK +1 (access-log format), Vitest +1 (Settings redirect); the admin-page test now mocks `/config`. Current baselines are in [`PROGRESS.md`](../PROGRESS.md).

## Demo mode — what landed (2026-10-29)

`/demo` serves the whole app on the in-memory mock, from the deployed build: "See a demo" on the signed-out home screen, and any `/demo/...` URL can be shared. Design in [`frontend.md` §17](../../docs/design/frontend.md#17-demo-mode); decision in [`decisions.md`](../../docs/decisions.md#demo-mode).

**What landed.**
- `src/demo.ts` decides demo-or-not once per page load from the URL. The router gets `basename: "/demo"`; `AuthProvider` and `api/client.ts` switch on `isMockMode` (mock build or demo) instead of `env.useMocks`.
- `utils/appStorage.ts`: the current group, the draft mirror and the iOS-banner flag go to an in-memory store in the demo, so it never reads or writes the real `localStorage` keys.
- No writes: `rawFetch` refuses to run in the demo, `uploadToS3` fakes the PUT in mock mode, `isPushSupported()` is `false` in the demo, and sign-out there only loads `/`.
- `DemoBanner` on every demo page, with Sign in (a real sign-in, which leaves the demo) and Exit demo; a signed-in visitor is told these aren't their groups. A sign-in in another tab (a new `oidc.user:*` key) ends the demo.
- Demo invite links are `/demo/join?code=…`.

**Bug fixed on the way.** Image and avatar uploads failed in local mock mode (`npm run dev`): the mock's `mock://upload/…` presign was PUT for real. `uploadToS3` now fakes it in mock mode.

**Decisions.** A signed-in visitor can use the demo; their session is left alone. Demo state lives in memory instead of being cleared on exit: no exit path can leave anything behind, and a blanket clear would sign a real user out.

**Tests.** Vitest +21 (366 → 387): `demo.test.ts`, `AuthProvider.demo.test.tsx`, `DemoBanner.test.tsx`, plus demo cases in the HomePage and NotificationsSection tests. Playwright +6: a new `demo` project that builds `.env.e2e` with mocks off into `dist-e2e-demo` and serves it on :4174.

## Demo photos and uploads (2026-10-29)

Every image in the demo was a black box: the mock served one 1×1 black PNG for every image, stretched to fit. Design in [`frontend.md` §17](../../docs/design/frontend.md#17-demo-mode).

**What landed.**
- Uploads show the picked file. The fake upload keeps it in memory as a `blob:` URL (`api/mockUploads.ts`), and the mock serves it, with its real size, for responses, comments and avatars. The progress bar now fills over 1.2–4 s, depending on the file's size.
- Three freely licensed Wikimedia Commons photos in `public/demo-media/`, with `CREDITS.txt`, on the seeded published edition: the Mailbox Peak mailbox, a ridge at sunset (newly added to Sam's answer), and a hiker on a summit.

**Bugs fixed on the way.**
- `mockFetch` returned its live fixture objects, so TanStack Query's cache and the mock shared arrays. A new comment was pushed by the mock and then appended again by `useAddComment`, showing twice after the refetch. This affected every comment in mock mode and the demo. `mockFetch` now returns a JSON copy, like the network.
- A site-relative image URL crashed the newsletter page: `mediaCacheKey` runs `new URL()` on it. The sample photos use absolute URLs.

**Tests.** Vitest +2 (387 → 389): an upload's `blob:` URL reaches its upload row and a comment and is freed on delete, and `mockFetch` returns copies. Playwright +2 (14 → 16): the sample photos load, and a comment's uploaded image is the picked file.
