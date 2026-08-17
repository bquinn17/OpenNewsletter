# OpenNewsletter

A multi-tenant PWA where small groups of friends collaboratively produce a monthly newsletter. See [`OpenNewsletter_concept.md`](OpenNewsletter_concept.md) for the product story and [`plans/00-overview.md`](plans/00-overview.md) for the system map.

## Repo layout

- `frontend/` — React + TypeScript + Vite + Tailwind PWA
- `backend/` — Rust workspace; one crate per Lambda plus shared libraries
- `infra/` — AWS CDK in Python
- `shared/openapi.yaml` — API contract; both sides codegen from it *(created in M5; until then `plans/03-api-contract.md` is the contract)*
- `scripts/` — bootstrap, codegen, dev seed
- `plans/` — design docs and milestone build order

## Status

Pre-release. See [`plans/12-build-order.md`](plans/12-build-order.md) for the milestone sequence.

## Local development

Full local dev instructions land alongside Milestone 3.5 ("Dev environment online"). Until then:

- Frontend: `cd frontend && npm install && npm run dev`
- Backend: `cd backend && cargo build` (requires Rust 1.79+ via `rust-toolchain.toml`)
- Infra: `cd infra && pip install -r requirements.txt && cdk synth --context env=dev`

## Conventions

Required reading for any contributor (human or agent):
1. [`plans/00-overview.md`](plans/00-overview.md)
2. [`plans/coding-standards.md`](plans/coding-standards.md)

See [`CLAUDE.md`](CLAUDE.md) for AI-assistant guidance in this repo.

## Reporting issues / feedback

Open an issue against the repo, or follow the project's internal channels.
