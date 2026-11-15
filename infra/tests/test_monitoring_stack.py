"""MonitoringStack assertions: dashboard in every env, alarms in prod only.

Per `docs/design/infrastructure.md` §9 (M13.5): prod's alarms fit inside the
10 free CloudWatch alarm metrics, each env's dashboard fits inside the 50
metrics a free dashboard may reference, dev has the dashboard and nothing else, and
the cost budget lives in CiStack (`test_ci_stack.py`), not here. The
PushFailureRate / CycleTick-Errors alarm tests stay in `test_stacks.py`.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.config import load_config
from opennewsletter.monitoring_stack import (
    _LAMBDA_NAMES,
    MAX_ALARM_METRICS,
    MAX_DASHBOARD_METRICS,
    MonitoringStack,
)

AWS_ENV = cdk.Environment(region="us-east-1")


def _template(env: str, stack_id: str) -> assertions.Template:
    app = cdk.App()
    stack = MonitoringStack(app, stack_id, config=load_config(env), env=AWS_ENV)
    return assertions.Template.from_stack(stack)


@pytest.fixture(scope="module")
def dev_template() -> assertions.Template:
    return _template("dev", "TestMonitoringStackDev")


@pytest.fixture(scope="module")
def prod_template() -> assertions.Template:
    return _template("prod", "TestMonitoringStackProd")


def _alarm_metric_count(alarm: dict[str, Any]) -> int:
    """CloudWatch bills an alarm per metric it reads: a plain alarm is one, a
    metric-math alarm is one per `MetricStat` in its `Metrics` list."""
    properties = alarm["Properties"]
    if "Metrics" in properties:
        return sum(1 for query in properties["Metrics"] if "MetricStat" in query)
    return 1


def _alarms_by_name(template: assertions.Template) -> dict[str, Any]:
    alarms = template.find_resources("AWS::CloudWatch::Alarm")
    return {a["Properties"]["AlarmName"]: a for a in alarms.values()}


# --- prod ---


def test_prod_alarms_fit_in_the_free_alarm_metrics(
    prod_template: assertions.Template,
) -> None:
    alarms = _alarms_by_name(prod_template)
    total = sum(_alarm_metric_count(alarm) for alarm in alarms.values())
    assert total <= MAX_ALARM_METRICS == 10
    assert total == 10


def test_prod_has_exactly_the_five_planned_alarms(
    prod_template: assertions.Template,
) -> None:
    assert set(_alarms_by_name(prod_template)) == {
        "OpenNewsletter-LambdaErrorRate-prod",
        "OpenNewsletter-Api5xxRate-prod",
        "OpenNewsletter-DynamoThrottles-prod",
        "OpenNewsletter-PushFailureRate-prod",
        "OpenNewsletter-CycleTick-Errors-prod",
    }


def test_every_prod_alarm_notifies_the_alarm_topic(
    prod_template: assertions.Template,
) -> None:
    topics = prod_template.find_resources("AWS::SNS::Topic")
    (topic_logical_id,) = topics.keys()
    for alarm in _alarms_by_name(prod_template).values():
        actions = alarm["Properties"]["AlarmActions"]
        assert len(actions) == 1
        # Ref to the one SNS topic, however CDK renders the Fn::GetAtt/Ref.
        assert topic_logical_id in str(actions[0])


def test_lambda_error_rate_alarm_is_account_level(
    prod_template: assertions.Template,
) -> None:
    alarm = _alarms_by_name(prod_template)["OpenNewsletter-LambdaErrorRate-prod"]
    stats = [
        q["MetricStat"] for q in alarm["Properties"]["Metrics"] if "MetricStat" in q
    ]
    assert sorted(stat["Metric"]["MetricName"] for stat in stats) == [
        "Errors",
        "Invocations",
    ]
    for stat in stats:
        assert stat["Metric"]["Namespace"] == "AWS/Lambda"
        # No FunctionName dimension: one metric pair covers every function.
        assert not stat["Metric"].get("Dimensions")
    assert alarm["Properties"]["Threshold"] == 5


def test_api_5xx_rate_alarm(prod_template: assertions.Template) -> None:
    prod_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-Api5xxRate-prod",
            "Threshold": 1,
            "EvaluationPeriods": 2,
        },
    )


def test_dynamo_throttle_alarm(prod_template: assertions.Template) -> None:
    prod_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-DynamoThrottles-prod",
            "Threshold": 0,
            "ComparisonOperator": "GreaterThanThreshold",
        },
    )


# --- dev ---


def test_dev_has_no_alarms_and_no_topic(dev_template: assertions.Template) -> None:
    dev_template.resource_count_is("AWS::CloudWatch::Alarm", 0)
    dev_template.resource_count_is("AWS::SNS::Topic", 0)


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_monitoring_stack_has_no_budget(env: str) -> None:
    # One account-wide budget, in CiStack (`docs/design/infrastructure.md` §9.2a).
    _template(env, f"TestMonitoringStackBudget{env}").resource_count_is(
        "AWS::Budgets::Budget", 0
    )


# --- dashboard (every env) ---


def test_dashboard_exists_exactly_once(dev_template: assertions.Template) -> None:
    dev_template.resource_count_is("AWS::CloudWatch::Dashboard", 1)


def test_dashboard_body_references_every_lambda(
    dev_template: assertions.Template,
) -> None:
    resources = dev_template.find_resources("AWS::CloudWatch::Dashboard")
    (dashboard,) = resources.values()
    body = str(dashboard["Properties"]["DashboardBody"])
    for name in _LAMBDA_NAMES:
        assert f"OpenNewsletter-{name}-dev" in body


def test_dashboard_body_references_the_table_and_apis(
    dev_template: assertions.Template,
) -> None:
    resources = dev_template.find_resources("AWS::CloudWatch::Dashboard")
    (dashboard,) = resources.values()
    body = str(dashboard["Properties"]["DashboardBody"])
    assert "OpenNewsletter-dev" in body  # table name
    assert "AWS/ApiGateway" in body
    assert "{{resolve:ssm:/opennewsletter/dev/http-api-id}}" in body


def _dashboard_body(template: assertions.Template) -> dict[str, Any]:
    """The dashboard body as JSON. CDK renders it as a string, or as an
    `Fn::Join` when it embeds a token; a token is always inside a JSON string,
    so a placeholder in its place keeps the body parseable."""
    (dashboard,) = template.find_resources("AWS::CloudWatch::Dashboard").values()
    body = dashboard["Properties"]["DashboardBody"]
    if isinstance(body, dict):
        _, parts = body["Fn::Join"]
        body = "".join(p if isinstance(p, str) else "TOKEN" for p in parts)
    parsed: dict[str, Any] = json.loads(body)
    return parsed


def _dashboard_metric_count(template: assertions.Template) -> int:
    """Every line on a graph counts toward the dashboard's free-tier 50,
    including each statistic of one metric. A metric-math expression would
    count each metric it reads; entries with an `expression` aren't counted
    here, so the test fails if one appears rather than guess at it."""
    count = 0
    for widget in _dashboard_body(template)["widgets"]:
        for entry in widget.get("properties", {}).get("metrics", []):
            assert not any(
                isinstance(part, dict) and "expression" in part for part in entry
            ), "count the metrics a math expression reads before adding one"
            count += 1
    return count


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_dashboard_fits_in_the_free_dashboard_metrics(env: str) -> None:
    template = _template(env, f"TestMonitoringStackDashboard{env}")
    total = _dashboard_metric_count(template)
    assert total <= MAX_DASHBOARD_METRICS == 50
    # 5 API + 37 Lambda + 3 DynamoDB + 2 push (monitoring_stack.py).
    assert total == 47


def test_dashboard_url_output(dev_template: assertions.Template) -> None:
    (output,) = dev_template.find_outputs("DashboardUrl").values()
    assert output["Value"] == (
        "https://us-east-1.console.aws.amazon.com/cloudwatch/home"
        "?region=us-east-1#dashboards/dashboard/OpenNewsletter-dev"
    )


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_monitoring_stack_has_no_cross_stack_dependencies(env: str) -> None:
    # MonitoringStack must stay destroy/redeploy-able on its own
    # (`VOLATILE_STACKS` in the Makefile) — it must never gain a CDK-level
    # dependency on a persistent stack via a construct reference.
    app = cdk.App()
    stack = MonitoringStack(
        app, "TestMonitoringStackDeps", config=load_config(env), env=AWS_ENV
    )
    assert stack.dependencies == []
