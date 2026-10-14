"""Parameter names for values MonitoringStack needs but can't take as a
construct reference.

MonitoringStack is volatile (`make redeploy-volatile` destroys and recreates
it), so it must not hold a CloudFormation cross-stack reference (Fn::ImportValue)
onto anything in a persistent stack — that would create an export the producer
stack can never again be updated or destroyed out from under. The HTTP API's
id and the CloudFront distribution's id are both assigned by AWS at deploy
time, not deterministic like a Lambda `function_name` or the DynamoDB table
name, so there's no literal string to put in a dimensions_map at synth time.

The fix: the producer stack (`ApiStack`, `MediaPersistentStack`) publishes the
value to an SSM parameter, and MonitoringStack reads it back via a
CloudFormation dynamic reference (`{{resolve:ssm:...}}`), the same mechanism
`api_stack.py`/`auth_stack.py` already use for Secrets Manager values. A
dynamic reference resolves at CloudFormation deploy time, not CDK synth time,
so it needs no AWS credentials during `cdk synth` and creates no CDK-level
stack dependency — MonitoringStack can be destroyed and redeployed without
touching ApiStack or MediaPersistentStack at all.
"""

from __future__ import annotations


def http_api_id_parameter_name(env: str) -> str:
    return f"/opennewsletter/{env}/http-api-id"


def cloudfront_distribution_id_parameter_name(env: str) -> str:
    return f"/opennewsletter/{env}/cloudfront-distribution-id"


def ssm_dynamic_ref(parameter_name: str) -> str:
    """A CloudFormation dynamic reference resolving to an SSM String parameter's value."""
    return f"{{{{resolve:ssm:{parameter_name}}}}}"
