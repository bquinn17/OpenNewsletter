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
from aws_cdk import aws_sns as sns
from aws_cdk import aws_sns_subscriptions as subscriptions
from constructs import Construct

from .config import EnvConfig


class MonitoringStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any) -> None:
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

        cdk.CfnOutput(self, "AlarmTopicArn", value=self.alarm_topic.topic_arn)
        cdk.CfnOutput(self, "DashboardName", value=self.dashboard.dashboard_name)
