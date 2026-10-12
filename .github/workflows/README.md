# workflows

GitHub Actions. Each file starts with a top-level comment describing its
trigger and purpose. See [`../../plans/11-testing-ci-cd.md`](../../plans/11-testing-ci-cd.md) §6.

`deploy.yml` (M13.5, D6) is the only deploy workflow; it replaced
`frontend-deploy.yml` and `backend-deploy.yml`. On every push to `main`:

1. Its `dev` job builds the Lambdas once and deploys them to `dev`.
2. Its `promote-prod` job waits for manual approval. It then deploys that same
   build to `prod` and publishes the frontend to `gh-pages`. The backend
   always deploys before the frontend, and both come from one commit SHA.

| Workflow | Trigger | Guard |
|---|---|---|
| `frontend-ci.yml` | PR / push to `main` touching `frontend/**`, `shared/openapi.yaml` | — |
| `backend-ci.yml` | PR / push to `main` touching `backend/**`, `shared/openapi.yaml`, `rust-toolchain.toml` | — |
| `infra-ci.yml` | PR / push to `main` touching `infra/**`, `scripts/**` | diff-comment sub-job additionally needs `vars.AWS_DIFF_ROLE_ARN` set and `github.event_name == 'pull_request'` |
| `deploy.yml` — `dev` job | push to `main` | `vars.DEPLOY_ENABLED == 'true'` |
| `deploy.yml` — `promote-prod` job | same run, `needs: dev` (skipped when `dev` is) | GitHub Environment `production` requires owner approval (see below) |
| `e2e.yml` | nightly cron (07:00 UTC) or manual | — (dev-stack-only tests skip without `E2E_USERNAME`/`E2E_PASSWORD`) |

`deploy.yml` and `infra-ci`'s diff job assume an IAM role via GitHub OIDC
(`aws-actions/configure-aws-credentials`) rather than long-lived AWS keys.
The trust policies and the roles themselves are defined in
[`../../infra/opennewsletter/ci_stack.py`](../../infra/opennewsletter/ci_stack.py)
(`CiStack`, account-wide, not suffixed by env). After deploying it, paste its
two `CfnOutput`s into the repo variables below.

The `dev` job uploads the built Lambdas as the `lambda-binaries` artifact, a
tarball so `bootstrap` keeps its executable bit. The artifact is kept 30 days.
`promote-prod` deploys exactly that build and never rebuilds. Both jobs run
`scripts/check_lambda_binaries.sh` first, because CDK silently swaps a missing
binary for a stub. Runs share one `deploy` concurrency group: a newer push
queues behind a running deploy rather than cancelling it mid-update.

## Action pinning

Every `uses:` is pinned to a full commit SHA, with its release as a trailing
comment: `actions/checkout@11d5960a… # v4.4.0`. Tags can be moved, and several of
these jobs hold AWS credentials or `contents: write`. Copy that form when you add
an action; `git ls-remote --tags https://github.com/<owner>/<repo>.git` gives the
SHA (use the `^{}` line for an annotated tag). `.github/dependabot.yml` bumps the
SHA and the comment together, in one grouped weekly PR.

## The `production` environment

`promote-prod` runs under a GitHub **Environment** named `production`, with
the owner as a required reviewer. To set it up: Settings → Environments →
New environment `production` → Required reviewers, then **Deployment
branches → Selected branches → `main`**. The branch rule is required, not
optional: `GithubDeployRole` trusts the OIDC subject
`repo:…:environment:production`, which carries no branch, so without the
rule a workflow on any branch that declares `environment: production` could
assume the deploy role once a reviewer approves it. This is free on public
repos.

**Private-repo fallback.** On a private repo without GitHub Team or
Enterprise, environment protection rules aren't available, so there's no
reviewer gate to attach. In that case, replace the `environment:` gate with
a `workflow_dispatch` input: a manual "Promote to prod" run that takes the
commit SHA to promote. A promotion then still needs an explicit, auditable
action rather than firing automatically after the dev deploy.

## Repo variables (Settings → Secrets and variables → Actions → Variables)

`infra/opennewsletter/config.py` reads the same variable names for every
env. **Unprefixed variables hold prod's values**, which `promote-prod` and
`infra-ci`'s prod diff read. **`DEV_`-prefixed variables hold dev's values**,
and the `dev` job maps them onto the unprefixed names itself. Dev can
therefore never silently pick up a prod value. An unset variable reaches
`config.py` as an empty string and falls back to its default; for
`APP_SECRET_ARN` that's a placeholder, so both jobs fail fast without it.

| Variable | Used by | Notes |
|---|---|---|
| `DEPLOY_ENABLED` | `deploy.yml` (`dev` job; `promote-prod` follows it) | replaces `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED` (D6). Set it to `true` once CI is green and the dev stack exists. `promote-prod` needs no variable of its own: the `production` reviewer is its gate. |
| `AWS_DEPLOY_ROLE_ARN` | `deploy.yml` | `CiStack`'s `GithubDeployRoleArn` output |
| `AWS_DIFF_ROLE_ARN` | `infra-ci.yml` | `CiStack`'s `GithubDiffRoleArn` output. The diff job no-ops until it's set. |
| `FRONTEND_DOMAIN` | `deploy.yml` (`promote-prod`) | written to `dist/CNAME` |
| `ROOT_DOMAIN`, `API_DOMAIN`, `COGNITO_DOMAIN_PREFIX` | `deploy.yml` (`promote-prod`), `infra-ci.yml` (diff) | prod values. `promote-prod` fails fast if any is empty. |
| `HOSTED_ZONE_ID`, `HOSTED_ZONE_NAME` | same | prod values. Leave them empty while DNS stays outside Route53. |
| `APP_SECRET_ARN` | same | ARN of prod's combined Secrets Manager secret `opennewsletter/prod`. It replaces `GOOGLE_OAUTH_SECRET_ARN`, `FACEBOOK_OAUTH_SECRET_ARN`, `VAPID_SECRET_ARN` and `CDN_SIGNING_SECRET_ARN` (D7), which can be deleted. See `docs/milestone-1-operator-runbook.md` §7. |
| `BOOTSTRAP_ADMIN_EMAIL` | same | prod only (D5): the email the prod admin invite is bound to. No stack uses it; `scripts/bootstrap_admin.py --env prod` does. It's passed through so the config matches what the operator runs locally. |
| `ALARM_EMAIL` | both `deploy.yml` jobs, `infra-ci.yml` | **account-wide**, one value for both envs. It receives prod's alarm emails and the account's $5/mo budget alerts. The budget lives in the account-wide `CiStack`, which both jobs deploy, so a per-env value would flip its subscriber back and forth. |
| `DEV_ROOT_DOMAIN`, `DEV_API_DOMAIN`, `DEV_COGNITO_DOMAIN_PREFIX`, `DEV_HOSTED_ZONE_ID`, `DEV_HOSTED_ZONE_NAME` | `deploy.yml` (`dev`) | dev's values for the same `config.py` inputs. Mirror whatever your local `.env.local` has; leave a variable empty to use `config.py`'s dev default. |
| `DEV_APP_SECRET_ARN` | `deploy.yml` (`dev`) | ARN of `opennewsletter/dev`. Required: the `dev` job fails fast without it. |
| `E2E_BASE_URL` | `e2e.yml` | targets a real dev stack instead of the self-served mock-mode app. Empty runs mock mode only. |

`GITHUB_REPO` (the `owner/repo` that CiStack's OIDC trust matches) is set
from `github.repository` inside the workflows, so it needs no variable.

You can also set prod's values as **`production` environment variables**
under the same names; they override the repo-level ones for
`promote-prod`. `infra-ci`'s diff job runs without an environment, though.
Gating it behind the reviewer would hold up every PR. It therefore only
sees repo-level values, so keep those in sync if you go this way.

## Secrets

| Secret | Used by | Notes |
|---|---|---|
| `VITE_API_BASE_URL`, `VITE_CDN_BASE_URL`, `VITE_COGNITO_USER_POOL_ID`, `VITE_COGNITO_CLIENT_ID`, `VITE_COGNITO_HOSTED_DOMAIN`, `VITE_REDIRECT_URI` | `deploy.yml` (`promote-prod`) | prod's build-time `VITE_*` env. They aren't secret in the credential sense, but they're environment-specific, so they're kept as Secrets. Store them as repo or `production` environment secrets. |
| `E2E_USERNAME`, `E2E_PASSWORD` | `e2e.yml` | dev-stack sign-in. The dev-stack-only tests skip when these are empty. |

`VITE_COGNITO_BOOTSTRAP_CLIENT_ID` is no longer passed to the prod build:
prod has no `admin-bootstrap` client (D5, `plans/01-infrastructure-cdk.md`
§4.3). Delete the secret if it was set. It only matters for dev builds,
which get it from `frontend/.env.dev` via `scripts/write_frontend_env.py`.

`GITHUB_TOKEN` (automatic) is what `promote-prod` uses to push to `gh-pages`.
