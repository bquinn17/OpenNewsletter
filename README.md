# OpenNewsletter

A multi-tenant PWA where small groups of friends collaboratively produce a monthly newsletter. See [`OpenNewsletter_concept.md`](OpenNewsletter_concept.md) for the product story and [`docs/`](docs/README.md) for the documentation, starting with the [overview](docs/overview.md).

## Repo layout

- `frontend/` — React + TypeScript + Vite + Tailwind PWA
- `backend/` — Rust workspace; one crate per Lambda plus shared libraries
- `infra/` — AWS CDK in Python
- `shared/openapi.yaml` — API contract; both sides codegen from it
- `scripts/` — bootstrap, codegen, dev seed
- `docs/` — documentation: design, development and operations ([index](docs/README.md))
- `plans/` — project tracking: progress, milestone build order, history

## Status

Pre-release. Milestones M0–M13.5 are code-complete, but nothing is deployed yet: the AWS deploy gates wait on the remaining operator setup, and the production deploy (M14) is next. See [`plans/PROGRESS.md`](plans/PROGRESS.md) for where things stand and [`plans/build-order.md`](plans/build-order.md) for the milestone sequence.

## Local development

This repo is developed on macOS (Apple Silicon) and Windows/WSL2 (Ubuntu 20.04), with different first-time toolchain setup on each. Follow [`docs/development/environments.md` §0](docs/development/environments.md#0-workstation-setup) once before running anything below.

**Frontend, no backend needed.** `npm run dev` serves the app against an in-memory mock API (`VITE_USE_MOCKS=true` in `.env.development`), so you can work on the UI without AWS:

```sh
cd frontend && npm install && npm run dev        # http://localhost:5173
```

**Backend.** The tests include DynamoDB Local integration tests, so they need Docker. On macOS and WSL2 the socket needs pointing at explicitly; see [`docs/development/environments.md` §10.1](docs/development/environments.md#101-docker-socket-by-platform-required-for-any-docker-backed-test):

```sh
cd backend && cargo test --workspace
```

**Infra.**

```sh
cd infra && python3.12 -m venv .venv && source .venv/bin/activate \
  && pip install -r requirements.txt -r ../scripts/requirements.txt \
  && npx aws-cdk@2 synth --quiet
```

**Against a real dev stack** (needs AWS credentials): `make deploy-dev && make seed && make fe`. See [`docs/operations/runbook.md`](docs/operations/runbook.md).

## Contributing

1. Branch from `main`.
2. Make sure the checks CI runs pass locally. The exact commands, with per-machine environment variables, are in [`CLAUDE.md`](CLAUDE.md) under "Running the Rust tests" and "Frontend checks":
   - **Backend**: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`
   - **Frontend**: `npm run typecheck && npx eslint . && npx prettier --check . && npm test && npm run build`, plus `npm run e2e` (Playwright against a mock-mode build)
   - **Infra**: `ruff check . && ruff format --check . && pytest tests/ && npx aws-cdk@2 synth --quiet`
3. If you change an API route, update [`shared/openapi.yaml`](shared/openapi.yaml) and the contract test's `ROUTE_TABLE`, then run `scripts/codegen_types.sh`.
4. Open a PR. `frontend-ci`, `backend-ci` and `infra-ci` run on the paths you touched. They're described in [`.github/workflows/README.md`](.github/workflows/README.md).

The pre-PR checklist is in [`coding-standards.md` §8](docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task).

## Documentation

[`docs/README.md`](docs/README.md) indexes everything. Required reading for any contributor (human or agent):
1. [`docs/overview.md`](docs/overview.md): architecture, stack, repo layout, glossary
2. [`docs/development/coding-standards.md`](docs/development/coding-standards.md)

Operators deploying the stacks start at [`docs/operations/`](docs/operations/): one-time setup, the runbook and the smoke test.

See [`CLAUDE.md`](CLAUDE.md) for AI-assistant guidance in this repo.

## Reporting issues / feedback

Open an issue against the repo, or follow the project's internal channels.
