"""MonitoringStack — CloudWatch dashboard, SNS alarm topic, and alarms skeleton.

Alarms referencing specific Lambda ARNs and API IDs are added in M13 once all
stacks exist. The SNS topic and dashboard are created now so stack outputs are
stable from M3 onward.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_budgets as budgets
from aws_cdk import aws_cloudwatch as cloudwatch
from aws_cdk import aws_cloudwatch_actions as cw_actions
from aws_cdk import aws_sns as sns
from aws_cdk import aws_sns_subscriptions as subscriptions
from constructs import Construct

from .config import EnvConfig


class MonitoringStack(cdk.Stack):
    def __init__(
        self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any
    ) -> None:
        super().__init__(scope, id, **kwargs)

        self.alarm_topic = sns.Topic(
            self,
            "AlarmTopic",
            topic_name=f"OpenNewsletter-Alarms-{config.env}",
            display_name=f"OpenNewsletter {config.env} alarms",
        )

        if config.alarm_email:
            self.alarm_topic.add_subscription(
                subscriptions.EmailSubscription(config.alarm_email)
            )

        self.dashboard = cloudwatch.Dashboard(
            self,
            "Dashboard",
            dashboard_name=f"OpenNewsletter-{config.env}",
        )

        # Placeholder text widget — replaced with metric widgets in M13.
        self.dashboard.add_widgets(
            cloudwatch.TextWidget(
                markdown=(
                    "# OpenNewsletter\n"
                    "Metric widgets are wired in M13 once all stacks are deployed."
                ),
                width=24,
                height=3,
            )
        )

        # AWS Budgets alarm — alert at 80 % actual spend ($8) and 100 % forecasted spend ($10).
        # Created unconditionally so the budget exists even if alarm_email is absent;
        # the SNS subscriber is optional.
        budgets.CfnBudget(
            self,
            "MonthlyCostBudget",
            budget=budgets.CfnBudget.BudgetDataProperty(
                budget_type="COST",
                time_unit="MONTHLY",
                budget_limit=budgets.CfnBudget.SpendProperty(amount=10, unit="USD"),
                budget_name=f"OpenNewsletter-{config.env}-monthly",
            ),
            notifications_with_subscribers=(
                [
                    budgets.CfnBudget.NotificationWithSubscribersProperty(
                        notification=budgets.CfnBudget.NotificationProperty(
                            notification_type="ACTUAL",
                            comparison_operator="GREATER_THAN",
                            threshold=80,
                            threshold_type="PERCENTAGE",
                        ),
                        subscribers=[
                            budgets.CfnBudget.SubscriberProperty(
                                subscription_type="EMAIL",
                                address=config.alarm_email,
                            )
                        ],
                    ),
                    budgets.CfnBudget.NotificationWithSubscribersProperty(
                        notification=budgets.CfnBudget.NotificationProperty(
                            notification_type="FORECASTED",
                            comparison_operator="GREATER_THAN",
                            threshold=100,
                            threshold_type="PERCENTAGE",
                        ),
                        subscribers=[
                            budgets.CfnBudget.SubscriberProperty(
                                subscription_type="EMAIL",
                                address=config.alarm_email,
                            )
                        ],
                    ),
                ]
                if config.alarm_email
                else []
            ),
        )

        # Push failure-rate alarm: PushFailed / (PushSent + PushFailed) > 0.2
        # over 30 minutes, treating missing data as not breaching (M11).
        # FILL(sent, 0): in a total outage PushSent has no datapoints, and an
        # unfilled expression would then be missing too — i.e. "not
        # breaching" at a 100% failure rate.
        push_sent_metric = cloudwatch.Metric(
            namespace=f"OpenNewsletter/{config.env}",
            metric_name="PushSent",
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(30),
        )
        push_failed_metric = cloudwatch.Metric(
            namespace=f"OpenNewsletter/{config.env}",
            metric_name="PushFailed",
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(30),
        )

        cloudwatch.Alarm(
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
            alarm_name=f"OpenNewsletter-PushFailureRate-{config.env}",
        ).add_alarm_action(cw_actions.SnsAction(self.alarm_topic))

        # NotifyTick Lambda error alarm (M11)
        notify_tick_errors = cloudwatch.Metric(
            namespace="AWS/Lambda",
            metric_name="Errors",
            dimensions_map={
                "FunctionName": f"OpenNewsletter-NotifyTick-{config.env}",
            },
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(1),
        )

        cloudwatch.Alarm(
            self,
            "NotifyTickErrorsAlarm",
            metric=notify_tick_errors,
            threshold=1,
            evaluation_periods=1,
            alarm_description="NotifyTick Lambda has encountered errors",
            alarm_name=f"OpenNewsletter-NotifyTick-Errors-{config.env}",
        ).add_alarm_action(cw_actions.SnsAction(self.alarm_topic))

        # CycleTick Lambda error alarm (M11, mirrors NotifyTick)
        cycle_tick_errors = cloudwatch.Metric(
            namespace="AWS/Lambda",
            metric_name="Errors",
            dimensions_map={
                "FunctionName": f"OpenNewsletter-CycleTick-{config.env}",
            },
            statistic=cloudwatch.Stats.SUM,
            period=cdk.Duration.minutes(1),
        )

        cloudwatch.Alarm(
            self,
            "CycleTickErrorsAlarm",
            metric=cycle_tick_errors,
            threshold=1,
            evaluation_periods=1,
            alarm_description="CycleTick Lambda has encountered errors",
            alarm_name=f"OpenNewsletter-CycleTick-Errors-{config.env}",
        ).add_alarm_action(cw_actions.SnsAction(self.alarm_topic))

        cdk.CfnOutput(self, "AlarmTopicArn", value=self.alarm_topic.topic_arn)
        cdk.CfnOutput(self, "DashboardName", value=self.dashboard.dashboard_name)
