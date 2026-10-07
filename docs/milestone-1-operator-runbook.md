# Milestone 1 — Operator runbook

Out-of-band setup that a human operator (not Claude) must complete. Tracks to
[`../plans/12-build-order.md`](../plans/12-build-order.md) Milestone 1 and
[`../plans/05-auth-flow.md`](../plans/05-auth-flow.md) §10.

Each step ends with "save the ARN/ID into `.env.local`" — that file is the
holding pen until [`../infra/opennewsletter/`](../infra/opennewsletter/)
`config.py` lands in Milestone 3. Copy [`../.env.local.example`](../.env.local.example)
to `.env.local` (gitignored) before starting.

Milestone 1 can run in parallel with Milestone 2 — M2 (backend foundations)
does not consume any of these ARNs.

**Decided 2026-10-07; lands in M13.5 (D7):** steps 3–6 below collect
credentials as before, but they're no longer each saved to their own Secrets
Manager secret. Collect them, then do step 7, which combines everything into
**one** secret per environment.

---

## 1. Decide on a domain

Pick a real domain (e.g. `opennewsletter.example.com`). Three subdomains will
be used:

- root → frontend (GitHub Pages / CloudFront)
- `cdn.` → CloudFront image distribution
- `api.` → API Gateway

Create a Route53 hosted zone (or your DNS provider's equivalent) and record
the hosted zone ID — or skip Route53 entirely and manage DNS at your
registrar instead (saves the $0.50/mo hosted-zone charge); leave
`HOSTED_ZONE_ID`/`HOSTED_ZONE_NAME` unset in that case and add the ACM
validation CNAMEs manually when `cdk deploy` prints them.

**`.env.local`**: `ROOT_DOMAIN`, `CDN_DOMAIN`, `API_DOMAIN`, `HOSTED_ZONE_ID`.

---

## 2. Decide on a Cognito hosted-UI domain prefix

Something like `opennewsletter-dev`. The OAuth redirect URI both IdPs
will use is:

```
https://{prefix}.auth.us-east-1.amazoncognito.com/oauth2/idpresponse
```

You can register the IdPs with a placeholder prefix now and update after M3
deploys the user pool, or wait until M3. Either works.

---

## 3. Google OAuth client

Google Cloud Console → APIs & Services → Credentials → Create OAuth 2.0
Client ID (Web).

- Authorized redirect URI: the Cognito URI above.
- Authorized JS origin: `https://opennewsletter.example.com`.
- Record the client ID and secret — they go into the combined secret in
  step 7, not a dedicated Secrets Manager secret (decided 2026-10-07, D7).

(No Apple Sign In step — dropped. Apple Developer membership is $99/yr,
which the owner declined; see decision in `plans/PROGRESS.md`.)

---

## 4. Facebook Login

Meta for Developers → Create App → Add Facebook Login product.

- Valid OAuth Redirect URI: Cognito URI above.
- Record the app ID and secret for step 7 (decided 2026-10-07, D7).

---

## 5. VAPID keys for Web Push

```sh
pip install py-vapid
python scripts/generate_vapid_keys.py --subject mailto:you@example.com --out vapid.json
```

Keep `vapid.json` around for step 7 instead of creating `opennewsletter/vapid/dev`
directly (decided 2026-10-07, D7); delete it once folded into the combined secret.

---

## 6. CloudFront signing keypair

Use **RSA 2048**. `lambda-media` parses only RSA keys and signs the policy with RSA-SHA1 (`backend/crates/lambda-media/src/cloudfront.rs`, `plans/05-auth-flow.md` §10). An ECDSA key gets through `cdk deploy` but makes every `GET /media-cookie` fail. (Corrected 2026-10-07: this section used to recommend ECDSA P-256.)

```sh
openssl genrsa -out cf-signing.key 2048
openssl rsa -in cf-signing.key -pubout -out infra/keys/cf-signing.pub.pem
git add infra/keys/cf-signing.pub.pem
```

Keep `cf-signing.key` around for step 7 instead of creating
`opennewsletter/cdn-signing/dev` directly (decided 2026-10-07, D7); delete it
once folded into the combined secret. Never commit the private key — only
the `.pub.pem` is checked in.

---

## 7. Combine credentials into one Secrets Manager secret per env (decided 2026-10-07; lands in M13.5, D7)

Steps 3–6 used to each get their own secret (4 per env). They now fold into
**one** JSON secret, `opennewsletter/{env}`, with these keys:

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

Build it with `jq` from the pieces collected above (adjust field names to
match `vapid.json`'s actual shape and `cf-signing.key`'s contents) and create
the secret:

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

**Migrating an existing dev setup:** if `opennewsletter/oauth/google/dev`,
`opennewsletter/oauth/facebook/dev`, `opennewsletter/vapid/dev` and
`opennewsletter/cdn-signing/dev` already exist from before M13.5, read each
with `aws secretsmanager get-secret-value`, fold their values into the `jq`
command above, create `opennewsletter/{env}`, update
`infra/opennewsletter/config.py` and any CFN dynamic references to point at
it (`{{resolve:secretsmanager:<arn>:SecretString:google_client_id}}` etc.),
confirm the stack deploys clean against the new secret, then delete the four
old secrets:

```sh
aws secretsmanager delete-secret --secret-id opennewsletter/oauth/google/dev
aws secretsmanager delete-secret --secret-id opennewsletter/oauth/facebook/dev
aws secretsmanager delete-secret --secret-id opennewsletter/vapid/dev
aws secretsmanager delete-secret --secret-id opennewsletter/cdn-signing/dev
```

Repeat the same for `prod` (`opennewsletter/prod`) during M14.

**`.env.local`**: `APP_SECRET_ARN` (replaces `GOOGLE_OAUTH_SECRET_ARN` /
`FACEBOOK_OAUTH_SECRET_ARN` / `VAPID_SECRET_ARN` / `CDN_SIGNING_SECRET_ARN`).

---

## 8. Operator contact email

Used for AWS Budgets notifications (one account-wide $5/mo budget as of
M13.5, D7) and, in prod, CloudWatch alarm notifications (alarms are prod-only
as of M13.5, D7).

**`.env.local`**: `ALARM_EMAIL`.

---

## 9. Prod bootstrap admin email (decided 2026-10-07; lands in M13.5, D5)

The email the prod bootstrap admin will sign in with via Google. Prod has no
password auth — `scripts/bootstrap_admin.py --env prod` instead writes an
admin invite bound to this email; see `docs/RUNBOOK.md` M14 "Bootstrap prod
admin". Not needed for dev, which keeps password bootstrap.

**`.env.local`**: `BOOTSTRAP_ADMIN_EMAIL`.

---

## Done when

All ARNs/IDs above are recorded in `.env.local`. They graduate into
`infra/opennewsletter/config.py` during Milestone 3.
