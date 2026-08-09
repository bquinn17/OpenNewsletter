# Progress & Blockers

Snapshot as of 2026-08-09. Working document — update as milestones complete or
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
| M4 | Auth + bootstrap | ⬜ | Includes **creating `ApiStack`** (per updated `12-build-order.md`). PreSignUp trigger is a pass-through; no PostConfirmation trigger exists. |
| M5 | Lifecycle engine + cycle CRUD | ⬜ | Includes **creating `NotificationsStack`** and wiring the dev tick routes into `ApiStack`. |
| M6 | Responses + drafts | ⬜ | Drafts are last-write-wins — no version-conflict handling (`03-api-contract.md` §7.3). |
| M7 | Frontend skeleton + auth | ⬜ (existing mock UI predates real API) | Commit `91188ba` shipped a rich mock-only UI; will need rework against real endpoints in M7. |
| M8 | Media pipeline | ⬜ | Scope grew on 2026-08-09: avatar buckets + `/avatar/*` CloudFront behavior (`01` §5), size-cap enforcement in `lambda-image-process` (`08` §3.2), dev signed-URL query-param mode (`08` §4.5). |
| M9 | Newsletter UI | ⬜ | |
| M10 | Engagement | ⬜ | |
| M11 | Notifications | ⬜ | |
| M12 | Admin UI | ⬜ | |
| M13 | Hardening | ⬜ | |
| M14 | Production deploy | ⬜ | |

M0 and M2 are complete; M3's `cdk synth`/`pytest infra/tests/` are verified (a real cyclic-stack-dependency bug was found and fixed — see M3 detail), but the actual `cdk deploy` gate in `12-build-order.md` still requires an AWS account and is unverified. M3.5 is code-complete and awaiting the same deploy step. Everything deploy-shaped is blocked on B5 (M1 operator tasks).

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

### M2 follow-ups (from the 2026-08-09 plan reconciliation — fold into M4–M8 work)

- `ImageMedia` entity + `persistence/media.rs`: add the new `purpose: response|comment` attribute (`02` §2.10) — needed by M8/M10.
- `shared/config.rs`: add `MAX_REMINDER_OFFSET_HOURS = 168` and the gradient/avatarColor slug lists (`03` §2.3/§4.3).
- `domain/error.rs`: drop `RESPONSE_VERSION_CONFLICT`; add `LAST_ADMIN` and `CANDIDATE_PROMOTED` if not already present.

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

## Active blockers

### B4 — Pre-existing mock-only frontend will need replacement in M7

Commit `91188ba` shipped a rich UI built against in-memory mocks. M7's deliverables (Vite + Tailwind + React Router skeleton wired to the real API + Cognito) overlap heavily with what's already there; expect that work to be partial rewrite, not greenfield.

### B5 — M1 operator tasks outstanding

OAuth app registrations (Google / Apple / Facebook), Cognito hosted-UI domain, DNS records, the real CloudFront signing keypair, VAPID keys in Secrets Manager, and `cdk bootstrap` of the dev account are still owed by the human operator. These are non-blocking for code work but block the M3/M3.5 deploy gates and any end-to-end auth verification in M4. See [`docs/RUNBOOK.md`](../docs/RUNBOOK.md).

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
5. **Begin M4** (Auth + bootstrap): create `ApiStack`; `lambda-invites` (invite routes + the pass-through PreSignUp trigger binary — no invite logic in the trigger), `lambda-groups` (`/config`, `/me`, `/groups*`), `scripts/bootstrap_admin.py`. Fold in the "M2 follow-ups" listed above while touching those crates.

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
