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

Create a Route53 hosted zone (or your DNS provider's equivalent). Record the
hosted zone ID.

**`.env.local`**: `ROOT_DOMAIN`, `CDN_DOMAIN`, `API_DOMAIN`, `HOSTED_ZONE_ID`.

---

## 2. Decide on a Cognito hosted-UI domain prefix

Something like `opennewsletter-dev`. The OAuth redirect URI all three IdPs
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

---

## 4. Apple Sign In

Apple Developer → Identifiers → Services ID.

- Configure Sign In With Apple. Domain: `opennewsletter.example.com`.
  Return URL: Cognito URI above.
- Generate a private key, download `.p8`.
- Secrets Manager `opennewsletter/oauth/apple/dev` with
  `{"teamId","keyId","privateKey","clientId"}` (privateKey is the `.p8`
  contents).

**`.env.local`**: `APPLE_OAUTH_SECRET_ARN`.

---

## 5. Facebook Login

Meta for Developers → Create App → Add Facebook Login product.

- Valid OAuth Redirect URI: Cognito URI above.
- Secrets Manager `opennewsletter/oauth/facebook/dev` with
  `{"clientId","clientSecret"}`.

**`.env.local`**: `FACEBOOK_OAUTH_SECRET_ARN`.

---

## 6. VAPID keys for Web Push

```sh
pip install py-vapid
python scripts/generate_vapid_keys.py --subject mailto:you@example.com --out vapid.json
aws secretsmanager create-secret --name opennewsletter/vapid/dev \
  --secret-string file://vapid.json
rm vapid.json
```

**`.env.local`**: `VAPID_SECRET_ARN`.

---

## 7. CloudFront signing keypair

```sh
openssl genrsa -out cf-signing.key 2048
openssl rsa -pubout -in cf-signing.key -out infra/keys/cf-signing.pub.pem
aws secretsmanager create-secret --name opennewsletter/cdn-signing/dev \
  --secret-string file://cf-signing.key
rm cf-signing.key   # never commit; only the .pub.pem is checked in
git add infra/keys/cf-signing.pub.pem
```

**`.env.local`**: `CDN_SIGNING_SECRET_ARN`.

---

## 8. Operator contact email

Used for AWS Budgets and CloudWatch alarm notifications.

**`.env.local`**: `ALARM_EMAIL`.

---

## Done when

All ARNs/IDs above are recorded in `.env.local`. They graduate into
`infra/opennewsletter/config.py` during Milestone 3.
