# Testing & CI/CD

For anyone writing tests or opening a PR: what gets tested, how, and what runs in CI. This page is the source of truth for **what gets tested**; [`environments.md`](environments.md) is the source of truth for the **developer experience** of running those tests day to day — per-machine commands for running the suites locally are there (§10) and in `CLAUDE.md`. When this page and the actual CI workflow files disagree, [`.github/workflows/`](../../.github/workflows/) wins.

---

## 1. Test pyramid by component

- **E2E (Playwright):** a small set of golden-path flows (§4.3).
- **Integration (Rust + DynamoDB Local):** every access pattern, every transaction, the lifecycle and image-pipeline suites.
- **Unit (Rust + Vitest):** pure functions and components; the bulk of the tests.

---

## 2. Rust backend tests

### 2.1 Unit (in-crate)

No IO; run on every `cargo test`. Mandatory surfaces:
- `persistence/keys.rs`: every key builder (gold-master strings).
- Env-derived behaviour in library crates (e.g. `shared::http::error_type_uri_from`). Each Lambda's `main.rs` env reads are entry glue ([`coding-standards.md` §2.3](coding-standards.md#23-error-handling)).
- Every pure validation function: emoji predicate, image MIME check, push-endpoint host allowlist, field validators. Markdown sanitising is frontend-only (Vitest).

There is no JWT verifier to test: API Gateway's Cognito authorizer verifies tokens before a Lambda runs ([`auth.md` §6](../design/auth.md#6-jwt-verification-on-the-backend)).

### 2.2 Integration

Integration tests live per crate under `crates/<crate>/tests/`, run against DynamoDB Local via `testcontainers`, and build their repo with `persistence`'s `test-utils` feature (`make_test_repo()`). Use unique group IDs per test: the membership cache is process-wide.

- **Access patterns:** ≥1 test per pattern AP1–AP21 ([`data-model.md` §3](../design/data-model.md#3-access-patterns--index-map)).
- **Transactions:** ≥1 test per transaction ([`data-model.md` §4](../design/data-model.md#4-transactional-invariants)). The high-stakes ones (vote, publish-response, redeem-invite) also have `tokio::join!` concurrency tests.
- **Lifecycle suite:** the list in [`lifecycle.md` §11](../design/lifecycle.md#11-tests).
- **Image pipeline and deletion:** the list in [`media.md` §10](../design/media.md#10-tests), run against `lambda-image-process` and `media-delete` directly.
- **Engagement:** the list in [`engagement.md` §8](../design/engagement.md#8-tests).

### 2.3 Contract test

`backend/crates/domain/tests/openapi_contract.rs` loads `shared/openapi.yaml` and round-trips every example payload through its `domain::api` type via an explicit `ROUTE_TABLE` of `(method, path, "request"|status) → type`. A YAML example without a table row fails, and so does a row without an example. It also checks that the `GradientSlug`/`AvatarColorSlug`/`FieldErrorCode` enums match `shared::config` and `domain`. Runs under `cargo test -p domain`, with no Docker.

A new or changed route needs a YAML example per request and response body and a `ROUTE_TABLE` row in the same PR. A missing example is a missing test.

### 2.4 Running locally

There is no `integration` feature flag: plain `cargo test --workspace` runs unit, contract and DynamoDB Local tests. It needs a reachable Docker daemon ([`environments.md` §10.1](environments.md#101-docker-socket-by-platform-required-for-any-docker-backed-test) for the per-machine socket).

```bash
cd backend
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace                            # unit + contract + DynamoDB Local
cargo test -p domain                              # contract test only, no Docker
cargo test -p lambda-questions --test lifecycle   # one suite
```

`testcontainers` starts one DynamoDB Local container per test binary. `make_test_repo` waits for the server to answer, sets SDK timeouts and caps concurrent container start-ups, so default parallelism is reliable. CI runs the same command on `ubuntu-latest`, whose stock Docker needs no `DOCKER_HOST`.

---

## 3. CDK tests

`infra/tests/` uses `aws_cdk.assertions`; run with `pytest infra/tests/`. Assert:
- Each handler by `FunctionName` rather than total Lambda counts (CDK adds its own helper functions). `PreSignUp` lives in `AuthStack`, the tick Lambdas in `NotificationsStack`, `lambda-image-process` in `MediaPipelineStack`.
- Cognito has two app clients in `dev` (`frontend` + `admin-bootstrap`) and one in `prod`.
- Each Lambda role's IAM policy matches the documented least-privilege scope ([`infrastructure.md` §10](../design/infrastructure.md#10-iam-least-privilege-summary)).
- Every API route has the JWT authorizer: there are zero public routes.
- DynamoDB keys, TTL attribute and both GSIs; CloudFront KeyGroup attached; S3 Block Public Access fully on.
- Both CloudFront signing public keys are distinct RSA-2048 keys (`test_cf_signing_keys.py`).

---

## 4. Frontend tests

### 4.1 Vitest

Component and unit tests with React Testing Library, including:
- the `ResponseEditor` save loop: debounce, flush on blur/hidden, retry on failure (last-write-wins, no conflict UI; [`api.md` §7.3](../design/api.md#73-put-groupsgroupidnewsletterscycleidquestionsquestionidmy-response))
- `PollWidget` state transitions and `ReactionBar` toggles
- the markdown sanitiser (strip script, allow headings, allow `image:` tokens)
- `utils/dates.ts` against fixed clocks in several group timezones

### 4.2 API mocking for component tests

Component tests `vi.mock` `api/client` and seed the TanStack Query cache, with fixtures typed against `types/api.ts` so contract drift fails `tsc`. Not MSW: `api/mockTransport.ts` (the stateful in-memory API behind `npm run dev`) already exists, and `api/client.test.ts` covers fetch-level behaviour by stubbing `fetch`. There is no shared factory module; fixtures are inline.

### 4.3 Playwright E2E

Two projects:
- **`mock`** (default, no env needed): `vite build --mode e2e` (checked-in `.env.e2e`, mocks on) served with `vite preview`. Runs the CSP checks from [`frontend.md` §12.1](../design/frontend.md#121-content-security-policy) against a real production build, plus the golden paths the mock transport supports (it has no auth and no dev tick routes). The viewport is 700px, the only width where `BottomNav` and its labels are both in the accessibility tree.
- **`dev`**: runs only when `E2E_BASE_URL` is set, against a deployed stack, with a real Cognito user (`E2E_USERNAME`/`E2E_PASSWORD`). Its tests are scaffolded as `fixme` until a dev deploy exists (B5).

Golden paths (≤10):
1. Sign in → no groups → redeem invite → see group.
2. Suggest a candidate question → see it in the list.
3. Upvote a candidate → count increments.
4. Cycle opens (via `POST /admin/dev/tick/cycle`) → see questions.
5. Save draft → close tab → reopen → draft persists.
6. Upload an image → `ready` → renders in preview.
7. Publish answer → cycle closes → comments enabled.
8. Comment on an answer → see it on reload.
9. React with emoji → toggle off → toggle on a different emoji.
10. Upload avatar → ready → `PATCH /me` → avatar in the member list.

Time fast-forwarding uses the dev-only tick routes ([`api.md` §11a](../design/api.md#11a-dev-only-endpoints)), which return 404 outside dev.

---

## 5. Lint, format, type-check

### 5.1 Rust

`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo deny check` (licences, advisories, crates.io-only sources; each ignore in `backend/deny.toml` carries a reason).

### 5.2 Frontend

`eslint .`, `prettier --check .`, `npm run typecheck` (`tsc --noEmit` for the app and the service worker). No stylelint: the only stylesheet holds the Tailwind directives.

### 5.3 Python (CDK)

`ruff check`, `ruff format --check`, `mypy --strict`, `cdk synth` for dev and prod.

---

## 6. GitHub Actions workflows

Five workflows, each with a header comment explaining it. Every `uses:` is pinned to a full commit SHA with a `# vX.Y.Z` comment, and `.github/dependabot.yml` bumps them in one weekly PR. Node comes from `.nvmrc` everywhere. Deploy-shaped steps stay off until repo variables enable them, so `main` is green without AWS. The variable and secret list is in [`.github/workflows/README.md`](../../.github/workflows/README.md).

OIDC roles live in `CiStack` (`infra/opennewsletter/ci_stack.py`):
- **`GithubDeployRole`** trusts `ref:refs/heads/main` and `environment:production`, and may only assume the CDK bootstrap roles.
- **`GithubDiffRole`** trusts pull requests and assumes **no** bootstrap role: the PR job runs the branch's own code, and the bootstrap lookup role's `ReadOnlyAccess` could scan the table and read private media. It gets only `DescribeStacks`/`GetTemplate` on the prod stacks and `CiStack`, and `cdk diff` runs with `--no-change-set`.

### 6.1 `frontend-ci.yml`

PRs and pushes to `main` touching `frontend/**` or shared types: `npm ci`, typecheck, ESLint, Prettier, `npm run test:coverage` (artifact uploaded), `npm run build`.

### 6.2 `backend-ci.yml`

PRs and pushes touching `backend/**`: fmt, clippy, `cargo deny`, `cargo test --workspace` (the runner's Docker hosts DynamoDB Local), `cargo lambda build --release --arm64`.

### 6.3 `infra-ci.yml`

PRs and pushes touching `infra/**`: ruff, mypy, pytest, `cdk synth` dev + prod. On PRs, when `vars.AWS_DIFF_ROLE_ARN` is set, `cdk diff --context env=prod --no-change-set` is posted as a comment, using the same prod config variables as the deploy.

### 6.4 `deploy.yml` — dev auto-deploy + gated prod promotion

Push to `main`.

**Config.** Unprefixed repo variables (`ROOT_DOMAIN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, …) are prod's; the dev job maps `DEV_*` variables onto the same names, so dev can never pick up a prod value. `ALARM_EMAIL` is shared (`CiStack` is account-wide). Each job fails fast on an empty app-secret ARN.

**OIDC.** A job with `environment: production` gets the subject `repo:…:environment:production`, which carries no branch, so the `production` environment **must** restrict deployment branches to `main`.

**Dev job** (when `vars.DEPLOY_ENABLED == 'true'`):
1. Build every Lambda (`cargo lambda build --release --arm64`), check none is a stub (`scripts/check_lambda_binaries.sh`), and upload them as a tarred artifact (keeps the exec bit).
2. Assume `GithubDeployRole` via OIDC; `cdk deploy --context env=dev --all --require-approval never`.

Dev has no hosted frontend; it's exercised via `make fe`.

**`promote-prod` job** (needs the dev job; `environment: production` with a required reviewer):
1. Download the dev job's Lambda artifact, so prod runs exactly what ran in dev.
2. `cdk deploy --context env=prod --all --require-approval never`.
3. `npm run build` with the prod `VITE_*` env, add `CNAME`, copy `index.html` → `404.html` ([`frontend.md` §14.5](../design/frontend.md#145-github-pages-spa-fallback)), push `dist/` to `gh-pages`.

Backend and frontend are always promoted together from one SHA, backend first. That makes the **expand → contract** rule ([`coding-standards.md` §8](coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)) load-bearing: an installed PWA runs its previous frontend until its service worker updates, so a route or field is removed only after the frontend that stopped using it has shipped. The SW activates new versions promptly (`activatePromptly`: `skipWaiting` + `clients.claim()`, not `autoUpdate`, which reloads open pages; [`frontend.md` §5.2](../design/frontend.md#52-service-worker--caching-strategy)).

### 6.5 `e2e.yml`

Nightly at 07:00 UTC and on manual dispatch: install Chromium, `npm run e2e`, upload traces on failure. Runs the `mock` project, and also the `dev` project when `vars.E2E_BASE_URL` is set. Per-PR stacks are a future idea ([`future-ideas.md` §3](../future-ideas.md#3-ephemeral-per-pr-ci-stacks)).

---

## 7. Pre-commit hooks

Optional and not set up. A `.pre-commit-config.yaml` would run fmt, a fast clippy subset, ESLint/Prettier and ruff on staged files.

---

## 8. Coverage targets

Not enforced in CI, and only partly measured. `frontend-ci.yml` runs `npm run test:coverage` (`vitest run --coverage`) and uploads the report as the `frontend-coverage` artifact, but neither that workflow nor `vite.config.ts` sets a threshold — nothing fails the build on a drop, so the artifact only helps if someone opens it. `backend-ci.yml` has no coverage step at all (no `cargo tarpaulin`/`cargo llvm-cov`), so Rust coverage isn't measured anywhere in CI today. The table below is therefore a set of targets to aim for, not a report of current numbers:

| Component | Statement coverage target |
|---|---|
| `domain/`, `persistence/keys.rs` | ≥90% |
| Other backend crates | ≥75% |
| `frontend/src/utils/`, `state/` | ≥85% |
| Frontend components | ≥60% (E2E covers integration) |

---

## 9. Test data factories

`backend/crates/persistence/src/test_factories.rs` (gated `#[cfg(any(test, feature = "test-utils"))]`): builders for every entity with sensible defaults, plus `make_test_repo()`. The frontend has none (§4.2).

---

## 10. Fixtures

`lambda-image-process` tests build their sample images (JPEG, PNG, WebP, animated GIF, oversized, corrupt) in code. `lambda-media` loads a throwaway signing key from `tests/fixtures/test_signing_key.pem` (explicitly un-ignored in `.gitignore`). Frontend sample data lives in the `api/mock*.ts` routes and in each test file.

---

## 11. Observability validation

Not built. The idea: a nightly step asserting every Lambda logs a structured line per invocation and that the EMF namespace is populated after a smoke run. For now [`smoke-test.md`](../operations/smoke-test.md) checks the push EMF metrics by hand.

---

## 12. Schema migration handling

Adding an entity or GSI key:
1. Update [`data-model.md`](../design/data-model.md) and `keys.rs`.
2. For a new GSI, deploy CDK first (DynamoDB backfills it), then the code that uses it.

Never change an existing key shape in place: add a new attribute or key, dual-write, migrate (`scripts/migrate_<name>.py`), then remove the old one.

---

## 13. Manual smoke runbook

[`docs/operations/smoke-test.md`](../operations/smoke-test.md), parameterised by `ENV=dev|prod`. Run it on dev before approving a `promote-prod`, and on prod after it ships. Prod has no dev tick routes, so commenting and reacting can only be smoke-tested there on an edition that is already published.
