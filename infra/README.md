# infra

AWS CDK in Python. One stack per concern (data, auth, media, API, notifications, frontend DNS, monitoring). Source of truth for AWS resources. See [`docs/design/infrastructure.md`](../docs/design/infrastructure.md) and [`docs/development/coding-standards.md` §4](../docs/development/coding-standards.md#4-python-cdk).

First time on a new machine, use a venv rather than a bare `pip install` — on macOS, Apple's system `python3` is too old (use Homebrew's `python3.12`); on Windows/WSL2, the system `python3` (3.8) is fine. See [`docs/development/environments.md` §0](../docs/development/environments.md#0-workstation-setup) for the per-OS setup, including the `cargo-lambda` and Docker prerequisites needed before `cdk deploy`.

Common commands:

- `pip install -r requirements.txt -r ../scripts/requirements.txt` (the second file is for `scripts/`, such as `make seed`)
- `cdk synth --context env=dev`
- `cdk deploy --context env=dev <StackName>`
- `pytest tests/`
