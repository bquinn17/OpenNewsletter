# 13 — Dev Environments & Local Testing

This document defines how engineers work day-to-day: where code runs, where state lives, and how to reset it. The guiding principle is **high-fidelity AWS over local emulation** — instead of maintaining a parallel local stack of mocks for DynamoDB, S3, Cognito, EventBridge, and CloudFront, we commit to a real-but-disposable AWS development environment. Local execution is reserved for fast unit tests.

This complements [`11-testing-ci-cd.md`](11-testing-ci-cd.md), which covers the test pyramid and CI workflows. Where the two overlap, this document is the source of truth for the **developer experience**; `11` is the source of truth for **what gets tested**.

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
| `dev` | Shared sandbox for manual testing, exploratory work, E2E target | Long-lived; data wiped routinely | Cognito user pool with bootstrap-admin client + federated IdPs | Raw AWS endpoints |
| `ci-pr-{n}` | Per-PR ephemeral stack for E2E in CI | Spun up per PR, torn down on merge/close | Reuses `dev` Cognito user pool | Raw AWS endpoints |
| `prod` | The real thing | Permanent | Federated IdPs only | Custom domains |

`dev` is shared today because the project is solo. To split into per-developer stacks later, change the env suffix from `dev` to `dev-{username}` in `--context env=...` and the rest of CDK falls into place.

---

## 3. Stack lifetime tiers

The stacks in [`01-infrastructure-cdk.md`](01-infrastructure-cdk.md) are regrouped by *cost-of-recreation* for the `dev` env. Volatile stacks are wiped freely; persistent stacks are stood up once and left alone.

| Tier | Stacks | Why |
|---|---|---|
| **Persistent** | `AuthStack`, the S3 buckets and CloudFront distribution from `MediaStack`, `FrontendStack` (ACM certs only — no domain in dev) | Cognito federated IdP redirect URIs are registered out-of-band on Google/Apple/Facebook consoles; recreating the user pool means re-registering. CloudFront distributions take 15–30 min to delete. ACM/Route53 churn is just noise. |
| **Volatile** | `DataStack`, `ApiStack`, `NotificationsStack`, `MonitoringStack`, `lambda-image-process` (split out of MediaStack into its own construct) | Pure code/config — fast to recreate, no out-of-band registration. |

Concretely, `MediaStack` is split into:
- `MediaPersistentStack` — S3 buckets (originals + processed) + CloudFront distribution + KeyGroup
- `MediaPipelineStack` — `lambda-image-process` and its S3 event subscription

`MediaPipelineStack` is volatile; `MediaPersistentStack` is persistent.

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

## 5. CDK changes required for dev wipeability

The plan as written must be adjusted so that `cdk destroy --context env=dev` actually works on volatile stacks without manual intervention:

- **DynamoDB table** — `removal_policy=DESTROY` and `point_in_time_recovery=False` in dev (already in [`01-infrastructure-cdk.md` §3.1](01-infrastructure-cdk.md)). Confirm.
- **S3 buckets** — `auto_delete_objects=True` and `removal_policy=DESTROY` in dev only. Without this, destroy fails on non-empty buckets.
- **Secrets Manager** — `removal_policy=DESTROY` and `recovery_window=Duration.days(0)`. The default 7-day soft-delete window blocks redeploy-within-a-week with "secret already scheduled for deletion."
- **CloudWatch log groups** — `removal_policy=DESTROY` in dev. Otherwise destroy succeeds but leaves orphans, and the next deploy errors on "log group already exists."
- **Cognito** — persistent; not affected.

Each is a one-line conditional on `config.env == "dev"`.

---

## 6. Cognito strategy in dev

The federated IdP flow (Google/Apple/Facebook) works in `dev` and is exercised periodically — but not on every iteration, because OAuth bounces through three external services and pollutes the iteration loop.

For inner-loop work, sign in via the `admin-bootstrap` app client described in [`05-auth-flow.md` §9](05-auth-flow.md). It supports `ALLOW_USER_PASSWORD_AUTH`, so `aws cognito-idp admin-initiate-auth` returns a real JWT in one round-trip, no browser involved. `seed_dev_data.py` prints the credentials after seeding.

The frontend's hidden `/admin/bootstrap-login` route (only enabled when `VITE_ENV=dev`) accepts username + password and stores tokens. The resulting JWT goes through the same API Gateway authorizer as a federated user's would — so the auth path is high-fidelity, just without the OAuth dance.

Federated sign-in gets exercised:
- Manually on `dev` before merging anything that touches auth
- On every nightly E2E run (one of the ~10 golden paths in [`11-testing-ci-cd.md` §4.3](11-testing-ci-cd.md) is "sign in via Google → land on home")

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

A small post-deploy script (`scripts/write_frontend_env.py`) reads `cdk.out/dev-outputs.json` and writes `frontend/.env.dev`. CORS in [`01-infrastructure-cdk.md` §6.1](01-infrastructure-cdk.md) already allows `http://localhost:5173`.

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
	  --function-name OpenNewsletter-$(LAMBDA)-dev \
	  --zip-file fileb://target/lambda/$(LAMBDA)/bootstrap.zip \
	  --no-cli-pager
```

~5s per deploy. Use `cdk deploy` only when CDK config, IAM, or env vars change. Document this distinction in `README.md` so the slow path isn't reached for by reflex.

---

## 9. EventBridge ticks in dev

Don't wait for the 5-minute / 15-minute schedule. Two paths:

1. **Manual invoke** — `aws lambda invoke --function-name OpenNewsletter-cycle-tick-dev /tmp/out.json`
2. **Admin dev route** — `POST /admin/dev/tick/{cycle|notify}`, gated on `ENV=dev`, that calls the same handler logic synchronously and returns the result.

Path 2 is also what Playwright uses to fast-forward time in E2E tests — see [`11-testing-ci-cd.md` §4.3](11-testing-ci-cd.md). Implement once, two consumers.

For tests that need to advance wall-clock time (a cycle's deadline is in the future), the seed script accepts `--cycle-close-in 5m` to write deadlines relative to now. Combined with `/admin/dev/tick`, you can walk a cycle from `voting` to `published` in a few seconds.

---

## 10. Local testing — what runs where

The test pyramid in [`11-testing-ci-cd.md`](11-testing-ci-cd.md) stands; this section pins down *where* each layer runs.

| Test layer | Local | CI | Notes |
|---|---|---|---|
| Rust unit (`domain/`, `persistence/keys.rs`, `shared/`, pure validators) | ✅ default | ✅ | Sub-second. No IO, no Docker, no AWS. Run on every save with `cargo watch -x test`. |
| Frontend unit (Vitest + MSW) | ✅ default | ✅ | Same as planned. |
| Contract test (OpenAPI ↔ Rust struct round-trip) | ✅ default | ✅ | Pure; runs under `cargo test`. |
| CDK assertions (`pytest infra/tests/`) | ✅ default | ✅ | Pure synth, no AWS calls. |
| Rust integration (DDB-local via `testcontainers`) | ⚙️ opt-in: `cargo test --features integration` | ✅ | Needs Docker. Devs don't need to run these to land work. |
| Playwright E2E | Optional, against `dev` | ✅ nightly + on demand | Targets the deployed `dev` stack or a per-PR `ci-pr-{n}` stack. |
| Manual smoke | Required against your `dev` stack before merging risky changes | — | The seed → click-through → tick → verify loop. |

The change vs. the original plan: **integration tests are a CI responsibility, not a per-commit local responsibility.** Devs run unit tests on every save and rely on CI for integration. The DDB-local container is still used in CI by `testcontainers`, so coverage is unchanged — only the developer's local machine is freed from running it.

If a CI integration test fails and you need to debug locally, opt in with `cargo test --features integration` and the same `testcontainers` path runs on your machine.

---

## 11. Ephemeral CI stacks

Per-PR E2E gets a fresh `ci-pr-{n}` stack:

1. PR opens (or `e2e` workflow triggered manually) → GitHub Action assumes the deploy role via OIDC.
2. `cdk deploy --context env=ci-pr-{n} --all` (uses the persistent `AuthStack` from `dev` — does NOT recreate Cognito).
3. Run Playwright against the new stack.
4. On workflow completion (success or failure), `cdk destroy --context env=ci-pr-{n} --all --force`.

Two safety nets:

- **Auto-cleanup sweeper** — a scheduled Lambda (in `dev` account, runs daily) that destroys any `ci-pr-*` stack older than 24h. Catches the case where a workflow crashed before its cleanup step.
- **AWS Budgets alarm** at $25/mo, emailed to `config.alarm_email`. Catches stranded resources.

---

## 12. Cost expectations

`dev` stack idle: ~$3/mo (Secrets Manager dominates: ~$0.40/secret × 5). Active use adds $1–2 in Lambda + DynamoDB + image storage. Per-PR ephemeral stacks: ~$0.50 each, deleted within hours.

Total dev-environment overhead: well under $10/mo.

---

## 13. What this replaces in the plan set

This document replaces the implicit assumption in [`11-testing-ci-cd.md` §2.4](11-testing-ci-cd.md) that developers run DynamoDB Local for inner-loop work. That assumption is now a CI-only path.

It also adds two CDK refinements that should be incorporated into [`01-infrastructure-cdk.md`](01-infrastructure-cdk.md):
- Splitting `MediaStack` into `MediaPersistentStack` + `MediaPipelineStack`.
- Per-resource removal-policy / retention overrides in dev (§5 above).

[`12-build-order.md`](12-build-order.md) gains a Milestone 3.5 between "Infrastructure baseline" and "Auth + bootstrap":

> **Milestone 3.5 — Dev environment online (½ day)**
>
> Read: this document.
>
> Deliverables: `make deploy-dev`, `make seed`, `make redeploy-volatile`, `make reset-all` working end-to-end. `frontend/.env.dev` auto-written. Bootstrap admin can sign in via the dev-only login route and hit a protected API endpoint.
>
> Done when: `make seed && make fe`, sign in as the seeded admin, and see the seeded group's home page.

Every milestone after 3.5 is testable end-to-end against a real AWS stack from the moment its code lands.

---

## 14. Daily workflow summary

```
# First thing in the morning, or after pulling main:
make seed                     # ~10s — clean state to work against

# Code changes in a Lambda:
make deploy-lambda LAMBDA=questions   # ~5s

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
- Live tail one Lambda: `aws logs tail /aws/lambda/OpenNewsletter-questions-dev --follow --since 5m`
- Search by correlation ID across all Lambdas in CloudWatch Logs Insights:
  ```
  fields @timestamp, @log, @message
  | filter correlation_id = "01H..."
  | sort @timestamp asc
  ```
- The frontend generates `x-correlation-id` per request ([`00-overview.md` §6](00-overview.md)) and surfaces it in the dev-mode error toast — copy it from there into the Insights query.

**Image pipeline**
- An image stuck in `pending` means `lambda-image-process` either didn't fire or threw. Tail its log first: `aws logs tail /aws/lambda/OpenNewsletter-image-process-dev --follow`.
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

## 16. Frontend mocking (interim)

[`frontend/src/api/mockData.ts`](../frontend/src/api/mockData.ts) and the MSW handlers in [`frontend/src/mocks/`](../frontend/src/mocks/) are scaffolding for frontend iteration **before** Milestone 3.5 lands a working `dev` stack. They let UI work proceed without an AWS account.

Once `dev` is online:
- For Vitest component tests, MSW stays — it's how tests stub the API surface, per [`11-testing-ci-cd.md` §4.2](11-testing-ci-cd.md). This is permanent.
- For browser dev (`npm run dev`), the default mode flips to `--mode dev` (real API). The MSW dev-mode toggle and `mockData.ts` are removed once no page in the app still depends on them.

Treat any new `mockData.ts` entry added after Milestone 3.5 as a smell — the right answer at that point is a `make seed` fixture entry plus a `dev`-stack call.
