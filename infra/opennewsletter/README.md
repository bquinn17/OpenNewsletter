# opennewsletter

CDK stacks and configuration, one module per stack in [`docs/design/infrastructure.md` §1](../../docs/design/infrastructure.md#1-stack-inventory): `data_stack.py`, `auth_stack.py`, `media_persistent_stack.py`, `media_pipeline_stack.py`, `api_stack.py`, `notifications_stack.py`, `frontend_stack.py`, `monitoring_stack.py` and `ci_stack.py`. Supporting modules: `config.py` (per-env settings), `lambda_assets.py` (locates the `cargo-lambda` build output) and `ssm_refs.py` (SSM parameter names shared between stacks).
