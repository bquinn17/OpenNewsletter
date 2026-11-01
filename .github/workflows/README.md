# workflows

GitHub Actions. Each file starts with a top-level comment describing its
trigger and purpose. See [`docs/development/testing-and-ci.md` §6](../../docs/development/testing-and-ci.md#6-github-actions-workflows).

Two workflows deploy (D6):

1. **`deploy.yml`, on every push to `main`.** Its `dev` job builds the Lambdas
   once and deploys them to `dev`. It then builds the dev frontend from the
   deployed stacks' outputs (`scripts/write_frontend_env.py --strict`) and
   publishes it to this repo's `gh-pages`, which GitHub Pages serves at
   `DEV_ROOT_DOMAIN`.
2. **`release.yml`, on every push to the `release` branch.** Pushing
   `release` is the manual gate: `git push origin main:release`. Its `prod`
   job checks the commit is on `main` and waits for, then requires, a
   successful dev deploy of that same SHA. It then deploys that exact Lambda
   build to `prod` and publishes the prod frontend to the `gh-pages` branch of
   a separate repo, `PROD_PAGES_REPO`, whose Pages site serves
   `FRONTEND_DOMAIN`. A repo hosts only one Pages site, and this one's is dev.

In each workflow the backend deploys before the frontend, from one commit SHA.
The tip of `release` is what's in prod; `git log release..main` is what isn't
yet.

| Workflow | Trigger | Guard |
|---|---|---|
| `frontend-ci.yml` | PR / push to `main` touching `frontend/**`, `shared/openapi.yaml` | — |
| `backend-ci.yml` | PR / push to `main` touching `backend/**`, `shared/openapi.yaml`, `rust-toolchain.toml` | — |
| `infra-ci.yml` | PR / push to `main` touching `infra/**`, `scripts/**` | diff-comment sub-job additionally needs `vars.AWS_DIFF_ROLE_ARN` set and `github.event_name == 'pull_request'` |
| `deploy.yml` (`dev` job) | push to `main` | `vars.DEPLOY_ENABLED == 'true'` |
| `release.yml` (`prod` job) | push to `release` | `vars.DEPLOY_ENABLED == 'true'`; only the owner can push `release`, and only `release` can deploy to the `production` environment (see below) |
| `e2e.yml` | nightly cron (07:00 UTC) or manual | — (dev-stack-only tests skip without `E2E_USERNAME`/`E2E_PASSWORD`) |

`deploy.yml`, `release.yml` and `infra-ci`'s diff job assume an IAM role via GitHub OIDC
(`aws-actions/configure-aws-credentials`) rather than long-lived AWS keys.
The trust policies and the roles themselves are defined in
[`../../infra/opennewsletter/ci_stack.py`](../../infra/opennewsletter/ci_stack.py)
(`CiStack`, account-wide, not suffixed by env). After deploying it, paste its
two `CfnOutput`s into the repo variables below.

The `dev` job uploads the built Lambdas as the `lambda-binaries` artifact, a
tarball so `bootstrap` keeps its executable bit. The artifact is kept 30 days.
The `prod` job downloads it from the dev run for its SHA, so it deploys exactly
that build and never rebuilds; a commit more than 30 days old can't be
released. Both jobs run `scripts/check_lambda_binaries.sh` first, because CDK
silently swaps a missing binary for a stub.

The `dev` job also caches the built binaries (`lambda-binaries-<hash>`, keyed
on `backend/**` and `rust-toolchain.toml`). A push that doesn't change those,
such as a frontend or docs change, restores them and skips the ~7-minute Rust
build; it still uploads the artifact, so it can still be released. Both deploys
run `cdk deploy --concurrency 4`, which deploys independent stacks side by side.

Each workflow has its own concurrency group, `deploy-dev` and `deploy-prod`: a
newer push queues behind a running deploy rather than cancelling it mid-update,
and a dev deploy never waits on prod. They can run side by side because they
touch disjoint stacks: dev deploys `--all`, including the account-wide
`CiStack`, and prod deploys `'*-prod'`, which leaves `CiStack` out.

## Action pinning

Every `uses:` is pinned to a full commit SHA, with its release as a trailing
comment: `actions/checkout@11d5960a… # v4.4.0`. Tags can be moved, and several of
these jobs hold AWS credentials or `contents: write`. Copy that form when you add
an action; `git ls-remote --tags https://github.com/<owner>/<repo>.git` gives the
SHA (use the `^{}` line for an annotated tag). `.github/dependabot.yml` bumps the
SHA and the comment together, in one grouped weekly PR.

## The `release` branch and the `production` environment

Three settings make pushing `release` the gate:

1. **The `production` environment admits only `release`.** Settings →
   Environments → `production` → Deployment branches → Selected branches →
   `release`, with no required reviewers. The branch rule is what matters:
   `GithubDeployRole` trusts the OIDC subject `repo:…:environment:production`,
   which carries no branch, so without the rule a workflow on any branch that
   declares `environment: production` could assume the deploy role.
2. **Ruleset `release: owner only`** (target `release`): restricts creations
   and updates, with the Repository admin role as the only bypass. On a
   personal repo only the owner holds that role, so only the owner can push
   `release`.
3. **Ruleset `release: no rewrites`** (target `release`): blocks force pushes
   and deletion, with no bypass, so `release` only moves forward. Roll back by
   reverting on `main`, letting dev deploy it, and pushing `release` again.

The `prod` job also refuses a SHA that isn't on `main` or has no successful
dev deploy, so prod only ever runs what dev ran first.

**Private-repo fallback.** On a private repo without GitHub Team or
Enterprise, environment branch rules and rulesets aren't available. In that
case, scope `GithubDeployRole`'s prod trust to `ref:refs/heads/release`
instead of the environment, and protect the branch as far as the plan
allows.

## Repo variables (Settings → Secrets and variables → Actions → Variables)

`infra/opennewsletter/config.py` reads the same variable names for every
env. **Unprefixed variables hold prod's values**, which `release.yml` and
`infra-ci`'s prod diff read. **`DEV_`-prefixed variables hold dev's values**,
and the `dev` job maps them onto the unprefixed names itself. Dev can
therefore never silently pick up a prod value. An unset variable reaches
`config.py` as an empty string and falls back to its default; for
`APP_SECRET_ARN` that's a placeholder, so both deploy jobs fail fast without it.

| Variable | Used by | Notes |
|---|---|---|
| `DEPLOY_ENABLED` | `deploy.yml`, `release.yml` | `true`: every push to `main` deploys dev (backend and frontend), and every push to `release` deploys prod. One switch for both. |
| `AWS_DEPLOY_ROLE_ARN` | `deploy.yml`, `release.yml` | `CiStack`'s `GithubDeployRoleArn` output |
| `AWS_DIFF_ROLE_ARN` | `infra-ci.yml` | `CiStack`'s `GithubDiffRoleArn` output. The diff job no-ops until it's set. |
| `FRONTEND_DOMAIN` | `release.yml` | prod's frontend domain, written to `dist/CNAME` |
| `PROD_PAGES_REPO` | `release.yml` | `owner/repo` whose GitHub Pages site serves prod; `release.yml` pushes its `gh-pages` branch. Setup: [`operator-setup.md` §10](../../docs/operations/operator-setup.md#10-prod-frontend-pages-repo). |
| `ROOT_DOMAIN`, `API_DOMAIN`, `COGNITO_DOMAIN_PREFIX` | `release.yml`, `infra-ci.yml` (diff) | prod values. `release.yml` fails fast if any is empty. |
| `HOSTED_ZONE_ID`, `HOSTED_ZONE_NAME` | same | prod values. Leave them empty while DNS stays outside Route53. |
| `APP_SECRET_ARN` | same | ARN of prod's combined Secrets Manager secret `opennewsletter/prod`. It replaces `GOOGLE_OAUTH_SECRET_ARN`, `FACEBOOK_OAUTH_SECRET_ARN` and `VAPID_SECRET_ARN` (D7), which can be deleted. See [`operator-setup.md` §7](../../docs/operations/operator-setup.md#7-combine-credentials-into-one-secrets-manager-secret-per-env). |
| `BOOTSTRAP_ADMIN_EMAIL` | same | prod only (D5): the email the prod admin invite is bound to. No stack uses it; `scripts/bootstrap_admin.py --env prod` does. It's passed through so the config matches what the operator runs locally. |
| `ALARM_EMAIL` | `deploy.yml`, `release.yml`, `infra-ci.yml` | **account-wide**, one value for both envs. It receives prod's alarm emails and the account's $5/mo budget alerts. The budget lives in the account-wide `CiStack`, which only `deploy.yml` deploys; a separate prod value would still reach it through a manual `cdk deploy --context env=prod --all`. |
| `DEV_ROOT_DOMAIN`, `DEV_API_DOMAIN`, `DEV_COGNITO_DOMAIN_PREFIX`, `DEV_HOSTED_ZONE_ID`, `DEV_HOSTED_ZONE_NAME` | `deploy.yml` (`dev`) | dev's values for the same `config.py` inputs. Mirror whatever your local `.env.local` has; leave a variable empty to use `config.py`'s dev default. |
| `DEV_APP_SECRET_ARN` | `deploy.yml` (`dev`) | ARN of `opennewsletter/dev`. Required: the `dev` job fails fast without it. |
| `E2E_BASE_URL` | `e2e.yml` | targets a real dev stack instead of the self-served mock-mode app. Empty runs mock mode only. |

`DEV_ROOT_DOMAIN` is also required: it's the dev frontend's domain, written to `dist/CNAME` and used for the dev build's sign-in redirect (`https://<domain>/auth/callback`, which dev's Cognito client allows). The dev build needs no `VITE_*` variables; it reads them from the stacks the job just deployed.

`GITHUB_REPO` (the `owner/repo` that CiStack's OIDC trust matches) is set
from `github.repository` inside the workflows, so it needs no variable.

You can also set prod's values as **`production` environment variables**
under the same names; they override the repo-level ones for
`release.yml`. `infra-ci`'s diff job runs without an environment, though,
since `production` only admits `release`. It therefore only sees repo-level
values, so keep those in sync if you go this way.

## Secrets

| Secret | Used by | Notes |
|---|---|---|
| `VITE_API_BASE_URL`, `VITE_MEDIA_BASE_URL`, `VITE_UPLOAD_ORIGINS`, `VITE_COGNITO_USER_POOL_ID`, `VITE_COGNITO_CLIENT_ID`, `VITE_COGNITO_HOSTED_DOMAIN`, `VITE_REDIRECT_URI` | `release.yml` | prod's build-time `VITE_*` env. They aren't secret in the credential sense, but they're environment-specific, so they're kept as Secrets. Store them as repo or `production` environment secrets. The two media values are prod's `MediaPersistentStack` outputs `MediaBaseUrl` (`https://opennewsletter-media-processed-prod-<account>.s3.us-east-1.amazonaws.com`) and `UploadOrigins` (the two originals buckets' origins, space-separated); the CSP must match them exactly ([`media.md` §4.4](../../docs/design/media.md#44-spa-usage)). After its `cdk deploy`, `release.yml` fails before the frontend build if any of these but `VITE_REDIRECT_URI` is empty, so a first prod run deploys the backend and then stops until the two media values are set. |
| `PROD_PAGES_DEPLOY_KEY` | `release.yml` | private half of a deploy key with write access to `PROD_PAGES_REPO`. `release.yml` checks it and `PROD_PAGES_REPO` before deploying anything. |
| `E2E_USERNAME`, `E2E_PASSWORD` | `e2e.yml` | dev-stack sign-in. The dev-stack-only tests skip when these are empty. |

`VITE_COGNITO_BOOTSTRAP_CLIENT_ID` is no longer passed to the prod build:
prod has no `admin-bootstrap` client (D5, [`infrastructure.md`](../../docs/design/infrastructure.md)
§4.3). Delete the secret if it was set. It only matters for dev builds,
which get it from `frontend/.env.dev` via `scripts/write_frontend_env.py`.

`GITHUB_TOKEN` (automatic) is what the `dev` job uses to push this repo's `gh-pages`.
