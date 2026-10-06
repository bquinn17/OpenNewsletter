"""NotificationsStack — EventBridge-scheduled tick Lambdas.

Per `plans/01-infrastructure-cdk.md` §7. M5 wires `lambda-cycle-tick`, which polls
GSI2 every 5 minutes and advances cycles whose transition timestamps have passed
(`plans/06-newsletter-lifecycle.md` §5). M11 adds `lambda-notify-tick`, which runs
the deadline reminder fan-out every 15 minutes.

`ApiStack` takes `cycle_tick_fn` and `notify_tick_fn` as constructor parameters so
it can wire the dev-only routes directly to these Lambdas. That makes `ApiStack`
depend on this stack, the opposite of the direction listed in
`01-infrastructure-cdk.md` §1's stack table (which has `NotificationsStack`
depending on `ApiStack`, for the M11 `lambda-push` invoke grant). See that file's
note for how M11 resolves both directions without a cycle.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_dynamodb as dynamodb
from aws_cdk import aws_events as events
from aws_cdk import aws_events_targets as events_targets
from aws_cdk import aws_iam as iam
from aws_cdk import aws_lambda
from aws_cdk import aws_logs as logs
from constructs import Construct

from .config import EnvConfig
from .lambda_assets import lambda_code


class NotificationsStack(cdk.Stack):
    def __init__(
        self,
        scope: Construct,
        id: str,
        *,
        config: EnvConfig,
        table: dynamodb.Table,
        **kwargs: Any,
    ) -> None:
        super().__init__(scope, id, **kwargs)

        log_retention = (
            logs.RetentionDays.ONE_MONTH
            if config.env == "dev"
            else logs.RetentionDays.THREE_MONTHS
        )
        removal_policy = (
            cdk.RemovalPolicy.DESTROY
            if config.env == "dev"
            else cdk.RemovalPolicy.RETAIN
        )

        cycle_tick_log_group = logs.LogGroup(
            self,
            "CycleTickLogs",
            log_group_name=f"/aws/lambda/OpenNewsletter-CycleTick-{config.env}",
            retention=log_retention,
            removal_policy=removal_policy,
        )

        self.cycle_tick_fn = aws_lambda.Function(
            self,
            "CycleTickFn",
            function_name=f"OpenNewsletter-CycleTick-{config.env}",
            description="Opens/locks/closes/publishes cycles whose timestamps have passed",
            runtime=aws_lambda.Runtime.PROVIDED_AL2023,
            architecture=aws_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=lambda_code("cycle-tick"),
            memory_size=512,
            timeout=cdk.Duration.seconds(60),
            environment={
                "TABLE_NAME": table.table_name,
                "ENV": config.env,
                "RUST_LOG": "info",
                # Fan-out target, addressed by name rather than by construct:
                # ApiStack depends on this stack, so the reverse ref would cycle.
                "PUSH_FUNCTION_NAME": f"OpenNewsletter-Push-{config.env}",
            },
            log_group=cycle_tick_log_group,
        )
        table.grant_read_write_data(self.cycle_tick_fn)

        # Grant cycle_tick_fn permission to invoke the Push Lambda
        # Use format_arn to avoid circular dependency (per 01-infrastructure-cdk.md §1 Correction (M5))
        push_lambda_arn = self.format_arn(
            service="lambda",
            resource="function",
            resource_name=f"OpenNewsletter-Push-{config.env}",
            arn_format=cdk.ArnFormat.COLON_RESOURCE_NAME,
        )
        self.cycle_tick_fn.add_to_role_policy(
            iam.PolicyStatement(
                actions=["lambda:InvokeFunction"],
                resources=[push_lambda_arn],
            )
        )

        events.Rule(
            self,
            "CycleTickSchedule",
            schedule=events.Schedule.rate(cdk.Duration.minutes(5)),
            targets=[events_targets.LambdaFunction(self.cycle_tick_fn)],
        )

        # NotifyTickFn — runs deadline reminder fan-out every 15 minutes (M11)
        notify_tick_log_group = logs.LogGroup(
            self,
            "NotifyTickLogs",
            log_group_name=f"/aws/lambda/OpenNewsletter-NotifyTick-{config.env}",
            retention=log_retention,
            removal_policy=removal_policy,
        )

        self.notify_tick_fn = aws_lambda.Function(
            self,
            "NotifyTickFn",
            function_name=f"OpenNewsletter-NotifyTick-{config.env}",
            description="Sends deadline reminder fan-outs",
            runtime=aws_lambda.Runtime.PROVIDED_AL2023,
            architecture=aws_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=lambda_code("notify-tick"),
            memory_size=512,
            timeout=cdk.Duration.seconds(120),
            environment={
                "TABLE_NAME": table.table_name,
                "ENV": config.env,
                "RUST_LOG": "info",
                "VAPID_SECRET_ARN": config.vapid_secret_arn,
            },
            log_group=notify_tick_log_group,
        )
        table.grant_read_write_data(self.notify_tick_fn)
        self.notify_tick_fn.add_to_role_policy(
            iam.PolicyStatement(
                actions=["secretsmanager:GetSecretValue"],
                resources=[config.vapid_secret_arn],
            )
        )

        events.Rule(
            self,
            "NotifyTickSchedule",
            schedule=events.Schedule.rate(cdk.Duration.minutes(15)),
            targets=[events_targets.LambdaFunction(self.notify_tick_fn)],
        )
