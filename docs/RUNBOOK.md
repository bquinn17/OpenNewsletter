# Operator runbook — all milestones

Steps a human must take that code cannot automate. Ordered by milestone.
Reference plan: [`../plans/12-build-order.md`](../plans/12-build-order.md).

---

## Environment prerequisites (one-time, before M3 deploy)

### AWS account and credentials

1. Create (or designate) a personal AWS account. Both `dev` and `prod` stacks live in the same account, distinguished by the `-dev` / `-prod` suffix on stack names.
2. Create an IAM user or SSO role with `AdministratorAccess` for your workstation. Sign in to the console as that user, **not root**, before running `aws login`; `aws login` issues keys for whichever identity the browser is signed in as. CI doesn't use these credentials: it assumes the OIDC roles in `CiStack` (see M13 below).
3. Configure credentials locally:
   ```sh
   aws configure          # or export AWS_PROFILE=opennewsletter
   aws sts get-caller-identity   # must succeed before any cdk command
   ```

### CDK bootstrap (once per account/region)

```sh
cd infra
source .venv/bin/activate
cdk bootstrap aws://ACCOUNT_ID/us-east-1
```

`cdk bootstrap` is idempotent — safe to re-run. Required before first `cdk deploy`.

### infra Python venv

The venv at `infra/.venv` is ready. Activate it before running any `cdk` or Python infra script:

```sh
source infra/.venv/bin/activate
```

All `make` targets that call CDK already activate it automatically.

---

## M1 — Operator prerequisites

See [`milestone-1-operator-runbook.md`](milestone-1-operator-runbook.md) for the detailed walkthrough.

Summary of steps (can run in parallel with M2/M3 code work):

- [ ] Decide on a domain. A Route53 hosted zone is optional: without one (the current setup, `betterloop.site` on Namecheap DNS), add the ACM validation CNAMEs at your registrar when `cdk deploy` prints them.
- [ ] Pick a Cognito hosted-UI domain prefix (e.g. `opennewsletter-dev`).
- [ ] **(decided 2026-10-07; lands in M13.5, D7)** Register the Google OAuth 2.0 client and the Facebook Login app, generate VAPID keys, and generate the CloudFront signing keypair as before — but save them all as keys inside **one** combined Secrets Manager secret, `opennewsletter/dev`, instead of four separate secrets. See [`milestone-1-operator-runbook.md`](milestone-1-operator-runbook.md) for the exact `aws secretsmanager create-secret` shape and how to migrate the four existing dev secrets. (No Apple IdP — dropped, see `plans/PROGRESS.md` decisions; Apple Developer membership is $99/yr.)
- [ ] Set `ALARM_EMAIL` in `.env.local` (receives the account-wide AWS Budgets alert; CloudWatch alarms are prod-only as of M13.5, D7).
- [ ] Set `BOOTSTRAP_ADMIN_EMAIL` in `.env.local` for prod (decided 2026-10-07, D5) — the email the prod bootstrap admin invite is bound to; see M14 below.

**`.env.local` must be populated with all ARNs before `make deploy-dev`.**

---

## M3 / M3.5 — First deploy

### Verify CDK synth

```sh
source infra/.venv/bin/activate
cd infra
cdk synth --context env=dev
pytest tests/
```

Both must be clean. If `cdk synth` fails, fix before deploying.

### Deploy dev stacks

```sh
make deploy-dev
```

This runs `cdk deploy --all --context env=dev` and writes `frontend/.env.dev`.

First deploy typically takes 10–20 minutes (CloudFront distribution creation dominates).

### Seed fixture data

```sh
make seed
```

Prints sign-in credentials for the bootstrap admin. Note the email and password.

### Verify the frontend

```sh
make fe
```

Open the printed `localhost:5173` URL. Navigate to `/admin/bootstrap-login` and sign in with the seeded admin credentials. You should see the seeded group's home page.

**M3.5 is verified when**: sign-in succeeds and the seeded group is visible.

---

## M4 — Auth + bootstrap

After M4 code lands:

### Prerequisite: real Lambda binaries

`ApiStack` substitutes a shell stub for any binary missing from
`backend/target/lambda/`, so confirm the build works before deploying — otherwise
every route returns nothing and the cause is invisible in CloudWatch:

```sh
cargo lambda --version || cargo install cargo-lambda
make build-lambdas
ls backend/target/lambda/groups-api/bootstrap   # must exist
```

On macOS, install `cargo-lambda` via `brew tap cargo-lambda/cargo-lambda && brew trust cargo-lambda/cargo-lambda && brew install cargo-lambda` instead (Homebrew refuses the tap otherwise).

On Windows/WSL2 (Ubuntu 20.04) this build fails inside `aws-lc-sys` under gcc 9
(see blocker B6 in `plans/PROGRESS.md`). `sudo apt install clang` clears it; this
does not affect macOS, which builds with Apple's clang toolchain. See
[`plans/13-dev-environments.md` §0](../plans/13-dev-environments.md#0-workstation-setup)
for full per-OS setup.

### Bootstrap the real admin

```sh
python scripts/bootstrap_admin.py \
  --env dev \
  --admin-email YOUR_EMAIL \
  --admin-display-name "Your Name" \
  --group-name "OpenNewsletter Dev" \
  --timezone America/New_York
```

The script will prompt for a password interactively.

### Verify with curl

```sh
# 1. Authenticate (USER_PASSWORD_AUTH via admin-bootstrap client)
aws cognito-idp admin-initiate-auth \
  --user-pool-id $(aws cloudformation describe-stacks \
      --stack-name AuthStack-dev \
      --query 'Stacks[0].Outputs[?OutputKey==`UserPoolId`].OutputValue' \
      --output text) \
  --client-id BOOTSTRAP_CLIENT_ID \
  --auth-flow USER_PASSWORD_AUTH \
  --auth-parameters "USERNAME=YOUR_EMAIL,PASSWORD=YOUR_PASSWORD"

# 2. Extract IdToken from the response, then:
curl -H "Authorization: Bearer ID_TOKEN" \
  https://API_ENDPOINT/me
```

**M4 is verified when**: `GET /me` returns the admin's profile.

---

## M5 — Lifecycle engine

After M5 code lands:

### End-to-end lifecycle flow

```sh
# Use the API_ENDPOINT from infra/cdk.out/dev-outputs.json

# 1. Create a group (or use the seeded one)
# 2. Suggest a candidate question
curl -X POST -H "Authorization: Bearer TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"text": "What was your highlight this month?"}' \
  https://API_ENDPOINT/groups/GROUP_ID/candidate-questions

# 3. Fast-forward cycle to open state
curl -X POST -H "Authorization: Bearer TOKEN" \
  https://API_ENDPOINT/admin/dev/tick/cycle

# 4. Verify cycle transitioned to `open`
curl -H "Authorization: Bearer TOKEN" \
  https://API_ENDPOINT/groups/GROUP_ID/newsletters/CURRENT

# 5. Fast-forward to published
curl -X POST -H "Authorization: Bearer TOKEN" \
  https://API_ENDPOINT/admin/dev/tick/cycle

# 6. Verify `published` state and that the next `voting` cycle was auto-created
```

**M5 is verified when**: the cycle transitions open → published and a new voting cycle exists.

---

## M7 — Frontend skeleton + auth

After M7 code lands:

### Sign in via Google

1. `make fe`
2. Navigate to `localhost:5173`.
3. Click "Sign in with Google" — completes the full OIDC round-trip.
4. Accept an invite URL as a second incognito browser to verify the invite-redemption flow.
5. Verify logout redirects back to the login page with no stale state.

**M7 is verified when**: Google sign-in works and an invited user can see their group.

---

## M11 — Notifications

After M11 code lands:

### Enable and test push

1. Open Settings → Notifications → enable for the test group.
2. Fast-forward a notify tick:
   ```sh
   curl -X POST -H "Authorization: Bearer TOKEN" \
     https://API_ENDPOINT/admin/dev/tick/notify
   ```
3. Verify a push notification arrives in the browser/OS notification centre.

**M11 is verified when**: a real OS notification arrives following a manual tick.

---

## M13 — Hardening

After M13 code lands:

### Execute the smoke runbook

Follow [`smoke.md`](smoke.md). All steps must pass clean.

### Wire up GitHub Actions

1. `CiStack` deploys with `--all`. Copy its outputs `GithubDeployRoleArn` and `GithubDiffRoleArn` into the repo variables `AWS_DEPLOY_ROLE_ARN` and `AWS_DIFF_ROLE_ARN`.
2. Set the remaining repo variables and secrets listed in [`.github/workflows/README.md`](../.github/workflows/README.md). Leave `DEPLOY_ENABLED` unset until M14, and create the `production` environment with yourself as required reviewer. (Until M13.5 lands `deploy.yml`, the old workflows read `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED`.)
3. Push a branch and confirm `frontend-ci`, `backend-ci` and `infra-ci` go green.

### Confirm monitoring alarms

**Decided 2026-10-07; lands in M13.5 (D7) — supersedes the counts below:** alarms move to **prod only** (≤10 alarm metrics total: Lambda error rate, API 5xx rate, DynamoDB throttles, push failure rate, `lambda-cycle-tick` errors). `dev` keeps its dashboard but has no alarms. The budget becomes **one account-wide $5/mo AWS Budget**, defined once in `CiStack`, not per-env in `MonitoringStack`.

1. Go to AWS CloudWatch → Dashboards → `OpenNewsletter-dev`. Confirm metric widgets are populated. (No alarms to confirm here post-M13.5 — dev has none.)
2. On `prod`, go to CloudWatch → Alarms. The ≤10 `OpenNewsletter-*-prod` alarm metrics should exist and sit in `OK` or `INSUFFICIENT_DATA`, not `ALARM`.
3. Go to AWS Budgets. Confirm the single account-wide budget exists at $5/mo with alerts at 80% actual and 100% forecasted.
4. Manually trigger a CloudWatch alarm (or wait for a test condition) to confirm the SNS email fires to `ALARM_EMAIL`.

---

## M14 — Production deploy

### Repeat M1 for `prod`

**Decided 2026-10-07; lands in M13.5 (D7):** secrets are no longer duplicated as four separate secrets. Create **one** combined secret, `opennewsletter/prod`, with the same keys as `opennewsletter/dev` (`google_client_id`, `google_client_secret`, `facebook_app_id`, `facebook_app_secret`, `vapid_public_key`, `vapid_private_key`, `vapid_subject`, `cdn_signing_private_key`) — see [`milestone-1-operator-runbook.md`](milestone-1-operator-runbook.md) for the exact shape.

Update the Cognito redirect URIs on Google/Facebook to include the prod Cognito domain.

Generate a **fresh** CloudFront signing keypair for prod (never share the dev private key), and put it in the combined secret under `cdn_signing_private_key`:

```sh
openssl genrsa -out cf-signing-prod.key 2048
openssl rsa -pubout -in cf-signing-prod.key -out infra/keys/cf-signing-prod.pub.pem
# merge into the existing opennewsletter/prod secret rather than creating a new one —
# see milestone-1-operator-runbook.md for the jq recipe
rm cf-signing-prod.key
```

Set `BOOTSTRAP_ADMIN_EMAIL` in the prod `.env.local`/config to the email the prod admin will sign in with (decided 2026-10-07, D5) — see "Bootstrap prod admin" below.

### Deploy prod stacks

**Decided 2026-10-07; lands in M13.5 (D6):** prod deploys go through the `promote-prod` GitHub Actions job (gated by the `production` environment's required reviewer), not a manual `cdk deploy` from an operator's machine — see [`.github/workflows/README.md`](../.github/workflows/README.md) and `plans/11-testing-ci-cd.md` §6. The command below is for first bring-up before that workflow is wired, or for break-glass recovery:

```sh
cd infra
source .venv/bin/activate
cdk deploy --context env=prod --all --require-approval broadening
```

First prod deploy will take 20–30 minutes (CloudFront + ACM cert validation).

### Swing DNS

After deploy, CDK outputs `FrontendStack-prod.AcmCertValidationCname` (if Route53 is not enabled) and `FrontendStack-prod.CloudFrontDomain`. Point your DNS:

```
opennewsletter.example.com   CNAME   <cloudfront-id>.cloudfront.net
cdn.opennewsletter.example.com  CNAME  <cloudfront-id>.cloudfront.net
api.opennewsletter.example.com  CNAME  <apigw-id>.execute-api.us-east-1.amazonaws.com
```

### Bootstrap prod admin

**Decided 2026-10-07; lands in M13.5 (D5) — replaces the password-bootstrap flow used in dev.** In prod, `scripts/bootstrap_admin.py` creates no Cognito user: it writes the Group, the first `voting` Newsletter, and an admin invite bound to `BOOTSTRAP_ADMIN_EMAIL` (`Invite.bound_email`, lowercased; `role_on_redeem=admin`; 30-day TTL), then prints the `/join?code=…` URL.

```sh
python scripts/bootstrap_admin.py \
  --env prod \
  --admin-email "$BOOTSTRAP_ADMIN_EMAIL" \
  --admin-display-name "Your Name" \
  --group-name "OpenNewsletter"
```

Open the printed URL and sign in with **Google**, using that exact email — the invite's `bound_email` check requires the ID token's `email` (lowercased) to match and `email_verified` to be true, so Facebook (which doesn't supply `email_verified`) won't work for this redemption. The normal redeem path then creates the User + membership, same as any other invite. There is no password auth in prod and no `admin-bootstrap` Cognito client there (`plans/13-dev-environments.md` §6) — this invite-based flow is the only way to bootstrap the first prod admin.

### Smoke test production

1. Navigate to `https://opennewsletter.example.com`.
2. Sign in via Google.
3. Generate an invite, share the URL, redeem in a second account.
4. Enable push notifications and verify a test notification arrives.

**M14 is verified when**: a real user can sign in and use the app at the production domain.
