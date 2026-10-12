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
- Env-derived behaviour in library crates. Since M13 that's `shared::http::error_type_uri_from`; `shared::config` turned out to be constants only, and each Lambda's `main.rs` env reads are entry glue (`coding-standards.md` §2.3).
- Any pure validation function (emoji predicate, image MIME check, push-endpoint host allowlist, field validators, etc.). Markdown sanitising is frontend-only (`coding-standards.md` §3.9) and tested in Vitest.

**Corrected M13:** this list used to require a `shared::auth::JwtVerifier`. No such type exists, and none should: the API Gateway Cognito JWT authorizer verifies issuer, audience, signature and expiry before a Lambda runs (`05-auth-flow.md` §6), and handlers only read the verified claims.

### 2.2 Integration (`backend/tests/`)

Run against a containerized DynamoDB Local. Each test wires up a fresh table at startup, populates fixtures, executes the access pattern, and asserts the result.

Per-access-pattern coverage from `02-data-model-dynamodb.md` §3 (AP1–AP21): ≥1 test per pattern.

Per-transaction coverage from `02-data-model-dynamodb.md` §4: ≥1 test per transaction; for the high-stakes ones (vote tx, publish-response tx, redeem-invite tx) include concurrency tests using `tokio` joins.

Lifecycle integration suite (matches `06-newsletter-lifecycle.md` §11):
1. Tick promotes top-N candidates correctly.
2. Tick auto-publishes at deadline.
3. Tick is idempotent under rapid re-runs (run loop 5x, assert single transition).
4. Empty-candidate cycle still opens and publishes empty.
5. Settings change applies to the current cycle per `06-newsletter-lifecycle.md` §6 (**decided 2026-10-07; lands in M13.5** — supersedes "applies at N+2").
6. Membership cap enforced.
7. Concurrent vote-cast and admin-delete-candidate produce consistent state.
8. DST boundary in `America/New_York` cycle creation.
9. **(decided 2026-10-07; lands in M13.5)** Non-empty drafts (text and poll) auto-publish when the response window closes; empty drafts stay `draft` (D2).
10. **(decided 2026-10-07; lands in M13.5)** A bound invite (`Invite.bound_email`) redeems only when the ID token's `email` matches it (lowercased) and `email_verified` is true; otherwise 403 `INVITE_EMAIL_MISMATCH` (D5).

Image-pipeline integration suite (matches `08-media-uploads.md` §10) running against `lambda-image-process` directly with sample fixtures in `backend/tests/fixtures/images/`. **Decided 2026-10-07; lands in M13.5:** add coverage for real deletion (`DELETE /uploads/{id}` removes the original, both variants, and the row; a repeat `DELETE` is idempotent 204) per `08-media-uploads.md` §8.0 (D8).

### 2.3 Contract test

`backend/crates/domain/tests/openapi_contract.rs` loads `shared/openapi.yaml`, then for every defined route checks that the corresponding Rust request/response struct in `domain/api.rs` round-trips with example payloads in the YAML, via an explicit `ROUTE_TABLE` mapping `(method, path, "request"|status) -> type`. Any drift fails CI, in either direction: a YAML example with no table entry fails, and a table entry with no matching YAML example fails. It also asserts the `GradientSlug`/`AvatarColorSlug` enums in the YAML match `shared::config::GRADIENT_SLUGS`/`AVATAR_COLOR_SLUGS`. Runs under plain `cargo test -p domain` — no Docker.

Landed in M5 alongside `shared/openapi.yaml` itself — see `12-build-order.md` M5 "OpenAPI contract". A route added in a later milestone needs both a YAML example and a `ROUTE_TABLE` row in the same PR, or this test fails.

Each route's YAML entry needs at least one `example` per request and response body; the test is only as good as those examples, so treat a missing example as a missing test.

### 2.4 Running locally

**Decided M13: there is no `integration` feature flag.** The original plan gated the DynamoDB Local suite behind `--features integration`; it never was. Integration tests live per crate under `crates/<crate>/tests/` (using `persistence`'s `test-utils` feature and `make_test_repo()`), and plain `cargo test --workspace` runs them along with the unit and contract tests. They need a reachable Docker daemon; see [`13-dev-environments.md` §10.1](13-dev-environments.md) for the per-machine socket setup.

```bash
cd backend
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                     # unit + contract + DDB-local integration
cargo test -p domain                       # contract test only, no Docker
cargo test -p lambda-questions --test lifecycle   # one suite
```

The `testcontainers` crate starts DynamoDB Local itself, one container per test binary. CI (`backend-ci.yml`) runs the same `cargo test --workspace` on `ubuntu-latest`, whose stock Docker needs no `DOCKER_HOST` override.

---

## 3. CDK tests

`infra/tests/test_stacks.py` using `aws_cdk.assertions`. For each stack, assert:
- Expected resource counts. `ApiStack` ends up with the eight request handlers from `01-infrastructure-cdk.md` §6.2, but they arrive milestone by milestone (M4 wires two: `lambda-invites` and `lambda-groups`), so assert on each handler's `FunctionName` rather than on a total count that is wrong until M11. Note the `PreSignUp` trigger is **not** in `ApiStack` — it lives in `AuthStack` alongside the user pool it attaches to (§6.2); the tick Lambdas live in `NotificationsStack` and `lambda-image-process` in `MediaPipelineStack`.
- Cognito user pool has exactly two app clients in `dev` (`frontend` + `admin-bootstrap`) and one in `prod` (`frontend`; M13.5, D5).
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

### 4.2 API mocking for component tests (decided M9: `vi.mock`, not MSW)

The original plan was MSW handlers generated from the OpenAPI spec. M7–M9 instead mock `api/client` with `vi.mock` and seed the TanStack Query cache, with every fixture typed against `types/api.ts` so contract drift fails `tsc`. That covers what component tests need without a second mock layer beside `api/mockTransport.ts` (the stateful in-memory API used by `npm run dev`). Revisit MSW only if a test needs real `fetch`-level behavior (headers, retries on 401) — `api/client.test.ts` already covers those by stubbing `fetch` directly.

### 4.3 Playwright E2E

Run against either a mock-mode production build (no backend) or a deployed `dev` stack. There is no docker-compose or in-process server option; see the two projects below.

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

**Decided M13: two Playwright projects.**
- `mock` (default, no env needed): `playwright.config.ts` builds the app with `vite build --mode e2e` (checked-in `.env.e2e`: `VITE_USE_MOCKS=true` plus placeholder https origins) and serves it with `vite preview`. It runs the CSP checks from `04` §12.1 against a real production build, plus the golden paths the in-memory mock transport supports (it has no auth and no dev tick routes).
- `dev`: runs only when `E2E_BASE_URL` is set, against a deployed stack. It needs a real Cognito session (`E2E_USERNAME`/`E2E_PASSWORD`), so its tests skip until B5 clears.

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
- `tsc --noEmit` (`npm run typecheck`)

**Decided M13: no stylelint.** The only stylesheet is `src/styles/tailwind.css`, which holds the Tailwind directives; everything else is utility classes.

### 5.3 Python (CDK)

- `ruff check infra/`
- `mypy infra/`
- `cdk synth --context env=dev` (smoke)

---

## 6. GitHub Actions workflows

**Decided 2026-10-07; lands in M13.5.** `deploy.yml` (§6.4) replaces `frontend-deploy.yml` and `backend-deploy.yml`: one workflow deploys the backend to `dev` on every push to `main`, then — behind a manual promotion gate — deploys the backend to `prod` and publishes the frontend to `gh-pages`, both from the same commit SHA. Repo variables collapse accordingly: `BACKEND_DEPLOY_ENABLED`/`FRONTEND_DEPLOY_ENABLED` become one `DEPLOY_ENABLED` (gates the dev auto-deploy); the prod leg is gated by the GitHub Environment approval instead of a variable. `infra-ci.yml`'s `cdk diff` step still runs only when `vars.AWS_DIFF_ROLE_ARN` is set, and `e2e.yml`'s `dev` project still skips unless `vars.E2E_BASE_URL` is set.

OIDC roles (`GithubDeployRole` on `main`, `GithubDiffRole` on pull requests) live in `CiStack` (`infra/opennewsletter/ci_stack.py`). The deploy role only assumes the CDK bootstrap roles. The diff role assumes none of them: the PR job runs the branch's own code, and the bootstrap lookup role carries `ReadOnlyAccess`, which can scan the table and read private media. It gets only `DescribeStacks`/`GetTemplate` on the prod stacks and `CiStack`, and `cdk diff` runs with `--no-change-set` (decided 2026-10-11). The variable and secret list is in [`.github/workflows/README.md`](../.github/workflows/README.md).

### 6.1 `frontend-ci.yml`

Triggers: PRs touching `frontend/**` or shared types.

Steps:
1. Set up Node from `.nvmrc` (24 since 2026-10-07). Every workflow uses `node-version-file: .nvmrc`.
2. `npm ci`.
3. `npm run typecheck`, `npx eslint .`, `npx prettier --check .`.
4. `npm run test:coverage` (Vitest with `--coverage`).
5. `npm run build`.
6. Upload coverage artifact.

### 6.2 `backend-ci.yml`

Triggers: PRs touching `backend/**`.

Steps:
1. Setup Rust toolchain (pinned via `rust-toolchain.toml`).
2. Cache `~/.cargo` and `target/`.
3. `cargo fmt --check && cargo clippy ... && cargo deny check`.
4. `cargo test --workspace` — the `testcontainers` crate starts DynamoDB Local itself (§2.4); the runner just needs Docker, no service-container setup step.
5. `cargo lambda build --release --arm64` (sanity: ensures all crates compile to lambda artifacts).

### 6.3 `infra-ci.yml`

Triggers: PRs touching `infra/**`.

Steps:
1. Setup Python 3.12.
2. `pip install -r requirements.txt`.
3. `ruff && mypy && pytest infra/tests/`.
4. `cdk synth --context env=dev` (smoke).
5. `cdk diff --context env=prod` against the live stacks (read-only). Posts diff as PR comment.

### 6.4 `deploy.yml` — dev auto-deploy + gated prod promotion (decided 2026-10-07; lands in M13.5)

Triggers: push to `main`.

**Config variables (M13.5).** Unprefixed repo variables (`ROOT_DOMAIN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, …) are prod's; the dev job maps `DEV_*` variables onto the same names, so dev can never silently pick up a prod value. `ALARM_EMAIL` is shared, since `CiStack` is account-wide and both jobs deploy it. Both jobs fail fast when their app-secret ARN is empty.

**OIDC trust (M13.5).** A job that declares `environment: production` gets the OIDC subject `repo:…:environment:production`, not `ref:refs/heads/main`, so `GithubDeployRole` trusts exactly those two subjects. The environment subject carries no branch, so the `production` environment **must** restrict deployment branches to `main` (`.github/workflows/README.md`).

**Dev job** (gated by `vars.DEPLOY_ENABLED == 'true'`):
1. Set up Rust + `cargo-lambda`; build all Lambdas (`cargo lambda build --release --arm64`) and upload them as a workflow artifact — built once here, reused by the prod job below.
2. Configure AWS credentials via OIDC (`GithubDeployRole`).
3. `cdk deploy --context env=dev --all --require-approval never`.
4. (Later: run the Playwright `dev` project here.)

Dev has no hosted frontend — GitHub Pages serves `prod` only; dev is exercised via `make fe`.

**`promote-prod` job** (needs: the dev job; `environment: production`, gated by a required reviewer — see `.github/workflows/README.md` for the private-repo fallback):
1. Download the Lambda artifact the dev job built (no rebuild, so what gets promoted is exactly what ran in dev).
2. `cdk deploy --context env=prod --all --require-approval never`.
3. `npm run build` with prod `VITE_*` env from secrets.
4. Add `CNAME`, copy `dist/index.html` → `dist/404.html`.
5. Push `dist/` to `gh-pages` (`peaceiris/actions-gh-pages`).

Backend and frontend are always promoted together from one SHA, backend first — never a lone frontend publish. This makes the `coding-standards.md` §8 **expand → contract** rule load-bearing: a route or field can be removed or renamed only after the frontend release that stopped depending on it has shipped, because an installed PWA keeps running its previous frontend until its service worker updates. The SW is configured to activate new versions promptly (`skipWaiting`/`clients.claim()`, or `vite-plugin-pwa`'s `autoUpdate` — `04-frontend-architecture.md` §5.2) so a client picks up a new frontend release on its next load.

### 6.5 `e2e.yml`

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

`backend/crates/persistence/src/test_factories.rs` (gated `#[cfg(any(test, feature = "test-utils"))]`, so other crates' tests enable `test-utils`): builders for every entity, with sensible defaults. Used by the integration tests.

The frontend has no shared factory module. Vitest fixtures are written inline and typed against `types/api.ts` (§4.2).

---

## 10. Fixtures

- `lambda-image-process`'s tests build their sample images (JPEG, PNG, WebP, animated GIF, oversized, corrupt) in code rather than from checked-in files.
- Frontend sample data lives in the `api/mock*.ts` mock routes and in each test file.

---

## 11. Observability validation

**Not built.** The plan was a nightly `e2e.yml` step asserting that every Lambda logs at least one structured line per invocation (CloudWatch Insights against `dev`) and that the `OpenNewsletter/dev` EMF namespace is populated after a smoke run. For now, `docs/smoke.md` covers checking the push EMF metrics by hand.

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

**Decided 2026-10-07; lands in M13.5:** the promotion gate (§6.4) makes it practical to smoke dev first, then prod — a reviewer can smoke-test what's about to be promoted on `dev` before approving `promote-prod`, then re-run the same checklist against `prod` once it ships. `docs/smoke.md` covers both, parameterized by `ENV=dev|prod`.

After every prod deploy (and now, before approving a promotion):
1. Sign in.
2. Generate an invite (admin tab).
3. Redeem in incognito with a different account.
4. Suggest a candidate, vote on it.
5. Save a draft and publish it, in whichever cycle is `open`. Prod has no dev tick routes, and invoking cycle-tick directly only performs transitions that are already due. So in prod, commenting and reacting can only be smoke-tested on an edition that is already published.
6. Comment and react on a published edition, if one exists.
7. Send a test push to self.

A single Markdown checklist `docs/smoke.md` lives in the repo with these steps.
