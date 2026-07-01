"""MonitoringStack — CloudWatch dashboard, SNS alarm topic, and alarms skeleton.

Alarms referencing specific Lambda ARNs and API IDs are added in M13 once all
stacks exist. The SNS topic and dashboard are created now so stack outputs are
stable from M3 onward.
"""

from __future__ import annotations

import aws_cdk as cdk
from aws_cdk import aws_cloudwatch as cloudwatch
from aws_cdk import aws_sns as sns
from aws_cdk import aws_sns_subscriptions as subscriptions
from constructs import Construct

from .config import EnvConfig


class MonitoringStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: object) -> None:
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

        cdk.CfnOutput(self, "AlarmTopicArn", value=self.alarm_topic.topic_arn)
        cdk.CfnOutput(self, "DashboardName", value=self.dashboard.dashboard_name)
