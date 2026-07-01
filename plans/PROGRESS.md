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
| M3 | Infrastructure baseline (CDK) | ⬜ not started | |
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

Roughly 2 of 14 milestones complete (~12%).

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

## Active blockers

### B2 — Toolchain pin (`rust-toolchain.toml`) bumped from `1.79` → `stable`

`coding-standards.md` §2.1 says "1.79+ stable". With 1.79 active, dependency resolution fails outright: `crypto-common 0.2.2` (transitive via the AWS SDK) requires Cargo's `edition2024` feature, which needs Rust 1.85+. I bumped the channel to `stable` (currently 1.96 on this machine) to make the workspace resolvable. **Action:** decide whether to (a) keep `stable`, (b) pin to a concrete minimum like `1.85`, or (c) pin transitive deps backwards. Then update `coding-standards.md` §2.1 wording to match.

### B3 — DDB-local integration tests require Docker

Integration test files are written (`backend/crates/persistence/tests/*.rs`); all 38 tests compile clean. They require `docker` in `PATH` to run — `testcontainers` pulls `amazon/dynamodb-local` and needs the Docker socket. On this WSL2 instance Docker is not yet installed. **Action:** install Docker Engine in WSL2 (`sudo apt-get install -y docker.io && sudo service docker start`) or enable Docker Desktop → Settings → WSL Integration for this distro, then run `cargo test -p persistence` to exercise all AP and transaction tests.

### B4 — Pre-existing mock-only frontend will need replacement in M7

Commit `91188ba` shipped a rich UI built against in-memory mocks. M7's deliverables (Vite + Tailwind + React Router skeleton wired to the real API + Cognito) overlap heavily with what's already there; expect that work to be partial rewrite, not greenfield.

### B5 — M1 operator tasks outstanding

OAuth app registrations (Google / Apple / Facebook), Cognito hosted-UI domain, and DNS records are still owed by the human operator. These are non-blocking for M2/M3 code work but block any end-to-end auth verification in M4.

---

## Suggested next steps (in order)

1. **Install Docker in WSL2** to clear B3: `sudo apt-get install -y docker.io && sudo usermod -aG docker $USER && sudo service docker start`, then `cargo test -p persistence` from `backend/` to run all 38+15 tests.
2. Resolve B2: edit `coding-standards.md` §2.1 to reflect the chosen toolchain floor (recommend pinning `1.85` as the minimum).
3. Begin M3 (CDK infra baseline).

---

## How to update this file

Each milestone-completion PR should:
- flip its row in the table from ⬜/🟡 to ✅
- delete blockers it cleared
- add new blockers it surfaced
