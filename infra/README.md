# infra

AWS CDK in Python. One stack per concern (data, auth, media, API, notifications, frontend DNS, monitoring). Source of truth for AWS resources. See [`../plans/01-infrastructure-cdk.md`](../plans/01-infrastructure-cdk.md) and [`../plans/coding-standards.md`](../plans/coding-standards.md) §4.

Common commands:

- `pip install -r requirements.txt`
- `cdk synth --context env=dev`
- `cdk deploy --context env=dev <StackName>`
- `pytest tests/`
