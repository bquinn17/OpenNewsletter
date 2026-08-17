# Operator runbook — all milestones

Steps a human must take that code cannot automate. Ordered by milestone.
Reference plan: [`../plans/12-build-order.md`](../plans/12-build-order.md).

---

## Environment prerequisites (one-time, before M3 deploy)

### AWS account and credentials

1. Create (or designate) a personal AWS account. Both `dev` and `prod` stacks live in the same account, distinguished by the `-dev` / `-prod` suffix on stack names.
2. Create an IAM user or SSO role with `AdministratorAccess` (scoped-down least-privilege is a M13 hardening task).
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

- [ ] Decide on a domain; create Route53 hosted zone; record `HOSTED_ZONE_ID`.
- [ ] Pick a Cognito hosted-UI domain prefix (e.g. `opennewsletter-dev`).
- [ ] Register Google OAuth 2.0 client; save credentials to Secrets Manager `opennewsletter/oauth/google/dev`.
- [ ] Register Apple Services ID + Sign In With Apple; save to `opennewsletter/oauth/apple/dev`.
- [ ] Register Facebook Login app; save to `opennewsletter/oauth/facebook/dev`.
- [ ] Generate VAPID keys (`python scripts/generate_vapid_keys.py`); store in `opennewsletter/vapid/dev`.
- [ ] Generate CloudFront signing keypair; commit `infra/keys/cf-signing.pub.pem`; store private key in `opennewsletter/cdn-signing/dev`.
- [ ] Set `ALARM_EMAIL` in `.env.local` (receives AWS Budgets + CloudWatch alerts).

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

On Ubuntu 20.04 this build fails inside `aws-lc-sys` under gcc 9
(see blocker B6 in `plans/PROGRESS.md`). `sudo apt install clang` clears it.

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

Follow [`smoke.md`](smoke.md) (created in M13). All steps must pass clean.

### Confirm monitoring alarms

1. Go to AWS CloudWatch → Dashboards → `OpenNewsletter-dev`. Confirm metric widgets are populated.
2. Go to AWS Budgets. Confirm the `OpenNewsletter-dev-monthly` budget exists at $10/mo with two alert thresholds.
3. Manually trigger a CloudWatch alarm (or wait for a test condition) to confirm the SNS email fires to `ALARM_EMAIL`.

---

## M14 — Production deploy

### Repeat M1 for `prod`

All the Secrets Manager secrets created in M1 must be duplicated for production:

- `opennewsletter/oauth/google/prod`
- `opennewsletter/oauth/apple/prod`
- `opennewsletter/oauth/facebook/prod`
- `opennewsletter/vapid/prod`
- `opennewsletter/cdn-signing/prod`

Update the Cognito redirect URIs on Google/Apple/Facebook to include the prod Cognito domain.

Generate a **fresh** CloudFront signing keypair for prod (never share the dev private key):

```sh
openssl genrsa -out cf-signing-prod.key 2048
openssl rsa -pubout -in cf-signing-prod.key -out infra/keys/cf-signing-prod.pub.pem
aws secretsmanager create-secret \
  --name opennewsletter/cdn-signing/prod \
  --secret-string file://cf-signing-prod.key
rm cf-signing-prod.key
```

### Deploy prod stacks

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

```sh
python scripts/bootstrap_admin.py \
  --env prod \
  --admin-email YOUR_EMAIL \
  --admin-display-name "Your Name" \
  --group-name "OpenNewsletter"
```

### Smoke test production

1. Navigate to `https://opennewsletter.example.com`.
2. Sign in via Google.
3. Generate an invite, share the URL, redeem in a second account.
4. Enable push notifications and verify a test notification arrives.

**M14 is verified when**: a real user can sign in and use the app at the production domain.
