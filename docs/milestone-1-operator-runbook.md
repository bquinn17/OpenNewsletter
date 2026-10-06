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
- Create a Secrets Manager secret `opennewsletter/oauth/google/dev` with
  `{"clientId": "...", "clientSecret": "..."}`.

**`.env.local`**: `GOOGLE_OAUTH_SECRET_ARN`.

(No Apple Sign In step — dropped. Apple Developer membership is $99/yr,
which the owner declined; see decision in `plans/PROGRESS.md`.)

---

## 4. Facebook Login

Meta for Developers → Create App → Add Facebook Login product.

- Valid OAuth Redirect URI: Cognito URI above.
- Secrets Manager `opennewsletter/oauth/facebook/dev` with
  `{"clientId","clientSecret"}`.

**`.env.local`**: `FACEBOOK_OAUTH_SECRET_ARN`.

---

## 5. VAPID keys for Web Push

```sh
pip install py-vapid
python scripts/generate_vapid_keys.py --subject mailto:you@example.com --out vapid.json
aws secretsmanager create-secret --name opennewsletter/vapid/dev \
  --secret-string file://vapid.json
rm vapid.json
```

**`.env.local`**: `VAPID_SECRET_ARN`.

---

## 6. CloudFront signing keypair

CloudFront signed URLs/cookies accept either RSA 2048 or ECDSA P-256 keys;
EC is smaller/faster to generate and works the same way with the CDK
`PublicKey`/`KeyGroup` constructs:

```sh
openssl ecparam -name prime256v1 -genkey -noout -out cf-signing.key
openssl ec -in cf-signing.key -pubout -out infra/keys/cf-signing.pub.pem
aws secretsmanager create-secret --name opennewsletter/cdn-signing/dev \
  --secret-string file://cf-signing.key
rm cf-signing.key   # never commit; only the .pub.pem is checked in
git add infra/keys/cf-signing.pub.pem
```

(RSA 2048 via `openssl genrsa` / `openssl rsa -pubout` also works if you'd
rather stick with that.)

**`.env.local`**: `CDN_SIGNING_SECRET_ARN`.

---

## 7. Operator contact email

Used for AWS Budgets and CloudWatch alarm notifications.

**`.env.local`**: `ALARM_EMAIL`.

---

## Done when

All ARNs/IDs above are recorded in `.env.local`. They graduate into
`infra/opennewsletter/config.py` during Milestone 3.
