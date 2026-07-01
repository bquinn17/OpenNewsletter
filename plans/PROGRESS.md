# Progress & Blockers

Snapshot as of 2026-06-29. Working document — update as milestones complete or
blockers resolve. Authoritative milestone definitions live in
[`12-build-order.md`](12-build-order.md).

---

## Milestone status

| # | Milestone | Status | Notes |
|---|---|---|---|
| M0 | Repo skeleton | ✅ done | Layout, READMEs, `.gitignore`, `rust-toolchain.toml`, etc. landed in prior commits. |
| M1 | Operator prerequisites | 🟡 partial | `scripts/generate_vapid_keys.py`, `infra/keys/`, `cf-signing.key` present as untracked files. Google/Apple/Facebook OAuth apps, Cognito domain, and DNS records still owed by the operator. |
| **M2** | **Backend foundations (domain + persistence)** | 🟡 **code-complete, unverified** | See "M2 detail" below. |
| **M3** | **Infrastructure baseline (CDK)** | 🟡 **code-complete, unverified** | See "M3 detail" below. |
| M3.5 | Dev environment online | ⬜ not started | |
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

Roughly 2 of 14 milestones complete (~14%); M3 code-complete and awaiting `cdk synth` verification.

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
- ✅ **Integration test files written** — 38 tests across 9 files in `backend/crates/persistence/tests/`. All compile clean against `testcontainers-modules` / `amazon/dynamodb-local`. **Blocked on Docker** (see B3); tests cannot run until Docker is present in WSL2.

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
- **`opennewsletter/monitoring_stack.py`** — `MonitoringStack` skeleton: SNS alarm topic (+ email subscription if `alarm_email` set), empty CloudWatch dashboard. Metric widgets + alarms land in M13.
- **`infra/keys/cf-signing.pub.pem`** — dev placeholder RSA-2048 public key for CloudFront `PublicKey`. In prod, operator generates real keypair: `openssl genrsa 2048 | openssl rsa -pubout > infra/keys/cf-signing.pub.pem`, uploads private key to Secrets Manager.
- **`backend/lambda-stubs/lambda-image-process/bootstrap`** — shell stub so CDK asset hashing works at synth time; replaced by the real Rust binary in M8.
- **`infra/tests/test_stacks.py`** — 20 CDK assertion tests covering DDB keys/TTL/GSIs, Cognito user pool settings (2 clients, 3 IdPs, no self-signup), S3 Block-Public-Access, CloudFront KeyGroup attachment, Lambda arm64/1024 MB.

### M3 coverage vs. plan

- ✅ `DataStack` — DynamoDB table + 2 GSIs.
- ✅ `AuthStack` — Cognito user pool + 3 IdPs + 2 app clients + hosted UI domain.
- ✅ `MediaPersistentStack` — S3 originals + processed + CloudFront + KeyGroup.
- ✅ `MediaPipelineStack` — `lambda-image-process` stub + S3 event subscription.
- ✅ `FrontendStack` — ACM cert + optional Route53 records.
- ✅ `MonitoringStack` skeleton.
- ✅ `infra/tests/test_stacks.py` — 20 assertion tests written.
- 🟡 **`cdk synth` not yet verified** — requires Python 3.12 + pip (system has 3.8 only). See "Suggested next steps" item 2.

---

## Active blockers

### B3 — DDB-local integration tests require Docker

Integration test files are written (`backend/crates/persistence/tests/*.rs`); all 38 tests compile clean. They require `docker` in `PATH` to run — `testcontainers` pulls `amazon/dynamodb-local` and needs the Docker socket. On this WSL2 instance Docker is not yet installed. **Action:** install Docker Engine in WSL2 (`sudo apt-get install -y docker.io && sudo service docker start`) or enable Docker Desktop → Settings → WSL Integration for this distro, then run `cargo test -p persistence` to exercise all AP and transaction tests.

### B4 — Pre-existing mock-only frontend will need replacement in M7

Commit `91188ba` shipped a rich UI built against in-memory mocks. M7's deliverables (Vite + Tailwind + React Router skeleton wired to the real API + Cognito) overlap heavily with what's already there; expect that work to be partial rewrite, not greenfield.

### B5 — M1 operator tasks outstanding

OAuth app registrations (Google / Apple / Facebook), Cognito hosted-UI domain, and DNS records are still owed by the human operator. These are non-blocking for M2/M3 code work but block any end-to-end auth verification in M4.

---

## Suggested next steps (in order)

1. **Install Docker in WSL2** to clear B3: `sudo apt-get install -y docker.io && sudo usermod -aG docker $USER && sudo service docker start`, then `cargo test --features integration -p persistence` from `backend/` to run all 38+15 tests.
2. **Install Python 3.12 + pip** in WSL2 (system has Python 3.8 only), then `cd infra && pip install -r requirements.txt && cdk synth --context env=dev` to verify M3.
3. **Complete M1 operator tasks** (OAuth apps, VAPID keys, DNS) to clear B5 and enable an end-to-end deploy.
4. Begin M3.5 (Makefile + dev scripts) once `cdk synth` is green.

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
