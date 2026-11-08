# scripts

Operator and dev utilities. The Python ones run from `infra/.venv`, with `requirements.txt` here installed into it alongside `infra/requirements.txt` ([`environments.md` §0](../docs/development/environments.md#0-workstation-setup)).

- `bootstrap_admin.py` — creates the first group and its admin. `--env dev` makes a password-auth Cognito user (§9.1). `--env prod` instead writes the group, its first cycle, and an admin invite bound to `BOOTSTRAP_ADMIN_EMAIL`, then prints the `/join?code=…` URL (§9.1a). See [`docs/design/auth.md` §9](../docs/design/auth.md#9-bootstrap-admin--first-group). Its item builders are unit-tested in `../infra/tests/test_bootstrap_admin.py`.
- `check_lambda_binaries.sh` — fails unless every Lambda binary named in the Makefile's `FUNCTION_*` map exists under `backend/target/lambda`. CDK silently falls back to a stub for a missing binary, so `make deploy-dev` and `deploy.yml` run this check before deploying.
- `generate_vapid_keys.py` — generates a VAPID keypair for Web Push.
- `seed_dev_data.py` — idempotent + destructive dev seed. See [`docs/development/environments.md` §4](../docs/development/environments.md#4-reset-granularities).
- `codegen_types.sh` — runs `openapi-typescript` against `shared/openapi.yaml`, writing `frontend/src/types/api.ts`. Re-run after any change to the YAML (see [`coding-standards.md` §8](../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
- `backfill_member_stats.py` — one-off, rerunnable recompute of every `MemberStats` row (reactions received/given, comments written, questions asked, streaks) plus the `reaction_count`/`gsi2*` attributes on `Response` rows, from the source data. Writes absolute values (`SET`, never `ADD`), so re-running it is harmless; use it once when the leaderboard feature ships and again any time to repair a missed `settle_streaks` pass or a drifted counter. `--group-id` limits it to one group (default: all groups), `--dry-run` prints the planned writes without making them. It reads, then overwrites, so a reaction or comment written while it runs can be lost from the counters; run it when the group is quiet, not just after an edition publishes, and rerun it if in doubt. Its pure computation (streak walk, self-reaction exclusion, deleted-comment exclusion, anonymous-question exclusion) is unit-tested in `tests/test_backfill_member_stats.py`, run with `pytest scripts/tests`. Semantics: [`data-model.md` §2.17](../docs/design/data-model.md#217-memberstats).

    ```
    python3 scripts/backfill_member_stats.py --env dev
    python3 scripts/backfill_member_stats.py --env prod --group-id 01HG2... --dry-run
    ```
