# keys

Public key material checked into the repo. **Never commit private keys here.**

CloudFront signing public keys, one **RSA-2048** pair per env:

| File | Env | Private half |
|---|---|---|
| `cf-signing.pub.pem` | dev | `cdn_signing_private_key` in `opennewsletter/dev` |
| `cf-signing-prod.pub.pem` | prod | `cdn_signing_private_key` in `opennewsletter/prod` |

`MediaPersistentStack` reads the env's file to create the CloudFront `PublicKey` + `KeyGroup`.
See [`docs/design/auth.md` §10](../../docs/design/auth.md#10-identity-provider-setup-design-relevant-facts) and
[`docs/design/infrastructure.md` §5](../../docs/design/infrastructure.md#5-mediapersistentstack-and-mediapipelinestack).

**Use RSA, not ECDSA.** `lambda-media` signs with RSA-SHA1 only, so an EC key
deploys cleanly and then fails every `GET /media-cookie`.
`infra/tests/test_cf_signing_keys.py` checks that both files are distinct
RSA-2048 keys.

Generate a pair (shown for prod; for dev, write `cf-signing.pub.pem` instead):

```sh
openssl genrsa -out cf-signing-prod.key 2048
openssl rsa -pubout -in cf-signing-prod.key -out cf-signing-prod.pub.pem
# store cf-signing-prod.key as cdn_signing_private_key in opennewsletter/prod, then delete it locally
```

Changing a public key replaces the CloudFront `PublicKey` on the next deploy,
so the key-pair ID changes with it. `lambda-media` picks the new ID up from
CDK, but cookies signed with the old key stop working (they last at most an hour).
