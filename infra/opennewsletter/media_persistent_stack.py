"""MediaPersistentStack — S3 buckets, CloudFront distribution, and signing KeyGroup.

Intentionally separated from MediaPipelineStack so that `cdk destroy` on the
volatile pipeline stack never touches the long-lived CloudFront distribution
(which takes 15–30 min to delete and recreate).
"""

from __future__ import annotations

from pathlib import Path
from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_certificatemanager as acm
from aws_cdk import aws_cloudfront as cloudfront
from aws_cdk import aws_cloudfront_origins as origins
from aws_cdk import aws_dynamodb as dynamodb
from aws_cdk import aws_s3 as s3
from constructs import Construct

from .config import EnvConfig

_KEYS_DIR = Path(__file__).parent.parent / "keys"


class MediaPersistentStack(cdk.Stack):
    def __init__(
        self,
        scope: Construct,
        id: str,
        *,
        config: EnvConfig,
        table: dynamodb.Table,
        certificate: acm.Certificate,
        **kwargs: Any,
    ) -> None:
        super().__init__(scope, id, **kwargs)

        is_dev = config.env == "dev"
        removal_policy = cdk.RemovalPolicy.DESTROY if is_dev else cdk.RemovalPolicy.RETAIN

        cors_rules = [
            s3.CorsRule(
                allowed_methods=[s3.HttpMethods.PUT, s3.HttpMethods.POST],
                allowed_origins=(
                    [f"https://{config.domain}", "http://localhost:5173"]
                    if is_dev
                    else [f"https://{config.domain}"]
                ),
                allowed_headers=[
                    "Content-Type",
                    "x-amz-content-sha256",
                    "x-amz-date",
                    "Authorization",
                ],
                exposed_headers=["ETag"],
            )
        ]

        self.originals_bucket = s3.Bucket(
            self,
            "OriginalsBucket",
            bucket_name=f"opennewsletter-media-originals-{config.env}-{self.account}",
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            versioned=not is_dev,
            encryption=s3.BucketEncryption.S3_MANAGED,
            cors=cors_rules,
            lifecycle_rules=[
                s3.LifecycleRule(
                    abort_incomplete_multipart_upload_after=cdk.Duration.days(1),
                ),
                s3.LifecycleRule(
                    transitions=[
                        s3.Transition(
                            storage_class=s3.StorageClass.INTELLIGENT_TIERING,
                            transition_after=cdk.Duration.days(30),
                        )
                    ]
                ),
            ],
            auto_delete_objects=is_dev,
            removal_policy=removal_policy,
        )

        self.processed_bucket = s3.Bucket(
            self,
            "ProcessedBucket",
            bucket_name=f"opennewsletter-media-processed-{config.env}-{self.account}",
            block_public_access=s3.BlockPublicAccess.BLOCK_ALL,
            versioned=False,
            encryption=s3.BucketEncryption.S3_MANAGED,
            lifecycle_rules=[
                s3.LifecycleRule(
                    abort_incomplete_multipart_upload_after=cdk.Duration.days(1),
                ),
                s3.LifecycleRule(
                    transitions=[
                        s3.Transition(
                            storage_class=s3.StorageClass.INTELLIGENT_TIERING,
                            transition_after=cdk.Duration.days(30),
                        )
                    ]
                ),
            ],
            auto_delete_objects=is_dev,
            removal_policy=removal_policy,
        )

        # --- CloudFront signing KeyGroup ---
        pub_key_path = _KEYS_DIR / "cf-signing.pub.pem"
        pub_key_body = pub_key_path.read_text()

        cf_public_key = cloudfront.PublicKey(
            self,
            "CdnSigningPublicKey",
            encoded_key=pub_key_body,
            comment=f"OpenNewsletter CDN signing key ({config.env})",
        )

        self.key_group = cloudfront.KeyGroup(
            self,
            "CdnKeyGroup",
            items=[cf_public_key],
            comment=f"OpenNewsletter CDN key group ({config.env})",
        )

        # --- CloudFront distribution ---
        processed_origin = origins.S3BucketOrigin.with_origin_access_control(
            self.processed_bucket
        )

        self.distribution = cloudfront.Distribution(
            self,
            "Distribution",
            domain_names=[f"cdn.{config.domain}"],
            certificate=certificate,
            price_class=cloudfront.PriceClass.PRICE_CLASS_100,
            default_behavior=cloudfront.BehaviorOptions(
                origin=processed_origin,
                viewer_protocol_policy=cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
                cache_policy=cloudfront.CachePolicy.CACHING_OPTIMIZED,
                allowed_methods=cloudfront.AllowedMethods.ALLOW_GET_HEAD,
                trusted_key_groups=[self.key_group],
            ),
        )

        # --- Outputs ---
        cdk.CfnOutput(self, "OriginalsBucketName", value=self.originals_bucket.bucket_name)
        cdk.CfnOutput(self, "OriginalsBucketArn", value=self.originals_bucket.bucket_arn)
        cdk.CfnOutput(self, "ProcessedBucketName", value=self.processed_bucket.bucket_name)
        cdk.CfnOutput(self, "ProcessedBucketArn", value=self.processed_bucket.bucket_arn)
        cdk.CfnOutput(
            self, "CloudFrontDistributionId", value=self.distribution.distribution_id
        )
        cdk.CfnOutput(
            self, "CloudFrontDomain", value=self.distribution.distribution_domain_name
        )
        cdk.CfnOutput(self, "CloudFrontKeyGroupId", value=self.key_group.key_group_id)
        cdk.CfnOutput(
            self,
            "CloudFrontSigningKeySecretArn",
            value=config.cdn_signing_secret_arn,
        )
