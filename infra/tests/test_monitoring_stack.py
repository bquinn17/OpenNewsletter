"""MonitoringStack assertions for the M13 alarm/dashboard build-out.

The pre-existing PushFailureRate / CycleTick-Errors / NotifyTick-Errors alarm
tests stay in `test_stacks.py`; this file covers what M13 added: the per-Lambda
error-rate alarms, the HTTP API 5xx alarm, and the DynamoDB throttle alarm.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.config import EnvConfig, load_config
from opennewsletter.monitoring_stack import _LAMBDA_NAMES, MonitoringStack

AWS_ENV = cdk.Environment(region="us-east-1")


@pytest.fixture(scope="module")
def dev_config() -> EnvConfig:
    return load_config("dev")


@pytest.fixture(scope="module")
def monitoring_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    stack = MonitoringStack(app, "TestMonitoringStack2", config=dev_config, env=AWS_ENV)
    return assertions.Template.from_stack(stack)


def test_total_alarm_count(monitoring_template: assertions.Template) -> None:
    # 1 API 5xx + one error-rate alarm per Lambda (12) + 1 Dynamo throttle +
    # 2 tick-specific "any error" alarms + 1 push failure-rate alarm.
    monitoring_template.resource_count_is(
        "AWS::CloudWatch::Alarm", 1 + len(_LAMBDA_NAMES) + 1 + 2 + 1
    )


def test_every_alarm_notifies_the_alarm_topic(
    monitoring_template: assertions.Template,
) -> None:
    alarms = monitoring_template.find_resources("AWS::CloudWatch::Alarm")
    topics = monitoring_template.find_resources("AWS::SNS::Topic")
    (topic_logical_id,) = topics.keys()
    for alarm in alarms.values():
        actions = alarm["Properties"]["AlarmActions"]
        assert len(actions) == 1
        # Ref to the one SNS topic, however CDK renders the Fn::GetAtt/Ref.
        assert topic_logical_id in str(actions[0])


def test_api_5xx_rate_alarm(monitoring_template: assertions.Template) -> None:
    monitoring_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-Api5xxRate-dev",
            "Threshold": 1,
            "EvaluationPeriods": 2,
        },
    )


def test_dynamo_throttle_alarm(monitoring_template: assertions.Template) -> None:
    monitoring_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-DynamoThrottles-dev",
            "Threshold": 0,
            "ComparisonOperator": "GreaterThanThreshold",
        },
    )


def test_every_lambda_has_an_error_rate_alarm(
    monitoring_template: assertions.Template,
) -> None:
    for name in _LAMBDA_NAMES:
        monitoring_template.has_resource_properties(
            "AWS::CloudWatch::Alarm",
            {
                "AlarmName": f"OpenNewsletter-{name}-ErrorRate-dev",
                "Threshold": 5,
                "EvaluationPeriods": 2,
            },
        )


def test_dashboard_exists_exactly_once(
    monitoring_template: assertions.Template,
) -> None:
    monitoring_template.resource_count_is("AWS::CloudWatch::Dashboard", 1)


def test_dashboard_body_references_every_lambda(
    monitoring_template: assertions.Template,
) -> None:
    resources = monitoring_template.find_resources("AWS::CloudWatch::Dashboard")
    (dashboard,) = resources.values()
    body = str(dashboard["Properties"]["DashboardBody"])
    for name in _LAMBDA_NAMES:
        assert f"OpenNewsletter-{name}-dev" in body


def test_dashboard_body_references_the_table_and_apis(
    monitoring_template: assertions.Template,
) -> None:
    resources = monitoring_template.find_resources("AWS::CloudWatch::Dashboard")
    (dashboard,) = resources.values()
    body = str(dashboard["Properties"]["DashboardBody"])
    assert "OpenNewsletter-dev" in body  # table name
    assert "AWS/ApiGateway" in body
    assert "AWS/CloudFront" in body
    assert "{{resolve:ssm:/opennewsletter/dev/http-api-id}}" in body
    assert "{{resolve:ssm:/opennewsletter/dev/cloudfront-distribution-id}}" in body


def test_monitoring_stack_has_no_cross_stack_dependencies() -> None:
    # MonitoringStack must stay destroy/redeploy-able on its own
    # (`VOLATILE_STACKS` in the Makefile) — it must never gain a CDK-level
    # dependency on a persistent stack via a construct reference.
    app = cdk.App()
    stack = MonitoringStack(
        app, "TestMonitoringStackDeps", config=load_config("dev"), env=AWS_ENV
    )
    assert stack.dependencies == []
