"""MediaPipelineStack — lambda-image-process and its S3 event subscription.

This stack is deliberately volatile: destroy and redeploy it freely without
touching the slow-to-rebuild CloudFront distribution in MediaPersistentStack.

The Lambda code is a shell stub at M3; the real Rust implementation lands in M8.
"""

from __future__ import annotations

from pathlib import Path

import aws_cdk as cdk
from aws_cdk import aws_dynamodb as dynamodb
from aws_cdk import aws_iam as iam
from aws_cdk import aws_lambda as aws_lambda
from aws_cdk import aws_logs as logs
from aws_cdk import aws_s3 as s3
from aws_cdk import aws_s3_notifications as s3_notifications
from constructs import Construct

from .config import EnvConfig

_STUB_PATH = str(
    Path(__file__).parent.parent.parent / "backend" / "lambda-stubs" / "lambda-image-process"
)


class MediaPipelineStack(cdk.Stack):
    def __init__(
        self,
        scope: Construct,
        id: str,
        *,
        config: EnvConfig,
        table: dynamodb.Table,
        originals_bucket: s3.Bucket,
        processed_bucket: s3.Bucket,
        **kwargs: object,
    ) -> None:
        super().__init__(scope, id, **kwargs)

        log_group = logs.LogGroup(
            self,
            "ImageProcessLogs",
            log_group_name=f"/aws/lambda/OpenNewsletter-ImageProcess-{config.env}",
            retention=logs.RetentionDays.ONE_MONTH
            if config.env == "dev"
            else logs.RetentionDays.THREE_MONTHS,
            removal_policy=cdk.RemovalPolicy.DESTROY
            if config.env == "dev"
            else cdk.RemovalPolicy.RETAIN,
        )

        self.image_process_fn = aws_lambda.Function(
            self,
            "ImageProcessFn",
            function_name=f"OpenNewsletter-ImageProcess-{config.env}",
            runtime=aws_lambda.Runtime.PROVIDED_AL2023,
            architecture=aws_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=aws_lambda.Code.from_asset(_STUB_PATH),
            memory_size=1024,
            timeout=cdk.Duration.seconds(60),
            environment={
                "TABLE_NAME": table.table_name,
                "PROCESSED_BUCKET": processed_bucket.bucket_name,
                "RUST_LOG": "info",
                "ENV": config.env,
            },
            log_group=log_group,
        )

        # Least-privilege IAM
        table.grant(
            self.image_process_fn,
            "dynamodb:UpdateItem",
        )
        originals_bucket.grant_read(self.image_process_fn)
        processed_bucket.grant_put(self.image_process_fn)

        # S3 event subscription — fires on every upload to the originals prefix
        originals_bucket.add_event_notification(
            s3.EventType.OBJECT_CREATED,
            s3_notifications.LambdaDestination(self.image_process_fn),
            s3.NotificationKeyFilter(prefix="uploads/"),
        )
