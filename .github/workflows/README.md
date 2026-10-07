# workflows

GitHub Actions. Each file starts with a top-level comment describing its
trigger and purpose. See [`../../plans/11-testing-ci-cd.md`](../../plans/11-testing-ci-cd.md) §6.

**Decided 2026-10-07; lands in M13.5.** `deploy.yml` replaces `frontend-deploy.yml`
and `backend-deploy.yml`: a `dev` job auto-deploys the backend on every push
to `main`, and a `promote-prod` job — gated by manual approval — deploys the
backend to `prod` and then publishes the frontend to `gh-pages`, both from
that same commit SHA. Until that lands, the two workflow files below are
still what's checked in; this README already describes the M13.5 target so
it doesn't need a second rewrite when the workflow files change.

| Workflow | Trigger | Guard |
|---|---|---|
| `frontend-ci.yml` | PR / push to `main` touching `frontend/**`, `shared/openapi.yaml` | — |
| `backend-ci.yml` | PR / push to `main` touching `backend/**`, `shared/openapi.yaml`, `rust-toolchain.toml` | — |
| `infra-ci.yml` | PR / push to `main` touching `infra/**` | diff-comment sub-job additionally needs `vars.AWS_DIFF_ROLE_ARN` set and `github.event_name == 'pull_request'` |
| `deploy.yml` — `dev` job | push to `main` | `vars.DEPLOY_ENABLED == 'true'` |
| `deploy.yml` — `promote-prod` job | same run, needs `dev` job | GitHub Environment `production` requires owner approval (see below) |
| `e2e.yml` | nightly cron (07:00 UTC) or manual | — (dev-stack-only tests skip without `E2E_USERNAME`/`E2E_PASSWORD`) |

`deploy.yml` and `infra-ci`'s diff job assume an IAM role via GitHub OIDC
(`aws-actions/configure-aws-credentials`) rather than long-lived AWS keys.
The trust policies and the roles themselves are defined in
[`../../infra/opennewsletter/ci_stack.py`](../../infra/opennewsletter/ci_stack.py)
(`CiStack`, account-wide, not suffixed by env). After deploying it, paste its
two `CfnOutput`s into the repo variables below.

## The `production` environment (decided 2026-10-07; lands in M13.5, D6)

`promote-prod` runs under a GitHub **Environment** named `production` with
the owner set as a required reviewer (Settings → Environments → New
environment → Required reviewers). This is free on public repos. On a
**private** repo without GitHub Team/Enterprise (where environment
protection rules aren't available), there's no reviewer gate to attach —
use a `workflow_dispatch` input instead: a manual "Promote to prod" run
that takes the commit SHA to promote, so a promotion still requires an
explicit, auditable action rather than firing automatically after the dev
deploy.

## Repo variables (Settings → Secrets and variables → Actions → Variables)

| Variable | Used by | Notes |
|---|---|---|
| `DEPLOY_ENABLED` | `deploy.yml` (`dev` job) | replaces `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED` (decided 2026-10-07, D6); set to `true` once CI is green and the dev stack exists. The `promote-prod` job needs no variable — the `production` environment's required reviewer is the gate. |
| `FRONTEND_DOMAIN` | `deploy.yml` (`promote-prod` job) | written to `dist/CNAME` |
| `AWS_DEPLOY_ROLE_ARN` | `deploy.yml` | `CiStack`'s `GithubDeployRoleArn` output |
| `AWS_DIFF_ROLE_ARN` | `infra-ci.yml` | `CiStack`'s `GithubDiffRoleArn` output; diff job no-ops until set |
| `ROOT_DOMAIN`, `API_DOMAIN`, `HOSTED_ZONE_ID`, `HOSTED_ZONE_NAME`, `COGNITO_DOMAIN_PREFIX`, `ALARM_EMAIL`, `BOOTSTRAP_ADMIN_EMAIL` | `deploy.yml` | read by `infra/opennewsletter/config.py`'s `EnvConfig`; resource identifiers and operator contacts, not credentials, so they're Variables rather than Secrets. `BOOTSTRAP_ADMIN_EMAIL` is new (decided 2026-10-07, D5) — the email the prod admin invite is bound to. |
| `APP_SECRET_ARN` | `deploy.yml` | the one combined Secrets Manager secret per env (decided 2026-10-07, D7 — replaces `GOOGLE_OAUTH_SECRET_ARN`/`FACEBOOK_OAUTH_SECRET_ARN`/`VAPID_SECRET_ARN`/`CDN_SIGNING_SECRET_ARN`); see `docs/milestone-1-operator-runbook.md` §7 |
| `E2E_BASE_URL` | `e2e.yml` | target a real dev stack instead of the self-served mock-mode app; empty runs mock-mode only |

## Repo secrets

| Secret | Used by | Notes |
|---|---|---|
| `VITE_API_BASE_URL`, `VITE_CDN_BASE_URL`, `VITE_COGNITO_USER_POOL_ID`, `VITE_COGNITO_CLIENT_ID`, `VITE_COGNITO_BOOTSTRAP_CLIENT_ID`, `VITE_COGNITO_HOSTED_DOMAIN`, `VITE_REDIRECT_URI` | `deploy.yml` (`promote-prod` job) | build-time `VITE_*` env; not secret in the credential sense, but environment-specific, so kept as Secrets rather than Variables |
| `E2E_USERNAME`, `E2E_PASSWORD` | `e2e.yml` | dev-stack sign-in; the dev-stack-only tests skip when empty |

`GITHUB_TOKEN` (automatic) is used by `deploy.yml`'s `promote-prod` job to push to `gh-pages`.
