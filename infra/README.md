# infra

AWS CDK in Python. One stack per concern (data, auth, media, API, notifications, frontend DNS, monitoring). Source of truth for AWS resources. See [`../plans/01-infrastructure-cdk.md`](../plans/01-infrastructure-cdk.md) and [`../plans/coding-standards.md`](../plans/coding-standards.md) §4.

First time on a new machine, use a venv rather than a bare `pip install` — on macOS, Apple's system `python3` is too old (use Homebrew's `python3.12`); on Windows/WSL2, the system `python3` (3.8) is fine. See [`../plans/13-dev-environments.md` §0](../plans/13-dev-environments.md#0-workstation-setup) for the per-OS setup, including the `cargo-lambda` and Docker prerequisites needed before `cdk deploy`.

Common commands:

- `pip install -r requirements.txt`
- `cdk synth --context env=dev`
- `cdk deploy --context env=dev <StackName>`
- `pytest tests/`
