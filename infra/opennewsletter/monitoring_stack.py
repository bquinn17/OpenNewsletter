"""MonitoringStack — CloudWatch dashboard, alarms, and the SNS topic they page.

Per `plans/01-infrastructure-cdk.md` §9. Every env gets the dashboard; only
prod gets alarms (and the SNS topic they page), capped at the 10 alarm metrics
CloudWatch's free tier covers (§9.2). The cost budget is account-wide and lives
in `CiStack` (§9.2a), not here. All of this is built from the fixed
naming scheme other stacks use for their resources (`OpenNewsletter-{Name}-{env}`
for Lambdas, `OpenNewsletter-{env}` for the table) rather than passed in as
construct references: MonitoringStack is volatile (`make redeploy-volatile`
destroys and recreates it — `01-infrastructure-cdk.md` §10.5, `Makefile`
`VOLATILE_STACKS`), and a construct reference from a persistent stack would
create a CloudFormation export that blocks that stack from ever being updated
or destroyed while this one still imports it.

The HTTP API id and the CloudFront distribution id aren't deterministic like
the names above (AWS assigns them at deploy time), so those come from SSM
parameters the producer stacks (`ApiStack`, `MediaPersistentStack`) publish —
see `ssm_refs.py` for why a CloudFormation dynamic reference is the right tool
here rather than a lookup or a construct reference.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_cloudwatch as cloudwatch
from aws_cdk import aws_cloudwatch_actions as cw_actions
from aws_cdk import aws_sns as sns
from aws_cdk import aws_sns_subscriptions as subscriptions
from constructs import Construct

from .config import EnvConfig
from .ssm_refs import (
    cloudfront_distribution_id_parameter_name,
    http_api_id_parameter_name,
    ssm_dynamic_ref,
)

# Short Lambda names, matching the {Name} segment of every
# `OpenNewsletter-{Name}-{env}` function_name across AuthStack (PreSignUp),
# ApiStack (the eight request handlers), NotificationsStack (the two tick
# Lambdas) and MediaPipelineStack (ImageProcess). Kept in one place so the
# dashboard can't drift from a Lambda that gets its `function_name` changed.
_LAMBDA_NAMES: tuple[str, ...] = (
    "PreSignUp",
    "Invites",
    "Groups",
    "Newsletters",
    "Questions",
    "Responses",
    "Engagement",
    "Media",
    "Push",
    "CycleTick",
    "NotifyTick",
    "ImageProcess",
)

# The free CloudWatch tier covers 10 alarm metrics; a metric-math alarm costs
# one per metric it reads (`01` §9.2). Prod's alarms must fit inside it.
MAX_ALARM_METRICS = 10

_FIVE_MIN = cdk.Duration.minutes(5)


def _lambda_function_name(name: str, env: str) -> str:
    return f"OpenNewsletter-{name}-{env}"


def _lambda_metric(name: str, env: str, metric_name: str) -> cloudwatch.Metric:
    return cloudwatch.Metric(
        namespace="AWS/Lambda",
        metric_name=metric_name,
        dimensions_map={"FunctionName": _lambda_function_name(name, env)},
        statistic=cloudwatch.Stats.SUM,
        period=_FIVE_MIN,
    )


class MonitoringStack(cdk.Stack):
    def __init__(
        self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any
    ) -> None:
        super().__init__(scope, id, **kwargs)

        self._env = config.env
        table_name = f"OpenNewsletter-{config.env}"
        http_api_id = ssm_dynamic_ref(http_api_id_parameter_name(config.env))
        cloudfront_distribution_id = ssm_dynamic_ref(
            cloudfront_distribution_id_parameter_name(config.env)
        )

        self.dashboard = cloudwatch.Dashboard(
            self,
            "Dashboard",
            dashboard_name=f"OpenNewsletter-{config.env}",
        )
        self.dashboard.add_widgets(
            cloudwatch.TextWidget(
                markdown=f"# OpenNewsletter ({config.env})",
                width=24,
                height=1,
            )
        )
        self.dashboard.add_widgets(*self._api_widgets(http_api_id))
        for lambda_name in _LAMBDA_NAMES:
            self.dashboard.add_widgets(self._lambda_widget(lambda_name, config.env))
        self.dashboard.add_widgets(self._dynamo_widget(table_name))
        self.dashboard.add_widgets(self._cloudfront_widget(cloudfront_distribution_id))
        self.dashboard.add_widgets(self._push_widget(config.env))

        cdk.CfnOutput(self, "DashboardName", value=self.dashboard.dashboard_name)

        # --- Alarms (prod only, `01` §9.2) ---
        # Dev has no alarms and so no topic either: nothing would publish to it.

        self.alarm_topic: sns.Topic | None = None
        if config.env != "prod":
            return

        topic = sns.Topic(
            self,
            "AlarmTopic",
            topic_name=f"OpenNewsletter-Alarms-{config.env}",
            display_name=f"OpenNewsletter {config.env} alarms",
        )
        if config.alarm_email:
            topic.add_subscription(subscriptions.EmailSubscription(config.alarm_email))
        self.alarm_topic = topic

        # 2 + 2 + 2 + 2 + 2 = 10 alarm metrics, exactly MAX_ALARM_METRICS.
        alarms = [
            self._add_lambda_error_rate_alarm(),
            self._add_api_5xx_alarm(http_api_id),
            self._add_dynamo_throttle_alarm(table_name),
            self._add_push_failure_rate_alarm(config.env),
            self._add_cycle_tick_errors_alarm(config.env),
        ]
        for alarm in alarms:
            alarm.add_alarm_action(cw_actions.SnsAction(topic))

        cdk.CfnOutput(self, "AlarmTopicArn", value=topic.topic_arn)

    # --- Dashboard widgets ---

    def _api_widgets(self, http_api_id: str) -> list[cloudwatch.IWidget]:
        dims = {"ApiId": http_api_id}
        count = cloudwatch.Metric(
            namespace="AWS/ApiGateway",
            metric_name="Count",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
            label="Requests",
        )
        c4xx = cloudwatch.Metric(
            namespace="AWS/ApiGateway",
            metric_name="4xx",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
            label="4xx",
        )
        c5xx = cloudwatch.Metric(
            namespace="AWS/ApiGateway",
            metric_name="5xx",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
            label="5xx",
        )
        latency_widgets = [
            cloudwatch.Metric(
                namespace="AWS/ApiGateway",
                metric_name="Latency",
                dimensions_map=dims,
                statistic=stat,
                period=_FIVE_MIN,
                label=stat,
            )
            for stat in ("p50", "p95", "p99")
        ]
        return [
            cloudwatch.GraphWidget(
                title="HTTP API — requests / 4xx / 5xx",
                left=[count, c4xx, c5xx],
                width=12,
                height=6,
            ),
            cloudwatch.GraphWidget(
                title="HTTP API — latency p50/p95/p99",
                left=latency_widgets,
                width=12,
                height=6,
            ),
        ]

    def _lambda_widget(self, name: str, env: str) -> cloudwatch.GraphWidget:
        return cloudwatch.GraphWidget(
            title=f"Lambda — {name}",
            left=[
                _lambda_metric(name, env, "Invocations").with_(label="Invocations"),
                _lambda_metric(name, env, "Errors").with_(label="Errors"),
                _lambda_metric(name, env, "Throttles").with_(label="Throttles"),
            ],
            right=[
                cloudwatch.Metric(
                    namespace="AWS/Lambda",
                    metric_name="Duration",
                    dimensions_map={"FunctionName": _lambda_function_name(name, env)},
                    statistic="p95",
                    period=_FIVE_MIN,
                    label="Duration p95",
                )
            ],
            width=12,
            height=6,
        )

    def _dynamo_widget(self, table_name: str) -> cloudwatch.GraphWidget:
        dims = {"TableName": table_name}
        return cloudwatch.GraphWidget(
            title="DynamoDB — capacity, throttles, conditional-check failures",
            left=[
                cloudwatch.Metric(
                    namespace="AWS/DynamoDB",
                    metric_name="ConsumedReadCapacityUnits",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    label="Consumed RCU",
                ),
                cloudwatch.Metric(
                    namespace="AWS/DynamoDB",
                    metric_name="ConsumedWriteCapacityUnits",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    label="Consumed WCU",
                ),
            ],
            right=[
                cloudwatch.Metric(
                    namespace="AWS/DynamoDB",
                    metric_name="ReadThrottleEvents",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    label="Read throttles",
                ),
                cloudwatch.Metric(
                    namespace="AWS/DynamoDB",
                    metric_name="WriteThrottleEvents",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    label="Write throttles",
                ),
                cloudwatch.Metric(
                    namespace="AWS/DynamoDB",
                    metric_name="ConditionalCheckFailedRequests",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    label="Conditional check failures",
                ),
            ],
            width=24,
            height=6,
        )

    def _cloudfront_widget(
        self, cloudfront_distribution_id: str
    ) -> cloudwatch.GraphWidget:
        # AWS/CloudFront metrics are published only in us-east-1, regardless
        # of which region the dashboard itself lives in, and require the
        # literal "Global" Region dimension value.
        dims = {"DistributionId": cloudfront_distribution_id, "Region": "Global"}
        return cloudwatch.GraphWidget(
            title="CloudFront — requests / error rates",
            left=[
                cloudwatch.Metric(
                    namespace="AWS/CloudFront",
                    metric_name="Requests",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=_FIVE_MIN,
                    region="us-east-1",
                    label="Requests",
                ),
            ],
            right=[
                cloudwatch.Metric(
                    namespace="AWS/CloudFront",
                    metric_name="4xxErrorRate",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.AVERAGE,
                    period=_FIVE_MIN,
                    region="us-east-1",
                    label="4xx rate",
                ),
                cloudwatch.Metric(
                    namespace="AWS/CloudFront",
                    metric_name="5xxErrorRate",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.AVERAGE,
                    period=_FIVE_MIN,
                    region="us-east-1",
                    label="5xx rate",
                ),
            ],
            width=24,
            height=6,
        )

    def _push_widget(self, env: str) -> cloudwatch.GraphWidget:
        dims: dict[str, str] = {}
        return cloudwatch.GraphWidget(
            title="Web Push — sent / failed",
            left=[
                cloudwatch.Metric(
                    namespace=f"OpenNewsletter/{env}",
                    metric_name="PushSent",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=cdk.Duration.minutes(30),
                    label="Sent",
                ),
                cloudwatch.Metric(
                    namespace=f"OpenNewsletter/{env}",
                    metric_name="PushFailed",
                    dimensions_map=dims,
                    statistic=cloudwatch.Stats.SUM,
                    period=cdk.Duration.minutes(30),
                    label="Failed",
                ),
            ],
            width=24,
            height=6,
        )

    # --- Alarms ---

    def _add_api_5xx_alarm(self, http_api_id: str) -> cloudwatch.Alarm:
        dims = {"ApiId": http_api_id}
        count = cloudwatch.Metric(
            namespace="AWS/ApiGateway",
            metric_name="Count",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        c5xx = cloudwatch.Metric(
            namespace="AWS/ApiGateway",
            metric_name="5xx",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        expression = cloudwatch.MathExpression(
            expression="(errors / FILL(requests, 0)) * 100",
            using_metrics={"errors": c5xx, "requests": count},
            period=_FIVE_MIN,
        )
        alarm = cloudwatch.Alarm(
            self,
            "ApiFiveXxRateAlarm",
            metric=expression,
            threshold=1,
            evaluation_periods=2,
            treat_missing_data=cloudwatch.TreatMissingData.NOT_BREACHING,
            alarm_description="HTTP API 5xx rate exceeds 1% over 10 minutes",
            alarm_name=f"OpenNewsletter-Api5xxRate-{self._env}",
        )
        return alarm

    def _add_lambda_error_rate_alarm(self) -> cloudwatch.Alarm:
        # Account-level aggregate (no FunctionName dimension): one metric pair
        # covers every function, where a per-Lambda alarm would need a pair per
        # function (`01` §9.2). Dev and prod share the account, so a dev error
        # burst can trip this too; the alarm says so in its description.
        invocations = cloudwatch.Metric(
            namespace="AWS/Lambda",
            metric_name="Invocations",
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        errors = cloudwatch.Metric(
            namespace="AWS/Lambda",
            metric_name="Errors",
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        expression = cloudwatch.MathExpression(
            expression="(errors / FILL(invocations, 0)) * 100",
            using_metrics={"errors": errors, "invocations": invocations},
            period=_FIVE_MIN,
        )
        alarm = cloudwatch.Alarm(
            self,
            "LambdaErrorRateAlarm",
            metric=expression,
            threshold=5,
            evaluation_periods=2,
            treat_missing_data=cloudwatch.TreatMissingData.NOT_BREACHING,
            alarm_description=(
                "Lambda error rate across every function in the account "
                "(dev included) exceeds 5% over 10 minutes"
            ),
            alarm_name=f"OpenNewsletter-LambdaErrorRate-{self._env}",
        )
        return alarm

    def _add_dynamo_throttle_alarm(self, table_name: str) -> cloudwatch.Alarm:
        dims = {"TableName": table_name}
        read_throttles = cloudwatch.Metric(
            namespace="AWS/DynamoDB",
            metric_name="ReadThrottleEvents",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        write_throttles = cloudwatch.Metric(
            namespace="AWS/DynamoDB",
            metric_name="WriteThrottleEvents",
            dimensions_map=dims,
            statistic=cloudwatch.Stats.SUM,
            period=_FIVE_MIN,
        )
        expression = cloudwatch.MathExpression(
            expression="FILL(reads, 0) + FILL(writes, 0)",
            using_metrics={"reads": read_throttles, "writes": write_throttles},
            period=_FIVE_MIN,
        )
        alarm = cloudwatch.Alarm(
            self,
            "DynamoThrottleAlarm",
            metric=expression,
            threshold=0,
            comparison_operator=cloudwatch.ComparisonOperator.GREATER_THAN_THRESHOLD,
            evaluation_periods=1,
            treat_missing_data=cloudwatch.TreatMissingData.NOT_BREACHING,
            alarm_description="DynamoDB table is being throttled",
            alarm_name=f"OpenNewsletter-DynamoThrottles-{self._env}",
        )
        return alarm

    def _add_cycle_tick_errors_alarm(self, env: str) -> cloudwatch.Alarm:
        # Any cycle-tick error at all: a stuck tick stalls every group's
        # lifecycle. Notify-tick isn't separately alarmed; its failures mostly
        # surface as push failures, already covered (`01` §9.2).
        #
        # The tick logs per-cycle failures rather than failing the invocation,
        # so Lambda `Errors` alone misses the one that loses user content:
        # auto-publish giving up on drafts after
        # `AUTO_PUBLISH_MAX_FAILED_SWEEPS` sweeps (`06` §5.3). The tick emits
        # that as the EMF metric `AutoPublishAbandoned`.
        one_minute = cdk.Duration.minutes(1)
        errors = cloudwatch.Metric(
            namespace="AWS/Lambda",
            metric_name="Errors",
            dimensions_map={"FunctionName": _lambda_function_name("CycleTick", env)},
            statistic=cloudwatch.Stats.SUM,
            period=one_minute,
        )
        abandoned = cloudwatch.Metric(
            namespace=f"OpenNewsletter/{env}",
            metric_name="AutoPublishAbandoned",
            statistic=cloudwatch.Stats.SUM,
            period=one_minute,
        )
        expression = cloudwatch.MathExpression(
            expression="FILL(errors, 0) + FILL(abandoned, 0)",
            using_metrics={"errors": errors, "abandoned": abandoned},
            period=one_minute,
        )
        alarm = cloudwatch.Alarm(
            self,
            "CycleTickErrorsAlarm",
            metric=expression,
            threshold=1,
            evaluation_periods=1,
            treat_missing_data=cloudwatch.TreatMissingData.NOT_BREACHING,
            alarm_description=(
                "CycleTick Lambda has encountered errors, or auto-publish gave up"
                " on drafts"
            ),
            alarm_name=f"OpenNewsletter-CycleTick-Errors-{env}",
        )
        return alarm

    def _add_push_failure_rate_alarm(self, env: str) -> cloudwatch.Alarm:
        # PushFailed / (PushSent + PushFailed) > 0.2 over 30 minutes, treating
        # missing data as not breaching (M11). FILL(sent, 0): in a total
        # outage PushSent has no datapoints, and an unfilled expression would
        # then be missing too — i.e. "not breaching" at a 100% failure rate.
        push_sent_metric = cloudwatch.Metric(
            namespace=f"OpenNewsletter/{env}",
            metric_name="PushSent",
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(30),
        )
        push_failed_metric = cloudwatch.Metric(
            namespace=f"OpenNewsletter/{env}",
            metric_name="PushFailed",
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(30),
        )

        alarm = cloudwatch.Alarm(
            self,
            "PushFailureRateAlarm",
            metric=cloudwatch.MathExpression(
                expression="failed / (FILL(sent, 0) + failed)",
                using_metrics={
                    "failed": push_failed_metric,
                    "sent": push_sent_metric,
                },
                # Without this the expression overrides its metrics' periods to
                # the 5-minute default, silently shrinking the 30-minute window.
                period=cdk.Duration.minutes(30),
            ),
            threshold=0.2,
            evaluation_periods=1,
            treat_missing_data=cloudwatch.TreatMissingData.NOT_BREACHING,
            alarm_description="Push failure rate exceeds 20% over 30 minutes",
            alarm_name=f"OpenNewsletter-PushFailureRate-{env}",
        )
        return alarm
