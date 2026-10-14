# Dev environments & local testing

For a contributor setting up a machine, running the suites locally, or resetting the shared `dev` AWS stack. It defines where code runs, where state lives, and how to reset it. The guiding principle is **high-fidelity AWS over local emulation** — instead of maintaining a parallel local stack of mocks for DynamoDB, S3, Cognito, EventBridge, and CloudFront, the project commits to a real-but-disposable AWS development environment. Local execution is reserved for fast unit tests.

This page and [`testing-and-ci.md`](testing-and-ci.md) split the same territory two ways: this one is the **developer experience** (how you work day to day, and how to stand up or reset the `dev` stack); that one is **what gets tested and how CI runs it**. Where a command or detail could live in either, it lives here and that page links to it, or vice versa — follow the links rather than expecting duplication. When this page and the infra code disagree, the code (`infra/opennewsletter/*.py`, `backend/crates/shared/src/config.rs`) wins; file an issue against this page.

---

## 0. Workstation setup

This repo is developed on two machines: a Windows box (WSL2, Ubuntu 20.04) and an Apple Silicon Mac (macOS). Toolchain provisioning, the Docker backend, and a couple of compiler quirks differ enough between them to document separately below; everything else in this document (the `make` targets, the dev-stack workflow) is identical on both once setup is done.

### 0.1 macOS (Apple Silicon)

A fresh checkout has nothing preinstalled beyond Homebrew and Apple's system `python3` (3.9.6 — too old for `infra/`; use Homebrew's 3.12 instead).

**First-time setup checklist**
1. Rust:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
   ```
   Open a new shell (or run `. "$HOME/.cargo/env"`) so `cargo` is on `PATH`; if rustup was installed with `--no-modify-path`, add that line to `~/.zshrc` yourself. `rust-toolchain.toml` then pulls the pinned Rust version + `rustfmt` + `clippy` automatically on first use.
2. `brew install colima docker nvm coreutils` — macOS has no native Docker Engine (Colima supplies the daemon) and no `timeout` (coreutils supplies `gtimeout`). Node comes from nvm, not Homebrew's `node` formula, so the version follows `.nvmrc` (Node 24). Add nvm to `~/.zshrc` as `brew info nvm` describes, then from the repo root:
   ```bash
   nvm install && nvm alias default 24
   ```
3. `colima start --cpu 4 --memory 6 --vm-type vz`. Colima points the `docker` CLI at itself automatically, but `testcontainers` (used by the Rust integration tests) needs its own env vars — add both to `~/.zshrc`:
   ```bash
   export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock
   export TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE=/var/run/docker.sock
   ```
   Colima doesn't survive a reboot on its own — run `colima start` again afterward.
4. `cargo-lambda` — only needed for deploy builds (`make build-lambdas` / `make deploy-*`), not for running tests. Homebrew refuses the tap unless it's explicitly trusted:
   ```bash
   brew tap cargo-lambda/cargo-lambda && brew trust cargo-lambda/cargo-lambda && brew install cargo-lambda
   ```
5. infra venv:
   ```bash
   cd infra && python3.12 -m venv .venv && .venv/bin/pip install -r requirements.txt && ln -sf ../../bin/cdk .venv/bin/cdk
   ```
   The `typeguard~=2.13.3` pin (chosen for older Python compat, see `requirements.txt`) works fine on 3.12. The CDK CLI itself runs via `npx aws-cdk@2`, fetched on first use; the committed wrapper `infra/bin/cdk` does exactly that, and the symlink above puts it on `PATH` as `cdk` whenever the venv is active (the `Makefile` relies on this). **Don't install it globally** (see below for why). `cdk.json` runs `python3 app.py`, so **activate the venv first** (`source infra/.venv/bin/activate`); otherwise macOS's system `python3` is used and synth fails with `No module named 'aws_cdk'`.

   **Spell it `npx aws-cdk@2`, not `npx cdk`.** With a global `cdk` installed (e.g. Homebrew's `aws-cdk` at `/opt/homebrew/bin/cdk`), `npx cdk` resolves to that binary and runs it with `/opt/homebrew/bin` prepended to `PATH`, ahead of the activated venv. Homebrew's `python3` then runs `app.py` and fails with the same `No module named 'aws_cdk'`, even though the venv is fine.
6. The Apple clang toolchain means the gcc-9/`aws-lc-sys` release-build failure that hits WSL2 (§0.2, blocker B6 in `plans/PROGRESS.md`) does not apply here — nothing to do for it.

**Verify your setup**
```bash
cargo --version
docker info | grep "Server Version"
node --version
(cd infra && .venv/bin/pytest tests -q)        # expect all passed
(cd backend && export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE=/var/run/docker.sock && gtimeout 1800 cargo test --workspace 2>&1)
```

**Fresh-clone gotcha**: `.gitignore`'s `*.pem` rule would otherwise also swallow the committed `infra/keys/cf-signing.pub.pem` placeholder, leaving a fresh clone with no signing key, so `cdk synth` and every `infra/tests/test_stacks.py` test that builds `MediaPersistentStack` (directly or via `ApiStack`) fail with `FileNotFoundError`, along with `test_cf_signing_keys.py`. A `!infra/keys/*.pub.pem` exception plus a committed placeholder key prevents this — don't remove that exception.

### 0.2 Windows / WSL2 (Ubuntu 20.04)

This is the machine the repo was originally built on. Docker runs rootless under WSL2 on a non-default socket, and the stock compiler has a known incompatibility with one dependency.

**First-time setup checklist**
1. Rust via the same rustup one-liner as macOS; `rust-toolchain.toml` pulls the pinned components.
2. Docker: rootless dockerd under WSL2, socket at `/mnt/wslg/runtime-dir/docker.sock` (not `/var/run/docker.sock`). `~/.profile` exports `DOCKER_HOST` for interactive shells, but non-login/non-interactive shells — CI steps, AI coding agents — never source it. See **"Running the Rust tests"** in `CLAUDE.md` for the authoritative command; the short version is to export it inline on every Docker-touching command:
   ```bash
   export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && timeout 1800 cargo test --workspace 2>&1
   ```
   If the daemon isn't running, start it detached (a plain backgrounded `nohup` dies when the caller's process group is reaped):
   ```bash
   setsid nohup ~/bin/dockerd-rootless.sh > /tmp/dockerd.log 2>&1 < /dev/null & disown
   ```
3. Release/Lambda builds abort inside `aws-lc-sys` under this host's gcc 9.4 (no clang installed) — see [gcc bug 95189](https://gcc.gnu.org/bugzilla/show_bug.cgi?id=95189) and blocker B6 in `plans/PROGRESS.md`. Debug builds and the default test suite are unaffected — only optimized builds hit it. Fix: `sudo apt install clang && export CC=clang`; installing `cargo-lambda` (next step) also sidesteps it, since its zig-based cross-compile brings its own clang and is the actual production build path.
4. `cargo-lambda`: `cargo install cargo-lambda`.
5. infra venv: the system `python3` (Ubuntu 20.04 ships 3.8, which is what the `typeguard~=2.13.3` pin was originally chosen for):
   ```bash
   cd infra && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt && ln -sf ../../bin/cdk .venv/bin/cdk
   ```
6. Node: install per `.nvmrc` (Node 24) via `nvm`: `nvm install && nvm alias default 24`.

**Verify your setup**
```bash
cargo --version
docker info | grep "Server Version"     # SocketNotFoundError means DOCKER_HOST is wrong or the daemon is down, not a test bug
node --version
(cd infra && .venv/bin/pytest tests -q)
(cd backend && export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && timeout 1800 cargo test --workspace 2>&1)
```

---

## 1. Goals & non-goals

**Goals**
- Inner-loop iteration in seconds, not minutes, for the common case (data reset, Lambda code change).
- Behavior in dev matches behavior in prod — same DynamoDB, same Cognito JWT shape, same S3 presigned URLs, same CloudFront signing, same EventBridge fan-out.
- Wiping and recreating a dev environment is cheap and routine.
- No second implementation of any AWS interaction to maintain.

**Non-goals**
- Running the full app offline. We do not maintain DynamoDB Local, LocalStack, MinIO, or a Cognito mock for application use. The cost of keeping those faithful to AWS exceeds the cost of a $3/mo cloud dev stack.
- Per-developer custom domains. Dev stacks use raw AWS endpoints.

---

## 2. Environments

| Env | Purpose | Lifetime | Auth | URL |
|---|---|---|---|---|
| `dev` | Shared sandbox for manual testing, exploratory work, E2E target | Long-lived; data wiped routinely | Cognito user pool with bootstrap-admin client + federated IdPs | Raw AWS endpoints for actual traffic (`ROOT_DOMAIN`/`API_DOMAIN` only register `dev.{domain}` on the shared ACM cert/CloudFront alias — see decision below) |
| `ci-pr-{n}` | *Not built* — per-PR ephemeral stack for E2E (§11) | Spun up per PR, torn down on merge/close | Reuses `dev` Cognito user pool | Raw AWS endpoints |
| `prod` | The real thing | Permanent | Federated IdPs only | Custom domains (bare apex) |

`dev` is shared today because the project is solo. To split into per-developer stacks later, change the env suffix from `dev` to `dev-{username}` in `--context env=...` and the rest of CDK falls into place.

**Decision (2026-10-13) — dev uses a subdomain, not raw endpoints, for its `ROOT_DOMAIN`/`API_DOMAIN`.** `config.py` has no local dev/prod split for these — it always reads the unprefixed `.env.local` names, regardless of `--context env=`. `MediaPersistentStack` attaches `domain_names=[f"cdn.{config.domain}"]` to its CloudFront distribution unconditionally (unlike `ApiStack`'s custom domain, which is correctly gated on `config.env != "dev"`). The first local `cdk deploy --context env=dev` had `.env.local`'s `ROOT_DOMAIN` set to the real apex (`betterloop.site`), so it created a real ACM cert and CloudFront alias for `cdn.betterloop.site`/`api.betterloop.site` on the **dev** distribution — and an alias can only be attached to one enabled distribution account-wide, so prod's later deploy of the same alias would conflict. Fix: `.env.local`'s `ROOT_DOMAIN`/`API_DOMAIN` are `dev.betterloop.site`/`api.dev.betterloop.site` (own ACM validation CNAMEs, see `plans/PROGRESS.md` B5); prod's real values live only in `PROD_ROOT_DOMAIN`/`PROD_API_DOMAIN` (local reference) and the unprefixed GitHub repo variables (CI only, never read locally). `ApiStack`'s `config.env != "dev"` gate means dev's `api.dev.betterloop.site` never actually gets a working API Gateway mapping — same as before, dev traffic still goes over the raw `execute-api.amazonaws.com`/`cloudfront.net` endpoints (`api_stack.py`'s `cdn_base_url`/`api_endpoint` branches are unchanged). The subdomain is registered on the cert/alias only so dev stops squatting on prod's name; it is not a live, routable hostname. `MediaPersistentStack`'s missing dev-gate is not itself fixed — not needed once the aliases no longer collide, but worth closing later for consistency with `ApiStack`.

---

## 3. Stack lifetime tiers

The stacks in [`infrastructure.md`](../design/infrastructure.md) are grouped here by *cost-of-recreation* for the `dev` env. Volatile stacks are wiped freely; persistent stacks are stood up once and left alone.

| Tier | Stacks | Why |
|---|---|---|
| **Persistent** | `AuthStack`, `MediaPersistentStack` (S3 originals/processed buckets + CloudFront distribution + KeyGroup), `FrontendStack` (ACM certs only — no domain in dev) | Cognito federated IdP redirect URIs are registered out-of-band on Google/Facebook consoles; recreating the user pool means re-registering. CloudFront distributions take 15–30 min to delete. ACM/Route53 churn is just noise. |
| **Volatile** | `DataStack`, `ApiStack`, `NotificationsStack`, `MonitoringStack`, `MediaPipelineStack` (`lambda-image-process` and its S3 event subscription) | Pure code/config — fast to recreate, no out-of-band registration. |

Media's resources are split across those two stacks along exactly this line: everything slow or manual to recreate (buckets, the distribution, the key group) lives in `MediaPersistentStack`; the processing Lambda and its event wiring, which are pure code/config, live in `MediaPipelineStack`.

---

## 4. Reset granularities

Three reset levels, used for different scenarios:

| Granularity | Command | Time | When |
|---|---|---|---|
| **Hot** — data only | `make seed` | ~10s | Between manual test scenarios; the daily reset button |
| **Warm** — volatile stacks | `make redeploy-volatile` | ~3–5 min | After schema, IAM, env-var, or Lambda config changes |
| **Cold** — full teardown | `make reset-all` | ~15–30 min | Rare; major architectural changes |

Optimizing for hot reset is what makes this strategy pleasant. `scripts/seed_dev_data.py` is idempotent and destructive: it scans + batch-deletes the DynamoDB table, empties S3 prefixes, and writes a small deterministic fixture (one group, the bootstrap admin as sole member, one `voting` cycle ready to accept question suggestions). It then prints sign-in credentials for the seeded admin.

The seed only bootstraps the admin. Additional users are added by exercising the real invite-and-signup flow against the `dev` stack (`POST /admin/invites` → redeem URL → federated sign-in). This keeps the fixture small and means manual testing of multi-user scenarios uses the same code path as production.

The seed deliberately does **not** touch the Cognito user pool. Federated users created during testing persist across `make seed` runs — only their DynamoDB rows and S3 objects are cleared. This avoids re-doing the OAuth dance after every reset and accepts a slow accumulation of stale Cognito accounts as a non-issue.

---

## 5. Dev wipeability: CDK settings

`cdk destroy --context env=dev` works cleanly on the volatile stacks (§3) because every resource that would otherwise block or orphan on destroy carries a one-line conditional on `config.env == "dev"`:

- **DynamoDB table** — `removal_policy=DESTROY` and `point_in_time_recovery=False` in dev ([`data-model.md`](../design/data-model.md), `infra/opennewsletter/data_stack.py`).
- **S3 buckets** — `auto_delete_objects=True` and `removal_policy=DESTROY` in dev only (`infra/opennewsletter/media_persistent_stack.py`). Without this, destroy fails on non-empty buckets.
- **CloudWatch log groups** — `removal_policy=DESTROY` in dev (`infra/opennewsletter/api_stack.py`, `notifications_stack.py`, `media_pipeline_stack.py`, `auth_stack.py`). Otherwise destroy succeeds but leaves orphans, and the next deploy errors on "log group already exists."
- **Cognito** — persistent; not affected.
- **Secrets Manager** — not a CDK resource at all. The per-env app secret (`opennewsletter/{env}`) is created and deleted by hand with the AWS CLI ([`operator-setup.md` §7](../operations/operator-setup.md#7-combine-credentials-into-one-secrets-manager-secret-per-env)); CDK only resolves its ARN at deploy time via `{{resolve:secretsmanager:...}}` ([`infrastructure.md`](../design/infrastructure.md)). Deleting and recreating a secret of the same name inside AWS's 7-day soft-delete window fails with "already scheduled for deletion" — pass `--force-delete-without-recovery` to `aws secretsmanager delete-secret` if you need to recreate one sooner.

---

## 6. Cognito strategy in dev

The federated IdP flow (Google/Facebook) works in `dev` and is exercised periodically — but not on every iteration, because OAuth bounces through external services and pollutes the iteration loop.

For inner-loop work, sign in via the `admin-bootstrap` app client described in [`auth.md` §9](../design/auth.md#9-bootstrap-admin--first-group). It supports `ALLOW_USER_PASSWORD_AUTH`, so `aws cognito-idp admin-initiate-auth` returns a real JWT in one round-trip, no browser involved. `seed_dev_data.py` prints the credentials after seeding.

The frontend's hidden `/admin/bootstrap-login` route (only enabled when `VITE_ENV=dev`) accepts username + password and stores tokens. The resulting JWT goes through the same API Gateway authorizer as a federated user's would — so the auth path is high-fidelity, just without the OAuth dance.

Federated sign-in gets exercised manually on `dev` before merging anything that touches auth. It can't be automated: Google blocks scripted sign-in, so the nightly E2E `dev` project signs in as a Cognito password user (`E2E_USERNAME`/`E2E_PASSWORD`, [`testing-and-ci.md` §4.3](testing-and-ci.md#43-playwright-e2e)).

The `admin-bootstrap` Cognito app client, and this whole password-login path, is **dev-only** ([D5](../decisions.md#d5)). In prod there is no password auth and no `admin-bootstrap` client — the JWT authorizer's audience is the frontend client only, and the prod bootstrap admin instead signs in via an email-bound invite redeemed with Google ([`runbook.md`](../operations/runbook.md), [`operator-setup.md`](../operations/operator-setup.md)).

---

## 7. Frontend pointing at dev

Vite reads a `.env.dev` file populated by `cdk deploy` outputs:

```
VITE_API_BASE_URL=https://abcd1234.execute-api.us-east-1.amazonaws.com
VITE_CDN_BASE_URL=https://d1234.cloudfront.net
VITE_COGNITO_USER_POOL_ID=us-east-1_xxxxx
VITE_COGNITO_CLIENT_ID=xxxxx
VITE_COGNITO_BOOTSTRAP_CLIENT_ID=yyyyy
VITE_ENV=dev
```

A small post-deploy script (`scripts/write_frontend_env.py`) reads `cdk.out/dev-outputs.json` and writes `frontend/.env.dev`. CORS in [`infrastructure.md` §6.1](../design/infrastructure.md#61-http-api-apigatewayv2) already allows `http://localhost:5173`.

**Images in dev**: with no shared registrable domain, CloudFront signed *cookies* can't flow — the SPA appends the equivalent signed-URL query params (from the `GET /media-cookie` JSON body) to each image URL instead. See [`media.md` §4.5](../design/media.md#45-dev-environments-signed-query-params-instead-of-cookies). Same signing key group, same tenant isolation; only the transport differs.

Daily commands:

```bash
make deploy-dev       # cdk deploy + writes frontend/.env.dev
make seed             # reset DDB + S3, recreate fixtures
make fe               # vite dev --mode dev
```

---

## 8. Fast Lambda iteration

`cdk deploy` for a Lambda code change is 60–90s. Bypass it for code-only changes:

```makefile
deploy-lambda:
	cd backend && cargo lambda build --release --arm64 --bin $(LAMBDA)
	aws lambda update-function-code \
	  --function-name OpenNewsletter-$(FUNCTION_$(LAMBDA))-dev \
	  --zip-file fileb://target/lambda/$(LAMBDA)/bootstrap.zip \
	  --no-cli-pager
```

`LAMBDA` is the **cargo binary** name, which is not the deployed function name: `lambda-invites` ships two entry points (`invites-api`, `invites-presignup`), so neither can be called `bootstrap`, and the Makefile keeps a `FUNCTION_<binary>` map to the CloudFormation names. Add a line to that map whenever a new handler lands.

~5s per deploy. Use `cdk deploy` only when CDK config, IAM, or env vars change — it's easy to reach for it by reflex and pay 60–90s for a change `make deploy-lambda` would have shipped in ~5s.

**`make deploy-dev` builds the binaries first.** `ApiStack` falls back to a shell stub for any binary missing from `backend/target/lambda/`, so a deploy that skips the build silently ships stubs that answer every request with nothing.

---

## 9. EventBridge ticks in dev

Don't wait for the 5-minute / 15-minute schedule. Two paths:

1. **Manual invoke** — `aws lambda invoke --function-name OpenNewsletter-CycleTick-dev /tmp/out.json` (names per the Makefile's `FUNCTION_<binary>` map)
2. **Admin dev route** — `POST /admin/dev/tick/{cycle|notify}`, gated on `ENV=dev`, that calls the same handler logic synchronously and returns the result.

Path 2 is also what Playwright uses to fast-forward time in E2E tests — see [`testing-and-ci.md` §4.3](testing-and-ci.md#43-playwright-e2e). The manual invoke and the admin route run the same handler logic, so there's one implementation behind both.

For tests that need to advance wall-clock time (a cycle's deadline is in the future), the seed script accepts `--cycle-close-in 5m` to write deadlines relative to now. Combined with `/admin/dev/tick`, you can walk a cycle from `voting` to `published` in a few seconds.

---

## 10. Local testing — what runs where

The test pyramid in [`testing-and-ci.md`](testing-and-ci.md) stands; this section pins down *where* each layer runs.

| Test layer | Local | CI | Notes |
|---|---|---|---|
| Rust unit (`domain/`, `persistence/keys.rs`, `shared/`, pure validators) | ✅ default | ✅ | Sub-second. No IO, no Docker, no AWS. Run on every save with `cargo watch -x test`. |
| Frontend unit (Vitest, `vi.mock` of `api/client`) | ✅ default | ✅ | Co-located `*.test.ts(x)` files; see [`testing-and-ci.md` §4.1](testing-and-ci.md#41-vitest). |
| Contract test (OpenAPI ↔ Rust struct round-trip) | ✅ default | ✅ | Pure; runs under `cargo test`. |
| CDK assertions (`pytest infra/tests/`) | ✅ default | ✅ | Pure synth, no AWS calls. |
| Rust integration (DDB-local via `testcontainers`) | ✅ part of `cargo test --workspace` (needs Docker, §10.1) | ✅ | No feature flag ([`testing-and-ci.md` §2.4](testing-and-ci.md#24-running-locally)). |
| Playwright E2E | ✅ `npm run e2e` (mock project, no backend) | ✅ nightly + on demand | `mock` project runs against a mock-mode production build; the `dev` project needs `E2E_BASE_URL` and a deployed stack ([`testing-and-ci.md` §4.3](testing-and-ci.md#43-playwright-e2e)). |
| Manual smoke | Required against your `dev` stack before merging risky changes | — | The seed → click-through → tick → verify loop. |

`cargo test --workspace` runs everything, so local runs need Docker (§10.1). Use `cargo test -p domain` or `cargo test --lib` for a Docker-free inner loop.

### 10.1 Docker socket by platform (required for any Docker-backed test)

Neither machine this repo is developed on uses the conventional `/var/run/docker.sock`, so `testcontainers` needs to be pointed at the right socket explicitly. Full first-time setup for each is in §0.

**Windows/WSL2**: Docker runs **rootless** under WSL2 and binds `unix:///mnt/wslg/runtime-dir/docker.sock`. `~/.profile` exports `DOCKER_HOST` for interactive terminals, but non-login/non-interactive shells — which is what CI steps and AI coding agents get — never source it. Export it inline on every command that touches Docker:

```bash
export DOCKER_HOST=unix:///mnt/wslg/runtime-dir/docker.sock && timeout 1800 cargo test --workspace 2>&1
```

If the daemon isn't running, start it detached (a plain backgrounded `nohup` dies when the caller's process group is reaped):

```bash
setsid nohup ~/bin/dockerd-rootless.sh > /tmp/dockerd.log 2>&1 < /dev/null & disown
```

**macOS**: Docker runs inside Colima. `docker` CLI is pointed there automatically, but `testcontainers` needs both `DOCKER_HOST` and `TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE` set (§0.1 has the one-time `~/.zshrc` addition); macOS also has no `timeout`, so use coreutils' `gtimeout`:

```bash
export DOCKER_HOST=unix://$HOME/.colima/default/docker.sock TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE=/var/run/docker.sock && gtimeout 1800 cargo test --workspace 2>&1
```

If Colima isn't running: `colima start` (it doesn't survive a reboot on its own).

Symptom of a missing/misdirected socket on either platform: `SocketNotFoundError` panics out of `backend/crates/persistence/tests/common/mod.rs`. Verify with `docker info | grep "Server Version"` before blaming the tests.

---

## 11. Ephemeral CI stacks

Not built ([D12](../decisions.md#d12)). Design moved to [`future-ideas.md` §3](../future-ideas.md#3-ephemeral-per-pr-ci-stacks). `e2e.yml` targets the long-lived `dev` stack via `E2E_BASE_URL`.

---

## 12. Cost expectations

`dev` stack idle cost is ≈ $0.40–1/mo; active use adds well under $1 in Lambda + DynamoDB + image storage. See [`infrastructure.md` §12](../design/infrastructure.md#12-cost-expectations) for the full prod+dev cost table ([D7](../decisions.md#d7)).

---

## 13. No local emulation stack

There is no DynamoDB Local, LocalStack, or Cognito mock for *application* use — see §1. DynamoDB Local is still used, but only as the Rust integration-test backend (§10).

Every change is testable end-to-end against a real AWS stack from the moment its code lands — there's no gap between "code written" and "code runnable against production-shaped infrastructure."

---

## 14. Daily workflow summary

```
# First thing in the morning, or after pulling main:
make seed                     # ~10s — clean state to work against

# Code changes in a Lambda:
make deploy-lambda LAMBDA=questions-api   # ~5s

# Code changes that touch CDK / IAM / env vars:
make deploy-dev               # ~60–90s

# Reset between scenarios:
make seed

# Schema or config drift you can't reason about:
make redeploy-volatile        # ~3–5 min, full data + infra reset

# Truly something is wrong:
make reset-all                # ~15–30 min, last resort
```

Unit tests run continuously in a sidecar terminal:

```bash
cd backend && cargo watch -x 'test --workspace'
cd frontend && npm run test -- --watch
```

That is the entire developer loop.

---

## 15. Inspection & debugging

Manual testing fails. When it does, these are the inspection ladders.

**DynamoDB state**
- One-off lookup: `aws dynamodb get-item --table-name OpenNewsletter-dev --key '{"pk":{"S":"GROUP#01H..."},"sk":{"S":"META"}}'`
- Browse a partition: `aws dynamodb query --table-name OpenNewsletter-dev --key-condition-expression "pk = :p" --expression-attribute-values '{":p":{"S":"GROUP#01H..."}}'`
- The AWS Console's "Explore items" view is fine for ad-hoc poking; prefer it over the CLI when you don't yet know what key you're looking for.

**Lambda logs**
- Live tail one Lambda: `aws logs tail /aws/lambda/OpenNewsletter-Questions-dev --follow --since 5m`
- Search by correlation ID across all Lambdas in CloudWatch Logs Insights:
  ```
  fields @timestamp, @log, @message
  | filter correlation_id = "01H..."
  | sort @timestamp asc
  ```
- The frontend generates `x-correlation-id` per request ([`overview.md` §6](../overview.md#6-cross-cutting-conventions)) and surfaces it in the dev-mode error toast — copy it from there into the Insights query.

**Image pipeline**
- An image stuck in `pending` means `lambda-image-process` either didn't fire or threw. Tail its log first: `aws logs tail /aws/lambda/OpenNewsletter-ImageProcess-dev --follow`.
- Confirm the S3 object exists: `aws s3 ls s3://opennewsletter-media-originals-dev-{account}/uploads/{groupId}/{cycleId}/{questionId}/{userId}/`.
- Confirm the processed variants were written: `aws s3 ls s3://opennewsletter-media-processed-dev-{account}/img/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}/`.
- Check the ImageMedia DDB row's `status` and `errorMessage` fields.

**Stuck CloudFormation stack**
- `cdk deploy` failures sometimes leave a stack in `UPDATE_ROLLBACK_FAILED` or `ROLLBACK_IN_PROGRESS`. Recovery, in order of escalation:
  1. `aws cloudformation continue-update-rollback --stack-name OpenNewsletter-Api-dev` (often resolves transient resource-conflict failures).
  2. `cdk destroy --context env=dev --force` against the affected volatile stack, then `cdk deploy` again.
  3. As a last resort: delete the stack from the CloudFormation console with "retain failed resources," manually clean those resources, redeploy.
- Persistent stacks (`AuthStack`, `MediaPersistentStack`) should never be destroyed casually — see §3.

---

## 16. Frontend mocking

One mock layer exists, for UI iteration without an AWS account. There is no MSW setup and no separate `mockData.ts` fixture module:

- **`frontend/src/api/mockTransport.ts`** is permanent. With `VITE_USE_MOCKS=true` (the checked-in `.env.development`, so plain `npm run dev`), `apiFetch` is served from in-memory, OpenAPI-typed fixtures and `AuthProvider` reports a fake signed-in user. Each new client endpoint needs matching mock routes added here. `make fe` (`--mode dev`) talks to the real dev stack instead.

Vitest component tests mock `api/client` with `vi.mock` and seed the TanStack Query cache directly (see `pages/JoinPage.test.tsx`); this is the standard ([`coding-standards.md` §3.9](coding-standards.md#39-tests), [`testing-and-ci.md` §4.2](testing-and-ci.md#42-api-mocking-for-component-tests)). MSW is not used.
