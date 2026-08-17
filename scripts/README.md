# scripts

Operator and dev utilities:

- `bootstrap_admin.py` — creates the first group + admin user. See [`../plans/05-auth-flow.md`](../plans/05-auth-flow.md) §9.1.
- `generate_vapid_keys.py` — generates a VAPID keypair for Web Push.
- `seed_dev_data.py` — idempotent + destructive dev seed. See [`../plans/13-dev-environments.md`](../plans/13-dev-environments.md) §4.
- `codegen_types.sh` — runs `openapi-typescript` and the Rust contract-test generator against `shared/openapi.yaml`. *(Not yet written; lands in M5 with `openapi.yaml` itself.)*
