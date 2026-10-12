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
from aws_cdk import aws_ssm as ssm
from constructs import Construct

from .config import EnvConfig
from .ssm_refs import cloudfront_distribution_id_parameter_name

_KEYS_DIR = Path(__file__).parent.parent / "keys"
# CloudFront signing public key per env; the private halves live in each
# env's app secret under `cdn_signing_private_key`.
_SIGNING_PUBLIC_KEY_FILES = {
    "dev": "cf-signing.pub.pem",
    "prod": "cf-signing-prod.pub.pem",
}


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
        removal_policy = (
            cdk.RemovalPolicy.DESTROY if is_dev else cdk.RemovalPolicy.RETAIN
        )

        # The SPA's origins: browser uploads here, and CDN reads (below).
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
            # (`plans/01-infrastructure-cdk.md` §5.1).
            versioned=False,
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

        self.avatars_processed_bucket = s3.Bucket(
            self,
            "AvatarsProcessedBucket",
            bucket_name=f"opennewsletter-avatars-processed-{config.env}-{self.account}",
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
        # Each env has its own RSA-2048 pair, so a leaked dev private key can't
        # sign prod cookies (`plans/05-auth-flow.md` §10).
        pub_key_path = _KEYS_DIR / _SIGNING_PUBLIC_KEY_FILES[config.env]
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
        avatars_processed_origin = origins.S3BucketOrigin.with_origin_access_control(
            self.avatars_processed_bucket
        )

        # CORS on every CDN response, so the SPA can load images with
        # `crossorigin`. Without it they load no-cors, the service worker sees
        # opaque status-0 responses, and its `cdn-images` cache can't tell a 403
        # from an image, so it stores nothing (`04` §5.2). Credentialed because
        # prod's signed cookies ride along; that requires an explicit origin
        # list, never `*`.
        media_cors_policy = cloudfront.ResponseHeadersPolicy(
            self,
            "MediaCorsPolicy",
            comment=f"OpenNewsletter CDN CORS for the SPA ({config.env})",
            cors_behavior=cloudfront.ResponseHeadersCorsBehavior(
                access_control_allow_credentials=True,
                access_control_allow_headers=["Range"],
                access_control_allow_methods=["GET", "HEAD"],
                access_control_allow_origins=spa_origins,
                access_control_max_age=cdk.Duration.minutes(10),
                origin_override=True,
            ),
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
                response_headers_policy=media_cors_policy,
            ),
            additional_behaviors={
                "/avatar/*": cloudfront.BehaviorOptions(
                    origin=avatars_processed_origin,
                    viewer_protocol_policy=cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
                    cache_policy=cloudfront.CachePolicy.CACHING_OPTIMIZED,
                    allowed_methods=cloudfront.AllowedMethods.ALLOW_GET_HEAD,
                    response_headers_policy=media_cors_policy,
                )
            },
        )

        # Store the key pair ID from the public key for use by lambda-media
        self.cdn_key_pair_id = cf_public_key.public_key_id
        # For the media-delete CloudFront invalidations ApiStack's Lambdas issue
        # (`plans/01-infrastructure-cdk.md` §6.2). ApiStack already depends on
        # this stack for the buckets and key pair id, so this adds no new edge.
        self.distribution_id = self.distribution.distribution_id

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
        cdk.CfnOutput(
            self,
            "AvatarsProcessedBucketName",
            value=self.avatars_processed_bucket.bucket_name,
        )
        cdk.CfnOutput(
            self, "CloudFrontDistributionId", value=self.distribution.distribution_id
        )
        cdk.CfnOutput(
            self, "CloudFrontDomain", value=self.distribution.distribution_domain_name
        )
        cdk.CfnOutput(self, "CloudFrontKeyGroupId", value=self.key_group.key_group_id)
        cdk.CfnOutput(
            self,
            "CloudFrontKeyPairId",
            value=self.cdn_key_pair_id,
        )

        # Published for MonitoringStack (volatile; see ssm_refs.py for why it
        # can't take a construct reference onto this persistent stack).
        ssm.StringParameter(
            self,
            "CloudFrontDistributionIdParam",
            parameter_name=cloudfront_distribution_id_parameter_name(config.env),
            string_value=self.distribution.distribution_id,
        )
