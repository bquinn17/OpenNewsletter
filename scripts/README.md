# scripts

Operator and dev utilities:

- `bootstrap_admin.py` — creates the first group + admin user. See [`../plans/05-auth-flow.md`](../plans/05-auth-flow.md) §9.1.
- `generate_vapid_keys.py` — generates a VAPID keypair for Web Push.
- `seed_dev_data.py` — idempotent + destructive dev seed. See [`../plans/13-dev-environments.md`](../plans/13-dev-environments.md) §4.
- `codegen_types.sh` — runs `openapi-typescript` against `shared/openapi.yaml`, writing `frontend/src/types/api.ts`. Re-run after any change to the YAML (see `../plans/coding-standards.md` §8).
