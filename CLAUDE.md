# CLAUDE.md — repo guidance for AI assistants

This file is loaded into every Claude Code conversation in this repo. Keep it short.

## Required reading before writing code

1. [`plans/00-overview.md`](plans/00-overview.md) — system map, glossary, repo layout
2. [`plans/coding-standards.md`](plans/coding-standards.md) — style, linting, testing, error handling, naming, comments, commits, PR conventions

A milestone or task is not "done" until its work conforms to those standards. The pre-flight checklist at `coding-standards.md` §8 applies before opening any PR.

## Running the Rust tests (read this before `cargo test`)

The `persistence` integration tests start a DynamoDB Local container via `testcontainers`, so they need a reachable Docker daemon whose socket is *not* `/var/run/docker.sock`. The repo is developed on two machines with different setups — **check `uname` to see which one you're on**. Agent shells are non-login and non-interactive, so they don't read shell profiles: **every** test command must set `PATH`/`DOCKER_HOST` inline.

**macOS (Apple Silicon)** — Rust via rustup (`~/.cargo/bin`), Docker via Colima, Node and Python 3.12 via Homebrew:

```bash
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" DOCKER_HOST=unix://$HOME/.colima/default/docker.sock TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE=/var/run/docker.sock && gtimeout 1800 cargo test --workspace 2>&1
```

macOS has no `timeout`; use `gtimeout` (Homebrew `coreutils`). If `docker info` fails, the Colima VM is stopped: `colima start`.

**Windows (WSL2, Ubuntu 20.04)** — Docker runs rootless under WSL2:

```bash
export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && timeout 1800 cargo test --workspace 2>&1
```

Notes:
- Env vars do not persist between tool calls — repeat the `export` in each command, don't run it once on its own.
- Always wrap in `timeout` (the suite pulls an image on a cold cache and can otherwise hang).
- Sanity-check the daemon first with the same exports plus `docker info | grep "Server Version"`. `SocketNotFoundError` panics from `tests/common/mod.rs` mean the daemon is down or `DOCKER_HOST` is wrong, not that the tests are broken.

**Frontend checks** (from `frontend/`; no Docker needed; on macOS prefix `export PATH="/opt/homebrew/bin:$PATH" &&`):

```bash
npm run typecheck && npx eslint . && npx prettier --check . && npm test && npm run build
```

**CDK synth** (from `infra/`; same command on both machines, but on macOS prefix `export PATH="/opt/homebrew/bin:$PATH" &&`):

```bash
source .venv/bin/activate && npx aws-cdk@2 synth --quiet
```

Use exactly `npx aws-cdk@2`. Don't use `npx cdk` or a global `cdk` install. When npx runs a globally installed binary it puts that binary's directory first on `PATH`, and on macOS that's `/opt/homebrew/bin`, whose `python3` then shadows the venv's. Synth then fails with `No module named 'aws_cdk'` even though the venv is fine.

Full first-time setup for each machine: [`plans/13-dev-environments.md`](plans/13-dev-environments.md) (which also covers which test layers need Docker at all).

## What this app is

OpenNewsletter is a multi-tenant PWA where small groups collaboratively produce a monthly newsletter. Members suggest and upvote candidate questions between cycles; on the 1st of each month the top-voted questions promote into an active newsletter and a 4-day response window opens; at close, the edition auto-publishes and comments/reactions become available.

## Architectural principles (must hold)

- **Idle backend.** Unauthenticated traffic never reaches Lambda. Static frontend on GitHub Pages; API Gateway only after Cognito issues a JWT.
- **Single repo, IaC-first.** Frontend, backend, infra, and ops scripts live here. CDK is source of truth for AWS resources.
- **Single-table DynamoDB.** All durable state lives in `OpenNewsletter` with two GSIs. Multi-tenancy is enforced via partition-key prefixes — see [`plans/02-data-model-dynamodb.md`](plans/02-data-model-dynamodb.md).
- **Strict tenant isolation.** Every group-scoped Lambda handler resolves caller memberships from the JWT and refuses any request whose group ID is not in that set. No exceptions.
- **Best-judgment defaults.** Numeric tunables (debounces, image dimensions, vote counts) live in `backend/crates/shared/src/config.rs` so they change in one place.

## Stack at a glance

Frontend: React 18 + TS + Vite + Tailwind, GitHub Pages.
Auth: Cognito User Pool, hosted UI, Google/Facebook IdPs. (Apple dropped — see PROGRESS.md decisions; Apple Developer membership is $99/yr.)
API: API Gateway HTTP API + Cognito JWT authorizer → Rust Lambdas (`provided.al2023`, `cargo-lambda`).
Data: DynamoDB on-demand single table.
Media: S3 + CloudFront with signed cookies.
Notifications: Web Push (VAPID) directly from Lambda.
Scheduling: EventBridge (cycle tick + notify tick).
IaC: AWS CDK in Python.

## Glossary (use these terms exactly)

Group, Member, Cycle/Newsletter, Candidate question, Locked question, Response/Answer, Comment, Reaction, Invite, Push subscription. Definitions in [`plans/00-overview.md`](plans/00-overview.md) §5.

## Build order

[`plans/12-build-order.md`](plans/12-build-order.md) is the milestone sequence an executor should follow. Each milestone names the plan docs to consult and the done-when gate.

## Picking up work

**Start with [`plans/PROGRESS.md`](plans/PROGRESS.md).** It records:
- which milestone is next
- what each finished milestone landed
- decisions made along the way
- active blockers
- known gaps carried forward

Update it at the end of every milestone; its last section says how.

Standing rules learned the hard way:
- **API contract changes:** every new or changed route updates [`shared/openapi.yaml`](shared/openapi.yaml) and `ROUTE_TABLE` in `backend/crates/domain/tests/openapi_contract.rs` in the same change. Then re-run `scripts/codegen_types.sh` to regenerate `frontend/src/types/api.ts`.
- **Crate template:** new Lambda crates copy the layout of `lambda-questions` / `lambda-responses`: `lib.rs`, `router.rs`, `handlers.rs`, `state.rs`, `validation.rs`, and DynamoDB Local tests under `tests/`. The binary is named `<name>-api`. Wire it in `infra/opennewsletter/api_stack.py` and add a `FUNCTION_<binary>` entry to the `Makefile`.
- **Key timestamps:** every timestamp in a key attribute goes through `persistence::keys::key_timestamp`.
- **Test group IDs:** integration tests use unique group IDs, because the membership cache is process-wide.
- **Spec gaps:** when you fill a gap in the spec, record the decision in the owning plan doc and in PROGRESS.md's "Decisions" list for that milestone.

## Delegating to parallel agents

The owner prefers that work be delegated to cheaper sub-agents, running in parallel where possible, with the lead agent planning, reviewing and verifying. The pattern that has worked (M5, M6):

1. **Lead reads the spec first.** Read the milestone's plan sections and settle ambiguities *before* spawning agents. Write the decided semantics into the agent prompt verbatim, and put them into the plan docs yourself while the agents run.
2. **Split by directory so agents never edit the same files.** Typically:
   - a **backend agent** (Sonnet) covering `backend/`, `shared/openapi.yaml` and the generated `frontend/src/types/api.ts`
   - an **infra agent** (Haiku is enough) covering `infra/` and the `Makefile`
   - a **docs agent** if plan docs need sweeping

   Run them in the background, in parallel. Tell each one which paths it may *not* touch, and that others are working concurrently, so it doesn't "fix" their changes.
3. **Every agent prompt must include:**
   - **Search scope:** "Stay inside this repo directory (or below) for every search, grep, find, ls and read — never `$HOME` or `/`." An earlier agent searched far too widely.
   - **A template to mirror:** point at an existing crate or stack as the pattern.
   - **The platform-correct verify command:** copied from the section above, with env vars inline.
   - **The baseline test count** to compare against.
   - **"Do not commit."**
   - **What to report back:** files changed, test counts, spec contradictions hit, and anything unfinished.
4. **Lead verifies; don't take an agent's word for it.**
   - Read the diff of the risky core, such as a transaction or a state transition.
   - Re-run `cargo fmt --check`, clippy with `-D warnings`, the full workspace tests, `pytest infra/tests`, `ruff check`/`ruff format --check` and `cdk synth`, plus the frontend checks above when `frontend/` changed.
   - Then update PROGRESS.md.

Commit only when the owner asks.

## When something is ambiguous

If a plan document seems wrong or contradicts another, **stop and surface it** rather than guessing. Plan docs are intended to be edited as we learn. For low-stakes gaps: pick the simplest default, document it in the appropriate plan file in the same PR, and continue.
