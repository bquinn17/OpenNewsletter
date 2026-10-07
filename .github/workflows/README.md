# workflows

GitHub Actions. Each file starts with a top-level comment describing its
trigger and purpose. See [`../../plans/11-testing-ci-cd.md`](../../plans/11-testing-ci-cd.md).

| Workflow | Trigger | Guard |
|---|---|---|
| `frontend-ci.yml` | PR / push to `main` touching `frontend/**`, `shared/openapi.yaml` | — |
| `frontend-deploy.yml` | push to `main` touching `frontend/**`, or manual | `vars.FRONTEND_DEPLOY_ENABLED == 'true'` |
| `backend-ci.yml` | PR / push to `main` touching `backend/**`, `shared/openapi.yaml`, `rust-toolchain.toml` | — |
| `backend-deploy.yml` | push to `main` touching `backend/**` or `infra/**`, or manual (`env` input: dev\|prod) | `vars.BACKEND_DEPLOY_ENABLED == 'true'` |
| `infra-ci.yml` | PR / push to `main` touching `infra/**` | diff-comment sub-job additionally needs `vars.AWS_DIFF_ROLE_ARN` set and `github.event_name == 'pull_request'` |
| `e2e.yml` | nightly cron (07:00 UTC) or manual | — (dev-stack-only tests skip without `E2E_USERNAME`/`E2E_PASSWORD`) |

Both deploy workflows and `infra-ci`'s diff job assume an IAM role via GitHub
OIDC (`aws-actions/configure-aws-credentials`) rather than long-lived AWS
keys. The trust policies and the roles themselves are defined in
[`../../infra/opennewsletter/ci_stack.py`](../../infra/opennewsletter/ci_stack.py)
(`CiStack`, account-wide, not suffixed by env). After deploying it, paste its
two `CfnOutput`s into the repo variables below.

## Repo variables (Settings → Secrets and variables → Actions → Variables)

| Variable | Used by | Notes |
|---|---|---|
| `FRONTEND_DEPLOY_ENABLED` | `frontend-deploy.yml` | set to `true` once `frontend-ci` is green and the dev/prod stacks exist (M14) |
| `BACKEND_DEPLOY_ENABLED` | `backend-deploy.yml` | same gating, backend side |
| `FRONTEND_DOMAIN` | `frontend-deploy.yml` | written to `dist/CNAME` |
| `AWS_DEPLOY_ROLE_ARN` | `backend-deploy.yml` | `CiStack`'s `GithubDeployRoleArn` output |
| `AWS_DIFF_ROLE_ARN` | `infra-ci.yml` | `CiStack`'s `GithubDiffRoleArn` output; diff job no-ops until set |
| `ROOT_DOMAIN`, `API_DOMAIN`, `HOSTED_ZONE_ID`, `HOSTED_ZONE_NAME`, `COGNITO_DOMAIN_PREFIX`, `GOOGLE_OAUTH_SECRET_ARN`, `FACEBOOK_OAUTH_SECRET_ARN`, `VAPID_SECRET_ARN`, `CDN_SIGNING_SECRET_ARN`, `ALARM_EMAIL` | `backend-deploy.yml` | read by `infra/opennewsletter/config.py`'s `EnvConfig`; these are resource identifiers (domains, ARNs of secrets already in Secrets Manager), not credentials, so they're Variables rather than Secrets |
| `E2E_BASE_URL` | `e2e.yml` | target a real dev stack instead of the self-served mock-mode app; empty runs mock-mode only |

## Repo secrets

| Secret | Used by | Notes |
|---|---|---|
| `VITE_API_BASE_URL`, `VITE_CDN_BASE_URL`, `VITE_COGNITO_USER_POOL_ID`, `VITE_COGNITO_CLIENT_ID`, `VITE_COGNITO_BOOTSTRAP_CLIENT_ID`, `VITE_COGNITO_HOSTED_DOMAIN`, `VITE_REDIRECT_URI` | `frontend-deploy.yml` | build-time `VITE_*` env; not secret in the credential sense, but environment-specific, so kept as Secrets rather than Variables |
| `E2E_USERNAME`, `E2E_PASSWORD` | `e2e.yml` | dev-stack sign-in; the dev-stack-only tests skip when empty |

`GITHUB_TOKEN` (automatic) is used by `frontend-deploy.yml` to push to `gh-pages`.
