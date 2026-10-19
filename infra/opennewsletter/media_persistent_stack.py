"""MediaPersistentStack — S3 buckets for media and avatars.

Intentionally separated from MediaPipelineStack so that `cdk destroy` on the
volatile pipeline stack never touches the long-lived S3 buckets.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_dynamodb as dynamodb
from aws_cdk import aws_s3 as s3
from constructs import Construct

from .config import EnvConfig


class MediaPersistentStack(cdk.Stack):
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

        is_dev = config.env == "dev"
        removal_policy = (
            cdk.RemovalPolicy.DESTROY if is_dev else cdk.RemovalPolicy.RETAIN
        )

        # The SPA's origins: browser uploads to the originals buckets, and
        # image reads from the processed bucket (below).
        spa_origins = (
            [f"https://{config.domain}", "http://localhost:5173"]
            if is_dev
            else [f"https://{config.domain}"]
        )
        cors_rules = [
            s3.CorsRule(
                allowed_methods=[s3.HttpMethods.PUT, s3.HttpMethods.POST],
                allowed_origins=spa_origins,
                allowed_headers=["*"],
                exposed_headers=["ETag"],
            )
        ]

        self.originals_bucket = s3.Bucket(
            self,
            "OriginalsBucket",
            bucket_name=f"opennewsletter-media-originals-{config.env}-{self.account}",
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            # No versioning in either env: a user-initiated delete must remove
            # the object, not leave it behind a delete marker
            # (`docs/design/infrastructure.md` §5.1).
            versioned=False,
            encryption=s3.BucketEncryption.S3_MANAGED,
            cors=cors_rules,
            lifecycle_rules=[
                s3.LifecycleRule(
                    abort_incomplete_multipart_upload_after=cdk.Duration.days(1),
                ),
                s3.LifecycleRule(
                    prefix="uploads/",
                    expiration=cdk.Duration.days(7),
                ),
            ],
            auto_delete_objects=is_dev,
            removal_policy=removal_policy,
        )

        # Processed bucket: images under img/* and avatars under avatar/*. The
        # SPA reads them only through presigned GET URLs the API Lambdas sign
        # (`docs/design/media.md` §4). GET/HEAD CORS because images load with
        # `crossorigin="anonymous"`, so the service worker sees real statuses
        # and can cache them; no credentials, since the signature is in the URL.
        processed_cors_rules = [
            s3.CorsRule(
                allowed_methods=[s3.HttpMethods.GET, s3.HttpMethods.HEAD],
                allowed_origins=spa_origins,
            )
        ]

        self.processed_bucket = s3.Bucket(
            self,
            "ProcessedBucket",
            bucket_name=f"opennewsletter-media-processed-{config.env}-{self.account}",
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            versioned=False,
            encryption=s3.BucketEncryption.S3_MANAGED,
            cors=processed_cors_rules,
            # No Intelligent-Tiering transition here: processed WebP variants
            # are well under the 128 KB floor where that tier ever pays for
            # itself, so the per-object transition fee would be pure cost.
            lifecycle_rules=[
                s3.LifecycleRule(
                    abort_incomplete_multipart_upload_after=cdk.Duration.days(1),
                ),
            ],
            auto_delete_objects=is_dev,
            removal_policy=removal_policy,
        )

        # --- Avatar buckets (separate pipeline, cross-group) ---
        self.avatars_originals_bucket = s3.Bucket(
            self,
            "AvatarsOriginalsBucket",
            bucket_name=f"opennewsletter-avatars-originals-{config.env}-{self.account}",
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            versioned=False,
            encryption=s3.BucketEncryption.S3_MANAGED,
            cors=cors_rules,
            lifecycle_rules=[
                s3.LifecycleRule(
                    abort_incomplete_multipart_upload_after=cdk.Duration.days(1),
                ),
                s3.LifecycleRule(
                    prefix="uploads/",
                    expiration=cdk.Duration.days(7),
                ),
            ],
            auto_delete_objects=is_dev,
            removal_policy=removal_policy,
        )

        # --- Outputs ---
        cdk.CfnOutput(
            self, "OriginalsBucketName", value=self.originals_bucket.bucket_name
        )
        cdk.CfnOutput(
            self, "OriginalsBucketArn", value=self.originals_bucket.bucket_arn
        )
        cdk.CfnOutput(
            self, "ProcessedBucketName", value=self.processed_bucket.bucket_name
        )
        cdk.CfnOutput(
            self, "ProcessedBucketArn", value=self.processed_bucket.bucket_arn
        )
        cdk.CfnOutput(
            self,
            "AvatarsOriginalsBucketName",
            value=self.avatars_originals_bucket.bucket_name,
        )

        # Origins for the SPA's CSP and service worker (`docs/design/media.md`
        # §4.4), written to its env by `scripts/write_frontend_env.py`. Built in
        # the regional virtual-hosted form the S3 SDK presigns against, which
        # the CSP must match exactly.
        def s3_origin(bucket: s3.Bucket) -> str:
            return f"https://{bucket.bucket_name}.s3.{self.region}.{self.url_suffix}"

        cdk.CfnOutput(self, "MediaBaseUrl", value=s3_origin(self.processed_bucket))
        cdk.CfnOutput(
            self,
            "UploadOrigins",
            value=" ".join(
                s3_origin(bucket)
                for bucket in (self.originals_bucket, self.avatars_originals_bucket)
            ),
        )
