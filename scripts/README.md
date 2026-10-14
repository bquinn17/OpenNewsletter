# scripts

Operator and dev utilities:

- `bootstrap_admin.py` — creates the first group and its admin. `--env dev` makes a password-auth Cognito user (§9.1). `--env prod` instead writes the group, its first cycle, and an admin invite bound to `BOOTSTRAP_ADMIN_EMAIL`, then prints the `/join?code=…` URL (§9.1a). See [`docs/design/auth.md` §9](../docs/design/auth.md#9-bootstrap-admin--first-group). Its item builders are unit-tested in `../infra/tests/test_bootstrap_admin.py`.
- `check_lambda_binaries.sh` — fails unless every Lambda binary named in the Makefile's `FUNCTION_*` map exists under `backend/target/lambda`. CDK silently falls back to a stub for a missing binary, so `make deploy-dev` and `deploy.yml` run this check before deploying.
- `generate_vapid_keys.py` — generates a VAPID keypair for Web Push.
- `seed_dev_data.py` — idempotent + destructive dev seed. See [`docs/development/environments.md` §4](../docs/development/environments.md#4-reset-granularities).
- `codegen_types.sh` — runs `openapi-typescript` against `shared/openapi.yaml`, writing `frontend/src/types/api.ts`. Re-run after any change to the YAML (see [`coding-standards.md` §8](../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
