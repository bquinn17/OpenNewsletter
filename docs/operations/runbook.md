# Operator runbook

Steps an operator runs by hand: deploying dev and prod, bootstrapping the first admin in each, verifying a deploy, wiring up CI/CD, and the day-to-day procedures that follow. [`build-order.md`](../../plans/build-order.md) lists the done-when gate each tagged heading below satisfies (e.g. "M5 gate"); [`operator-setup.md`](operator-setup.md) covers the one-time out-of-band setup (domain, OAuth apps, keys, the combined secret) this page assumes is already done. Treat the `Makefile` and `scripts/` themselves as the source of truth for exact flags — this page explains what each step does and why.

---

## One-time prerequisites

### AWS account and credentials

1. Create (or designate) a personal AWS account. Both `dev` and `prod` stacks live in the same account, distinguished by the `-dev` / `-prod` suffix on stack names.
2. Create an IAM user or SSO role with `AdministratorAccess` for your workstation. Sign in to the console as that user, **not root**, before running `aws login`; `aws login` issues keys for whichever identity the browser is signed in as. CI doesn't use these credentials: it assumes the OIDC roles in `CiStack` (see "Wiring up GitHub Actions" below).
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

### Infra Python venv

The venv at `infra/.venv` is ready. Activate it before running any `cdk` or Python infra script:

```sh
source infra/.venv/bin/activate
```

All `make` targets that call CDK already activate it automatically.

### Operator setup per environment

Before any deploy, [`operator-setup.md`](operator-setup.md) walks through the domain, the Cognito hosted-UI prefix, the Google/Facebook OAuth apps, VAPID keys, the CloudFront signing keypair, and the combined `opennewsletter/{env}` Secrets Manager secret they all land in — plus the alarm contact email and, for prod, the bootstrap admin's email. **`.env.local` must hold every ARN/ID it produces before running `make deploy-dev`.**

---

## Deploying dev

### Verify CDK synth and infra tests (M3 gate)

```sh
source infra/.venv/bin/activate
cd infra
cdk synth --context env=dev
pytest tests/
```

Both must be clean. If `cdk synth` fails, fix before deploying.

### Lambda binaries must be real before deploying (M4 gate)

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
(see blocker B6 in [`PROGRESS.md`](../../plans/PROGRESS.md)). `sudo apt install clang` clears it; this
does not affect macOS, which builds with Apple's clang toolchain. See
[`environments.md` §0](../development/environments.md#0-workstation-setup)
for full per-OS setup.

`make deploy-dev` (below) runs this build and `scripts/check_lambda_binaries.sh` itself, so a stub can't reach `dev` silently; this section is for a manual `cdk deploy`.

### Deploy dev stacks (M3 gate, M3.5 gate)

```sh
make deploy-dev
```

This builds every Lambda, runs `scripts/check_lambda_binaries.sh`, deploys all stacks (`cdk deploy --context env=dev --all`), and writes `frontend/.env.dev` from the stack outputs via `scripts/write_frontend_env.py`.

First deploy typically takes 10–20 minutes (CloudFront distribution creation dominates).

### Seed fixture data (M3.5 gate)

```sh
make seed
```

Prints sign-in credentials for the bootstrap admin. Note the email and password.

### Verify the frontend (M3.5 gate)

```sh
make fe
```

Open the printed `localhost:5173` URL. Navigate to `/admin/bootstrap-login` and sign in with the seeded admin credentials. You should see the seeded group's home page.

### Redeploy the volatile stacks (M3.5 gate)

```sh
make redeploy-volatile
```

Destroys and redeploys the dev volatile stacks (`DataStack`, `MediaPipelineStack`, `MonitoringStack`), then rewrites `infra/cdk.out/dev-outputs.json`. This is the "warm" reset from [`environments.md` §4](../development/environments.md#4-reset-granularities): it wipes all dev data but keeps Cognito users. Run `make seed` afterwards to get the fixture back.

**M3.5 is verified when**: sign-in succeeds, the seeded group is visible, and `make redeploy-volatile` completes.

---

## Bootstrapping an admin

### Bootstrap the dev admin (M4 gate)

```sh
python scripts/bootstrap_admin.py \
  --env dev \
  --admin-email YOUR_EMAIL \
  --admin-display-name "Your Name" \
  --group-name "OpenNewsletter Dev" \
  --timezone America/New_York
```

The script will prompt for a password interactively.

### Verify with curl (M4 gate)

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

## Verifying a deploy

### Verify the lifecycle flow (M5 gate)

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

### Sign in via Google (M7 gate)

1. `make fe`
2. Navigate to `localhost:5173`.
3. Click "Sign in with Google" — completes the full OIDC round-trip.
4. Accept an invite URL as a second incognito browser to verify the invite-redemption flow.
5. Verify logout redirects back to the login page with no stale state.

**M7 is verified when**: Google sign-in works and an invited user can see their group.

### Enable and test push (M11 gate)

1. Open Settings → Notifications → enable for the test group.
2. Fast-forward a notify tick:
   ```sh
   curl -X POST -H "Authorization: Bearer TOKEN" \
     https://API_ENDPOINT/admin/dev/tick/notify
   ```
3. Verify a push notification arrives in the browser/OS notification centre.

**M11 is verified when**: a real OS notification arrives following a manual tick.

### Confirm monitoring alarms (M13 gate)

Alarms live in `prod` only (≤10 alarm metrics total: Lambda error rate, API 5xx rate, DynamoDB throttles, push failure rate, `lambda-cycle-tick` errors) — `dev` keeps its dashboard but has no alarms. The budget is one account-wide $5/mo AWS Budget, defined once in `CiStack`, not per-env ([D7](../decisions.md#d7)).

1. Go to AWS CloudWatch → Dashboards → `OpenNewsletter-dev`. Confirm metric widgets are populated. (No alarms to confirm here — dev has none.)
2. On `prod`, go to CloudWatch → Alarms. The ≤10 `OpenNewsletter-*-prod` alarm metrics should exist and sit in `OK` or `INSUFFICIENT_DATA`, not `ALARM`.
3. Go to AWS Budgets. Confirm the single account-wide budget exists at $5/mo with alerts at 80% actual and 100% forecasted.
4. Manually trigger a CloudWatch alarm (or wait for a test condition) to confirm the SNS email fires to `ALARM_EMAIL`.
5. On `prod`, open `OpenNewsletter-CycleTick-Errors-prod`. Its math expression should read both the CycleTick Lambda's `Errors` and `OpenNewsletter/prod` `AutoPublishAbandoned` ([`infrastructure.md` §9.2](../design/infrastructure.md#92-alarms)). The `AutoPublishAbandoned` metric only appears in CloudWatch once a tick has emitted it, so an empty metric list here is expected on a healthy stack.

### Run the smoke test (M13 gate)

Follow [`smoke-test.md`](smoke-test.md). All steps must pass clean.

---

## Wiring up GitHub Actions (M13 gate)

1. `CiStack` deploys with `--all`. Copy its outputs `GithubDeployRoleArn` and `GithubDiffRoleArn` into the repo variables `AWS_DEPLOY_ROLE_ARN` and `AWS_DIFF_ROLE_ARN`.
2. Set the remaining repo variables and secrets listed in [`.github/workflows/README.md`](../../.github/workflows/README.md). Leave `DEPLOY_ENABLED` unset until the M14 production deploy ([`build-order.md`](../../plans/build-order.md)), and create the `production` environment with yourself as required reviewer.
3. Push a branch and confirm `frontend-ci`, `backend-ci` and `infra-ci` go green.

---

## Production deploy (M14 gate)

### Repeat the one-time setup for prod

Create **one** combined secret, `opennewsletter/prod`, with the same keys as `opennewsletter/dev` (`google_client_id`, `google_client_secret`, `facebook_app_id`, `facebook_app_secret`, `vapid_public_key`, `vapid_private_key`, `vapid_subject`, `cdn_signing_private_key` — no Apple keys; Google and Facebook only) — see [`operator-setup.md`](operator-setup.md) for the exact shape and the `jq` recipe.

Update the Cognito redirect URIs on Google/Facebook to include the prod Cognito domain.

Generate a **fresh** RSA-2048 CloudFront signing keypair for prod (never share the dev private key), and put it in the combined secret under `cdn_signing_private_key`. `MediaPersistentStack` reads `infra/keys/cf-signing-prod.pub.pem` for prod, so prod synth fails until it exists:

```sh
openssl genrsa -out cf-signing-prod.key 2048
openssl rsa -pubout -in cf-signing-prod.key -out infra/keys/cf-signing-prod.pub.pem
# merge into the opennewsletter/prod secret with put-secret-value rather than
# creating a new one — see operator-setup.md for the jq recipe
rm cf-signing-prod.key
```

If `opennewsletter/prod` already exists holding only `cdn_signing_private_key` (tracked in [`PROGRESS.md`](../../plans/PROGRESS.md) B5), merge the other keys into it with `put-secret-value` rather than creating a new secret, and skip the key-generation block above unless you're rotating the key.

Set `BOOTSTRAP_ADMIN_EMAIL` in the prod `.env.local`/config to the email the prod admin will sign in with ([D5](../decisions.md#d5)) — see "Bootstrap the production admin" below.

### Deploy prod stacks

Prod deploys go through the `promote-prod` GitHub Actions job ([D6](../decisions.md#d6)), gated by the `production` environment's required reviewer, not a manual `cdk deploy` from an operator's machine — see [`.github/workflows/README.md`](../../.github/workflows/README.md) and [`testing-and-ci.md` §6](../development/testing-and-ci.md#6-github-actions-workflows). The command below is for first bring-up before that workflow has run, or for break-glass recovery:

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

### Bootstrap the production admin

In prod ([D5](../decisions.md#d5)), `scripts/bootstrap_admin.py` creates no Cognito user: it writes the Group, the first `voting` Newsletter, and an admin invite bound to `BOOTSTRAP_ADMIN_EMAIL` (`Invite.bound_email`, lowercased; `role_on_redeem=admin`; 30-day TTL), then prints the `/join?code=…` URL.

```sh
python scripts/bootstrap_admin.py \
  --env prod \
  --group-name "OpenNewsletter"
```

The email comes from `BOOTSTRAP_ADMIN_EMAIL`; the script exits without it. `--admin-email` is optional in prod and only cross-checked against it, and `--admin-display-name`/`--password` are ignored (the display name comes from the Google profile at redemption). The table name comes from `cdk.out/prod-outputs.json` if present, otherwise `OpenNewsletter-prod`. Every put is conditional on the row not existing, so a re-run can't reset a live group.

Open the printed URL and sign in with **Google**, using that exact email — the invite's `bound_email` check requires the ID token's `email` (lowercased) to match and `email_verified` to be true, so Facebook (which doesn't supply `email_verified`) won't work for this redemption. The normal redeem path then creates the User + membership, same as any other invite. There is no password auth in prod and no `admin-bootstrap` Cognito client there ([`environments.md` §6](../development/environments.md#6-cognito-strategy-in-dev)) — this invite-based flow is the only way to bootstrap the first prod admin.

### Smoke test production

1. Navigate to `https://opennewsletter.example.com`.
2. Sign in via Google.
3. Generate an invite, share the URL, redeem in a second account.
4. Enable push notifications and verify a test notification arrives.
5. Sign up a fresh account via **Facebook**. In the Cognito console, its `email_verified` attribute must be `false` (or absent). PreSignUp no longer auto-verifies federated emails ([`auth.md` §3.3](../design/auth.md#33-presignup-logic)). If it's `true`, a Facebook user could redeem an email-bound invite, so stop and investigate.

**M14 is verified when**: a real user can sign in and use the app at the production domain.

---

## Operations

### Drafts that auto-publish gave up on

At a cycle's close, the tick publishes every non-empty draft. A draft that keeps failing is retried on the next ticks. After `AUTO_PUBLISH_MAX_FAILED_SWEEPS` (3) failed passes, the edition publishes without it and the draft stays `draft` ([`lifecycle.md` §5.3](../design/lifecycle.md#53-publishnl)). This emits `AutoPublishAbandoned`, which trips the `OpenNewsletter-CycleTick-Errors-prod` alarm.

1. **Find the drafts.** In CloudWatch Logs Insights, on `/aws/lambda/OpenNewsletter-CycleTick-prod`:

   ```
   fields @timestamp, @message
   | filter @message like /failed to auto-publish draft|unreadable answer row|failed to list answers for auto-publish|auto-publish kept failing/
   | sort @timestamp desc
   ```

   Each line names the group, cycle and question, plus either a `response_id`, the row's sort key `sk` (`A#{userId}`), or (for a listing failure) no row at all. A listing failure means every draft under that question was skipped.
2. **Fix the cause.** An `unreadable answer row` is corrupt data. Inspect the item at `pk = GROUP#{groupId}#NL#{cycleId}#Q#{questionId}`, `sk = A#{userId}`, and repair or delete it. Anything else (throttling, a validation error) shows in the log line's `error` field.
3. **Publish each draft by hand.** Do what the tick would have done: set `status` and `published_at`, the latter to the cycle's `response_close_at` as RFC 3339:

   ```sh
   aws dynamodb update-item --table-name OpenNewsletter-prod \
     --key '{"pk":{"S":"GROUP#<g>#NL#<c>#Q#<q>"},"sk":{"S":"A#<u>"}}' \
     --update-expression 'SET #s = :p, published_at = :at' \
     --condition-expression '#s = :d' \
     --expression-attribute-names '{"#s":"status"}' \
     --expression-attribute-values '{":p":{"S":"published"},":d":{"S":"draft"},":at":{"S":"<responseCloseAt>"}}'
   ```

   Only publish a draft with content: a non-blank `body` or a live image for text, or a `poll_option_id` for polls.
4. **Count the edition.** If the member has no other published answer in that cycle, bump their membership the same way, so the edition counts once:

   ```sh
   aws dynamodb update-item --table-name OpenNewsletter-prod \
     --key '{"pk":{"S":"USER#<u>"},"sk":{"S":"GROUP#<g>"}}' \
     --update-expression 'SET editions_answered = editions_answered + :one, last_answered_cycle = :c' \
     --condition-expression 'attribute_not_exists(last_answered_cycle) OR last_answered_cycle <> :c' \
     --expression-attribute-values '{":one":{"N":"1"},":c":{"S":"<c>"}}'
   ```

   A `ConditionalCheckFailedException` here means the edition was already counted. That's fine.
