# Operator setup

One-time, out-of-band setup for each environment (`dev`, then `prod`): the domain, the Cognito hosted-UI prefix, the Google and Facebook OAuth apps, VAPID keys for Web Push, the CloudFront signing keypair, and the combined `opennewsletter/{env}` Secrets Manager secret they all land in, plus the alarm contact email and, for prod, the bootstrap admin's email. None of this is in CDK — a human does it once per environment before that environment's first deploy. [`auth.md` §10](../design/auth.md#10-identity-provider-setup-design-relevant-facts) summarizes why each piece exists; [`runbook.md`](runbook.md) is what you run afterward.

Values are recorded in `.env.local`, a plain `KEY=value` file you create yourself at the repo root (gitignored). [`infra/opennewsletter/config.py`](../../infra/opennewsletter/config.py)'s `load_config` reads it — via `python-dotenv` if installed, otherwise a small built-in parser — then falls back to real environment variables for any key it doesn't find, so the same keys work locally or in CI. `scripts/bootstrap_admin.py` reads the same config in prod mode. Each step below names the keys to add.

**Caution.** `load_config` reads `.env.local` the same way for `prod` as for `dev` — the file has no per-environment split — so a stray dev value (`APP_SECRET_ARN` especially) can leak into a local `prod` `cdk synth` or `cdk deploy`. Either keep `.env.local` dev-only and pass prod values as real environment variables for any local prod command, or double-check its contents first. CI has no `.env.local`, so this only affects local operator runs (tracked in [`PROGRESS.md`](../../plans/PROGRESS.md) B5).

---

## 1. Decide on a domain

Pick a real domain (e.g. `opennewsletter.example.com`). Three subdomains will be used:

- root → frontend (GitHub Pages / CloudFront)
- `cdn.` → CloudFront image distribution
- `api.` → API Gateway

Create a Route53 hosted zone (or your DNS provider's equivalent) and record the hosted zone ID — or skip Route53 entirely and manage DNS at your registrar instead (saves the $0.50/mo hosted-zone charge); leave `HOSTED_ZONE_ID`/`HOSTED_ZONE_NAME` unset in that case and add the ACM validation CNAMEs manually when `cdk deploy` prints them.

**`.env.local`**: `ROOT_DOMAIN`, `CDN_DOMAIN`, `API_DOMAIN`, `HOSTED_ZONE_ID`.

---

## 2. Decide on a Cognito hosted-UI domain prefix

Something like `opennewsletter-dev`. The OAuth redirect URI both IdPs will use is:

```
https://{prefix}.auth.us-east-1.amazoncognito.com/oauth2/idpresponse
```

You can register the IdPs with a placeholder prefix now and update it after the user pool deploys, or wait until then. Either works.

---

## 3. Google OAuth client

Google Cloud Console → APIs & Services → Credentials → Create OAuth 2.0 Client ID (Web).

- Authorized redirect URI: the Cognito URI above.
- Authorized JS origin: `https://opennewsletter.example.com`.
- Record the client ID and secret — they go into the combined secret in step 7, not a dedicated Secrets Manager secret ([D7](../decisions.md#d7)).

(No Apple Sign In step. Google and Facebook only — Apple Developer membership is $99/yr, which the owner declined.)

---

## 4. Facebook Login

Meta for Developers → Create App → Add Facebook Login product.

- Valid OAuth Redirect URI: Cognito URI above.
- Record the app ID and secret for step 7 ([D7](../decisions.md#d7)).

---

## 5. VAPID keys for Web Push

```sh
pip install py-vapid
python scripts/generate_vapid_keys.py --subject mailto:you@example.com --out vapid.json
```

Keep `vapid.json` around for step 7 — its keys fold into the combined secret rather than a dedicated `opennewsletter/vapid/dev` secret ([D7](../decisions.md#d7)) — then delete it.

---

## 6. CloudFront signing keypair

Use **RSA 2048**. `lambda-media` parses only RSA keys and signs the policy with RSA-SHA1 (`backend/crates/lambda-media/src/cloudfront.rs`, [`auth.md` §10](../design/auth.md#10-identity-provider-setup-design-relevant-facts)). An ECDSA key gets through `cdk deploy` but makes every `GET /media-cookie` fail.

```sh
openssl genrsa -out cf-signing.key 2048
openssl rsa -in cf-signing.key -pubout -out infra/keys/cf-signing.pub.pem
git add infra/keys/cf-signing.pub.pem
```

Keep `cf-signing.key` around for step 7 — it folds into the combined secret rather than a dedicated `opennewsletter/cdn-signing/dev` secret ([D7](../decisions.md#d7)) — then delete it. Never commit the private key — only the `.pub.pem` is checked in.

---

## 7. Combine credentials into one Secrets Manager secret per env

The credentials from steps 3–6 all land as keys inside **one** JSON secret per environment, `opennewsletter/{env}` ([D7](../decisions.md#d7)):

```json
{
  "google_client_id": "...",
  "google_client_secret": "...",
  "facebook_app_id": "...",
  "facebook_app_secret": "...",
  "vapid_public_key": "...",
  "vapid_private_key": "...",
  "vapid_subject": "mailto:you@example.com",
  "cdn_signing_private_key": "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n"
}
```

Build it with `jq` from the pieces collected above (adjust field names to match `vapid.json`'s actual shape and `cf-signing.key`'s contents) and create the secret:

```sh
jq -n \
  --arg google_client_id "$GOOGLE_CLIENT_ID" \
  --arg google_client_secret "$GOOGLE_CLIENT_SECRET" \
  --arg facebook_app_id "$FACEBOOK_APP_ID" \
  --arg facebook_app_secret "$FACEBOOK_APP_SECRET" \
  --arg vapid_public_key "$(jq -r .publicKey vapid.json)" \
  --arg vapid_private_key "$(jq -r .privateKey vapid.json)" \
  --arg vapid_subject "mailto:you@example.com" \
  --arg cdn_signing_private_key "$(cat cf-signing.key)" \
  '{google_client_id:$google_client_id, google_client_secret:$google_client_secret,
    facebook_app_id:$facebook_app_id, facebook_app_secret:$facebook_app_secret,
    vapid_public_key:$vapid_public_key, vapid_private_key:$vapid_private_key,
    vapid_subject:$vapid_subject, cdn_signing_private_key:$cdn_signing_private_key}' \
  > opennewsletter-dev-secret.json

aws secretsmanager create-secret --name opennewsletter/dev \
  --secret-string file://opennewsletter-dev-secret.json

rm opennewsletter-dev-secret.json vapid.json cf-signing.key   # never commit any of these
```

**Migrating from four separate secrets.** If `opennewsletter/oauth/google/dev`, `opennewsletter/oauth/facebook/dev`, `opennewsletter/vapid/dev` and `opennewsletter/cdn-signing/dev` already exist from an earlier setup, read each with `aws secretsmanager get-secret-value`, fold their values into the `jq` command above, create `opennewsletter/{env}`, update `infra/opennewsletter/config.py` and any CFN dynamic references to point at it (`{{resolve:secretsmanager:<arn>:SecretString:google_client_id}}` etc.), confirm the stack deploys clean against the new secret, then delete the four old secrets:

```sh
aws secretsmanager delete-secret --secret-id opennewsletter/oauth/google/dev
aws secretsmanager delete-secret --secret-id opennewsletter/oauth/facebook/dev
aws secretsmanager delete-secret --secret-id opennewsletter/vapid/dev
aws secretsmanager delete-secret --secret-id opennewsletter/cdn-signing/dev
```

Repeat the same for `prod` (`opennewsletter/prod`) as part of the M14 production deploy ([`build-order.md`](../../plans/build-order.md)) — see [`runbook.md`](runbook.md) "Repeat the one-time setup for prod".

**`.env.local`**: `APP_SECRET_ARN` (previously four separate ARNs: `GOOGLE_OAUTH_SECRET_ARN`, `FACEBOOK_OAUTH_SECRET_ARN`, `VAPID_SECRET_ARN`, `CDN_SIGNING_SECRET_ARN`).

---

## 8. Operator contact email

Used for AWS Budgets notifications (one account-wide $5/mo budget) and, in prod, CloudWatch alarm notifications (alarms are prod-only) — both per [D7](../decisions.md#d7).

**`.env.local`**: `ALARM_EMAIL`.

---

## 9. Prod bootstrap admin email

The email the prod bootstrap admin will sign in with via Google. Prod has no password auth — `scripts/bootstrap_admin.py --env prod` instead writes an admin invite bound to this email ([D5](../decisions.md#d5)); see [`runbook.md`](runbook.md) "Bootstrap the production admin". Not needed for dev, which keeps password bootstrap.

**`.env.local`**: `BOOTSTRAP_ADMIN_EMAIL`.

---

## Done when

All ARNs/IDs above are recorded in `.env.local` (or, in CI, as real environment variables) and read by `infra/opennewsletter/config.py`'s `load_config`.
