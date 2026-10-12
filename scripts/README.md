# scripts

Operator and dev utilities:

- `bootstrap_admin.py` — creates the first group and its admin. `--env dev` makes a password-auth Cognito user (§9.1). `--env prod` instead writes the group, its first cycle, and an admin invite bound to `BOOTSTRAP_ADMIN_EMAIL`, then prints the `/join?code=…` URL (§9.1a). See [`../plans/05-auth-flow.md`](../plans/05-auth-flow.md) §9. Its item builders are unit-tested in `../infra/tests/test_bootstrap_admin.py`.
- `check_lambda_binaries.sh` — fails unless every Lambda binary named in the Makefile's `FUNCTION_*` map exists under `backend/target/lambda`. CDK silently falls back to a stub for a missing binary, so `make deploy-dev` and `deploy.yml` run this check before deploying.
- `generate_vapid_keys.py` — generates a VAPID keypair for Web Push.
- `seed_dev_data.py` — idempotent + destructive dev seed. See [`../plans/13-dev-environments.md`](../plans/13-dev-environments.md) §4.
- `codegen_types.sh` — runs `openapi-typescript` against `shared/openapi.yaml`, writing `frontend/src/types/api.ts`. Re-run after any change to the YAML (see `../plans/coding-standards.md` §8).
