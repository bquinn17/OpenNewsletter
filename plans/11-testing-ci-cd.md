# 11 — Testing & CI/CD

This document defines what we test, how we test it, and what runs in CI.

---

## 1. Test pyramid by component

```
                    ┌──────────────────────┐
                    │   E2E (Playwright)    │  small set of golden-path flows
                    │   ~10 tests           │
                    └──────────┬───────────┘
                ┌──────────────┴──────────────────┐
                │  Integration (Rust + DDB-local)  │  one test per access pattern + lifecycle
                │  ~80 tests                       │
                └──────────────┬──────────────────┘
        ┌──────────────────────┴────────────────────────┐
        │   Unit (Rust + Vitest)                         │ pure functions, components
        │   ~hundreds                                    │
        └────────────────────────────────────────────────┘
```

---

## 2. Rust backend tests

### 2.1 Unit (in-crate)

In `domain/`, `persistence/keys.rs`, and `shared/`. No IO. Run on every `cargo test`.

Mandatory unit-test surfaces:
- `keys.rs` — every key-builder function (gold-master strings)
- `shared::config` — env parsing
- `shared::auth::JwtVerifier` — JWKS handling (with a test JWK)
- Any pure validation function (markdown allowlist, emoji predicate, image MIME check, etc.)

### 2.2 Integration (`backend/tests/`)

Run against a containerized DynamoDB Local. Each test wires up a fresh table at startup, populates fixtures, executes the access pattern, and asserts the result.

Per-access-pattern coverage from `02-data-model-dynamodb.md` §3 (AP1–AP21): ≥1 test per pattern.

Per-transaction coverage from `02-data-model-dynamodb.md` §4: ≥1 test per transaction; for the high-stakes ones (vote tx, publish-response tx, redeem-invite tx) include concurrency tests using `tokio` joins.

Lifecycle integration suite (matches `06-newsletter-lifecycle.md` §11):
1. Tick promotes top-N candidates correctly.
2. Tick auto-publishes at deadline.
3. Tick is idempotent under rapid re-runs (run loop 5x, assert single transition).
4. Empty-candidate cycle still opens and publishes empty.
5. Settings change mid-cycle applies to N+2.
6. Membership cap enforced.
7. Concurrent vote-cast and admin-delete-candidate produce consistent state.
8. DST boundary in `America/New_York` cycle creation.

Image-pipeline integration suite (matches `08-media-uploads.md` §10) running against `lambda-image-process` directly with sample fixtures in `backend/tests/fixtures/images/`.

### 2.3 Contract test

A test that loads `shared/openapi.yaml`, then for every defined route checks that the corresponding Rust request/response struct in `domain/api.rs` round-trips with example payloads in the YAML. Any drift fails CI.

### 2.4 Running locally

By default, `cargo test` runs **unit tests only** — pure, no IO, no Docker, no AWS. This is the inner loop, intended to run on every save (`cargo watch -x test`).

Integration tests (the DDB-local-backed suite in §2.2) are gated behind a feature flag and are **CI-only by default**. Developers don't need Docker or DynamoDB Local on their machines. See [`13-dev-environments.md` §10](13-dev-environments.md) for the full test-placement matrix.

```bash
cd backend

# Inner loop (default): unit + contract tests, ~sub-second.
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Opt-in: integration tests against DDB-local. Requires Docker.
# Used to debug a failing CI integration test locally.
cargo test --features integration
cargo test --features integration --test lifecycle   # one suite
```

When `--features integration` is passed, the `testcontainers` crate auto-starts a DDB-local container per test process — no manual `docker run` needed. One container per test binary, parallelism via per-test unique table names.

CI runs `cargo test --features integration` on every backend PR, so coverage of the access-pattern + transaction tests is unchanged from the original plan; only the developer's local default has shifted.

---

## 3. CDK tests

`infra/tests/test_stacks.py` using `aws_cdk.assertions`. For each stack, assert:
- Expected resource counts (e.g. exactly 8 `AWS::Lambda::Function` in `ApiStack` — the eight request handlers in `01-infrastructure-cdk.md` §6.2; the tick Lambdas live in `NotificationsStack` and `lambda-image-process` in `MediaPipelineStack`).
- Cognito user pool has exactly two app clients (`frontend` + `admin-bootstrap`).
- IAM policy templates for each Lambda role match the documented least-privilege scope.
- API routes match `03-api-contract.md` (auth attached to all but the explicit list of public ones — and there should be ZERO public routes).
- DynamoDB table has correct keys + TTL attribute + 2 GSIs.
- CloudFront distribution has the trusted KeyGroup attached.
- S3 buckets are private (BlockPublicAccess fully on).

Run via `pytest infra/tests/`.

`cdk synth --context env=dev` runs in CI as a synth-smoke and to populate `cdk.out/` for snapshot diffing.

---

## 4. Frontend tests

### 4.1 Vitest

Component tests with React Testing Library:
- `ResponseEditor` save loop (debounce, flush on blur/hidden, retry on failure — saves are last-write-wins, there is no conflict UI; see `03-api-contract.md` §7.3).
- `PollWidget` state transitions (voting → tally on publish).
- `ReactionBar` toggle.
- Markdown sanitizer (strip script, allow headers, allow `image:` tokens).
- Date utilities in `utils/dates.ts` against fixed clocks for various group TZs.

### 4.2 MSW (Mock Service Worker) for component tests

Rather than mocking individual fetches, use MSW to mock the full API surface based on the OpenAPI spec. Generates pre-canned responses; tests can override per-test.

### 4.3 Playwright E2E

Run against:
- A fully deployed `dev` stack (CI's deploy-and-test job)
- OR a docker-compose stack (DDB-local + a single in-process Rust server) for fast local runs

Golden-path tests (≤10):
1. Sign in → land on home → see no groups → redeem invite → see group.
2. Suggest a candidate question → see it in the list.
3. Upvote a candidate → vote count increments.
4. Cycle opens (test fast-forwards via `POST /admin/dev/tick/cycle`) → see questions.
5. Save draft → close tab → reopen → draft persists.
6. Upload an image → wait for `ready` → image renders in preview.
7. Publish answer → cycle closes → comments enabled.
8. Comment on an answer → see it on reload.
9. React with emoji → toggle off → toggle on different emoji.
10. Upload avatar → poll until ready → `PATCH /me` → avatar renders in member listing.

Time fast-forwarding uses the dev-only `POST /admin/dev/tick/{cycle|notify}` routes (`03-api-contract.md` §11a), which are wired into the API only when `ENV=dev` and refuse with 404 in prod.

---

## 5. Lint, format, type-check

### 5.1 Rust

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo deny check` (license, advisory, source registries)

### 5.2 Frontend

- `eslint .` (with `@typescript-eslint`, `react-hooks`, `import` rules)
- `prettier --check .`
- `tsc --noEmit`
- `stylelint "src/**/*.css"`

### 5.3 Python (CDK)

- `ruff check infra/`
- `mypy infra/`
- `cdk synth --context env=dev` (smoke)

---

## 6. GitHub Actions workflows

### 6.1 `frontend-ci.yml`

Triggers: PRs touching `frontend/**` or shared types.

Steps:
1. Setup Node 20.
2. `npm ci`.
3. `npm run lint` (eslint + prettier + stylelint + tsc).
4. `npm run test` (Vitest with `--coverage`).
5. Upload coverage artifact.

### 6.2 `frontend-deploy.yml`

Triggers: push to `main`, paths `frontend/**`.

Steps:
1. Build (`npm run build` with prod env vars from secrets).
2. Add `CNAME` file with the custom domain.
3. Copy `dist/index.html` → `dist/404.html`.
4. Push `dist/` to `gh-pages` branch (use `peaceiris/actions-gh-pages`).

Secrets needed: `VITE_API_BASE_URL`, `VITE_CDN_BASE_URL`, `VITE_COGNITO_*`.

### 6.3 `backend-ci.yml`

Triggers: PRs touching `backend/**`.

Steps:
1. Setup Rust toolchain (pinned via `rust-toolchain.toml`).
2. Cache `~/.cargo` and `target/`.
3. `cargo fmt --check && cargo clippy ... && cargo deny check`.
4. `cargo test --workspace --features integration` — the `testcontainers` crate starts DynamoDB Local itself (§2.4); the runner just needs Docker, no service-container setup step.
5. `cargo lambda build --release --arm64` (sanity: ensures all crates compile to lambda artifacts).

### 6.4 `backend-deploy.yml`

Triggers: push to `main`, paths `backend/**` or `infra/**`.

Steps:
1. Build all Lambdas via `cargo lambda build --release --arm64`.
2. Configure AWS credentials (OIDC role assumption — no long-lived access keys).
3. `cdk deploy --context env=prod --all --require-approval never`.

### 6.5 `infra-ci.yml`

Triggers: PRs touching `infra/**`.

Steps:
1. Setup Python 3.12.
2. `pip install -r requirements.txt`.
3. `ruff && mypy && pytest infra/tests/`.
4. `cdk synth --context env=dev` (smoke).
5. `cdk diff --context env=prod` against the live stacks (read-only). Posts diff as PR comment.

### 6.6 `e2e.yml`

Triggers: nightly (cron) + manual.

Steps:
1. Spin up a dedicated `e2e-{run-id}` CDK stack OR target the always-on `dev` stack.
2. `npx playwright install chromium`.
3. `npm run e2e`.
4. Upload screenshots/traces on failure.

---

## 7. Pre-commit hooks (recommended, optional)

`.pre-commit-config.yaml` with:
- `cargo fmt --check`
- `cargo clippy` (fast subset)
- `eslint` on staged files
- `prettier`
- `ruff` on staged files

---

## 8. Coverage targets

Numbers, not religion:

| Component | Statement coverage target |
|---|---|
| `domain/`, `persistence/keys.rs` | ≥90% |
| Other backend crates | ≥75% |
| `frontend/src/utils/`, `state/` | ≥85% |
| Frontend components | ≥60% (E2E covers integration) |

Coverage is reported, not gated — failing only if it drops more than 5pp from the previous run.

---

## 9. Test data factories

`backend/crates/persistence/src/test_factories.rs` (gated under `#[cfg(test)]`): builders for every entity, with sensible defaults. Used by all integration tests.

`frontend/tests/factories.ts`: TypeScript counterparts, used by Vitest + MSW.

---

## 10. Fixtures

- `backend/tests/fixtures/images/` — sample JPEG, PNG, WebP, animated GIF, large (16 MB rejection), corrupt.
- `frontend/tests/fixtures/` — sample API responses keyed by route.

---

## 11. Observability validation

A small CI step asserts:
- Every Lambda emits at least one structured log line per invocation (via a CloudWatch Insights query against the `dev` stack).
- The custom EMF metrics namespace `OpenNewsletter/dev` is populated after a smoke run.

This is part of the nightly `e2e.yml`, not per-PR.

---

## 12. Schema migration handling

DynamoDB single-table is intrinsically schema-less, but new entity types or attribute additions still require care.

When adding a new entity / GSI key:
1. Update `02-data-model-dynamodb.md`.
2. Update `keys.rs` with the new key builders.
3. If adding a GSI: deploy CDK first (CDK adds the GSI to the live table; DDB backfills automatically).
4. Then deploy code that reads/writes the new keys.

When changing an existing key shape: don't. Add a new attribute or new key, dual-write, migrate, then remove. Migrations are scripted in `scripts/migrate_<name>.py`.

---

## 13. Manual smoke runbook

After every prod deploy:
1. Sign in.
2. Generate an invite (admin tab).
3. Redeem in incognito with a different account.
4. Suggest a candidate, vote on it.
5. Trigger cycle-tick manually (operator-only Lambda invoke) to advance state.
6. Save a draft, publish, comment, react.
7. Send a test push to self.

A single Markdown checklist `docs/smoke.md` lives in the repo with these steps.
