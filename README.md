# OpenNewsletter

A multi-tenant PWA where small groups of friends collaboratively produce a newsletter: monthly in production, weekly on dev. See [`OpenNewsletter_concept.md`](OpenNewsletter_concept.md) for the product story and [`docs/`](docs/README.md) for the documentation, starting with the [overview](docs/overview.md).

## Repo layout

- `frontend/` — React + TypeScript + Vite + Tailwind PWA
- `backend/` — Rust workspace; one crate per Lambda plus shared libraries
- `infra/` — AWS CDK in Python
- `shared/openapi.yaml` — API contract; both sides codegen from it
- `scripts/` — bootstrap, codegen, dev seed
- `docs/` — documentation: design, development and operations ([index](docs/README.md))
- `plans/` — project tracking: progress, milestone build order, history

## Status

Pre-release. Milestones M0–M13.5 are code-complete. Dev is live at [dev.betterloop.site](https://dev.betterloop.site), and every push to `main` redeploys it. Production isn't deployed yet: it waits on the remaining operator setup, and the production deploy (M14) is next. See [`plans/PROGRESS.md`](plans/PROGRESS.md) for where things stand and [`plans/build-order.md`](plans/build-order.md) for the milestone sequence.

## Local development

This repo is developed on macOS (Apple Silicon) and Windows/WSL2 (Ubuntu 20.04), with different first-time toolchain setup on each. Follow [`docs/development/environments.md` §0](docs/development/environments.md#0-workstation-setup) once before running anything below.

**Frontend, no backend needed.** `npm run dev` serves the app against an in-memory mock API (`VITE_USE_MOCKS=true` in `.env.development`), so you can work on the UI without AWS:

```sh
cd frontend && npm install && npm run dev        # http://localhost:5173
```

**Backend.** The integration tests share one DynamoDB Local container, so they need Docker. Start it with `make dynamodb-local` and stop it with `make dynamodb-local-stop`. On WSL2, `DOCKER_HOST` also needs setting; see [`docs/development/environments.md` §10.1](docs/development/environments.md#101-docker-and-dynamodb-local-by-platform):

```sh
make dynamodb-local && cd backend && cargo test --workspace
```

**Infra.**

```sh
cd infra && python3.12 -m venv .venv && source .venv/bin/activate \
  && pip install -r requirements.txt -r ../scripts/requirements.txt \
  && npx aws-cdk@2 synth --quiet
```

**Against the real dev stack** (needs AWS credentials): merging to `main` deploys it, and `make deploy-dev` deploys from your machine. Then `make seed && make fe`. See [`docs/operations/runbook.md`](docs/operations/runbook.md).

## Contributing

1. Branch from `main`.
2. Make sure the checks CI runs pass locally. The exact commands, with per-machine environment variables, are in [`CLAUDE.md`](CLAUDE.md) under "Running the Rust tests" and "Frontend checks":
   - **Backend**: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo deny check`, `cargo test --workspace`
   - **Frontend**: `npm run typecheck && npx eslint . && npx prettier --check . && npm test && npm run build`, plus `npm run e2e` (Playwright against a mock-mode build)
   - **Infra** (from `infra/`): `ruff check . ../scripts && ruff format --check . ../scripts && mypy --strict . && pytest tests/ && npx aws-cdk@2 synth --quiet`
3. If you change an API route, update [`shared/openapi.yaml`](shared/openapi.yaml) and the contract test's `ROUTE_TABLE`, then run `scripts/codegen_types.sh`.
4. Open a PR. `frontend-ci`, `backend-ci` and `infra-ci` run on the paths you touched, and `e2e` runs nightly. Merging to `main` deploys dev. They're described in [`.github/workflows/README.md`](.github/workflows/README.md).

The pre-PR checklist is in [`coding-standards.md` §8](docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task).

## Documentation

[`docs/README.md`](docs/README.md) indexes everything. Required reading for any contributor (human or agent):
1. [`docs/overview.md`](docs/overview.md): architecture, stack, repo layout, glossary
2. [`docs/development/coding-standards.md`](docs/development/coding-standards.md)

Operators deploying the stacks start at [`docs/operations/`](docs/operations/): one-time setup, the runbook and the smoke test.

See [`CLAUDE.md`](CLAUDE.md) for AI-assistant guidance in this repo.

## Reporting issues / feedback

Open an issue against the repo, or follow the project's internal channels.
