# CLAUDE.md — repo guidance for AI assistants

This file is loaded into every Claude Code conversation in this repo. Keep it short.

## Required reading before writing code

1. [`plans/00-overview.md`](plans/00-overview.md) — system map, glossary, repo layout
2. [`plans/coding-standards.md`](plans/coding-standards.md) — style, linting, testing, error handling, naming, comments, commits, PR conventions

A milestone or task is not "done" until its work conforms to those standards. The pre-flight checklist at `coding-standards.md` §8 applies before opening any PR.

## Running the Rust tests (read this before `cargo test`)

The `persistence` integration tests start a DynamoDB Local container via `testcontainers`, so they need a reachable Docker daemon. On this machine Docker runs **rootless under WSL2** and its socket is *not* `/var/run/docker.sock`. `DOCKER_HOST` is set in `~/.profile`, but agent shells are non-login and non-interactive, so they never read it — **every** test command must export it inline:

```bash
export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && timeout 1800 cargo test --workspace 2>&1
```

Notes:
- Env vars do not persist between tool calls — repeat the `export` in each command, don't run it once on its own.
- Always wrap in `timeout` (the suite pulls an image on a cold cache and can otherwise hang).
- Sanity-check the daemon first with `export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && docker info | grep "Server Version"`. `SocketNotFoundError` panics from `tests/common/mod.rs` mean the daemon is down or `DOCKER_HOST` is wrong, not that the tests are broken.

See [`plans/13-dev-environments.md`](plans/13-dev-environments.md) §10 for which test layers need Docker at all.

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
Auth: Cognito User Pool, hosted UI, Google/Apple/Facebook IdPs.
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

## When something is ambiguous

If a plan document seems wrong or contradicts another, **stop and surface it** rather than guessing. Plan docs are intended to be edited as we learn. For low-stakes gaps: pick the simplest default, document it in the appropriate plan file in the same PR, and continue.
