"""NotificationsStack — EventBridge-scheduled tick Lambdas.

Per `plans/01-infrastructure-cdk.md` §7. M5 wires `lambda-cycle-tick`, which polls
GSI2 every 5 minutes and advances cycles whose transition timestamps have passed
(`plans/06-newsletter-lifecycle.md` §5). `lambda-notify-tick` joins this stack in
M11 (`plans/03-api-contract.md` §11a.2's dev route depends on it and isn't wired
until then) — do not add it here.

`ApiStack` takes `cycle_tick_fn` as a constructor parameter so it can wire the
dev-only `POST /admin/dev/tick/cycle` route directly to this Lambda. That makes
`ApiStack` depend on this stack, the opposite of the direction listed in
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

        log_group = logs.LogGroup(
            self,
            "CycleTickLogs",
            log_group_name=f"/aws/lambda/OpenNewsletter-CycleTick-{config.env}",
            retention=logs.RetentionDays.ONE_MONTH
            if config.env == "dev"
            else logs.RetentionDays.THREE_MONTHS,
            removal_policy=cdk.RemovalPolicy.DESTROY
            if config.env == "dev"
            else cdk.RemovalPolicy.RETAIN,
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
            },
            log_group=log_group,
        )
        table.grant_read_write_data(self.cycle_tick_fn)

        events.Rule(
            self,
            "CycleTickSchedule",
            schedule=events.Schedule.rate(cdk.Duration.minutes(5)),
            targets=[events_targets.LambdaFunction(self.cycle_tick_fn)],
        )
