"""CDK stack assertion tests per 11-testing-ci-cd.md §3.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import sys
from pathlib import Path

# Ensure the infra package is importable regardless of working directory.
sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.auth_stack import AuthStack
from opennewsletter.config import EnvConfig, load_config
from opennewsletter.data_stack import DataStack
from opennewsletter.frontend_stack import FrontendStack
from opennewsletter.media_persistent_stack import MediaPersistentStack
from opennewsletter.media_pipeline_stack import MediaPipelineStack

AWS_ENV = cdk.Environment(region="us-east-1")


@pytest.fixture(scope="module")
def dev_config() -> EnvConfig:
    return load_config("dev")


@pytest.fixture(scope="module")
def data_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    stack = DataStack(app, "TestDataStack", config=dev_config, env=AWS_ENV)
    return assertions.Template.from_stack(stack)


@pytest.fixture(scope="module")
def auth_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    stack = AuthStack(app, "TestAuthStack", config=dev_config, env=AWS_ENV)
    return assertions.Template.from_stack(stack)


@pytest.fixture(scope="module")
def media_templates(
    dev_config: EnvConfig,
) -> tuple[assertions.Template, assertions.Template]:
    app = cdk.App()
    data_stack = DataStack(app, "TestDataStack2", config=dev_config, env=AWS_ENV)
    frontend_stack = FrontendStack(app, "TestFrontendStack", config=dev_config, env=AWS_ENV)
    persistent_stack = MediaPersistentStack(
        app,
        "TestMediaPersistentStack",
        config=dev_config,
        table=data_stack.table,
        certificate=frontend_stack.certificate,
        env=AWS_ENV,
    )
    pipeline_stack = MediaPipelineStack(
        app,
        "TestMediaPipelineStack",
        config=dev_config,
        table=data_stack.table,
        originals_bucket=persistent_stack.originals_bucket,
        processed_bucket=persistent_stack.processed_bucket,
        env=AWS_ENV,
    )
    return (
        assertions.Template.from_stack(persistent_stack),
        assertions.Template.from_stack(pipeline_stack),
    )


# ---------------------------------------------------------------------------
# DataStack
# ---------------------------------------------------------------------------


def test_dynamodb_table_count(data_template: assertions.Template) -> None:
    data_template.resource_count_is("AWS::DynamoDB::Table", 1)


def test_dynamodb_table_keys(data_template: assertions.Template) -> None:
    data_template.has_resource_properties(
        "AWS::DynamoDB::Table",
        {
            "KeySchema": assertions.Match.array_with(
                [
                    {"AttributeName": "pk", "KeyType": "HASH"},
                    {"AttributeName": "sk", "KeyType": "RANGE"},
                ]
            ),
        },
    )


def test_dynamodb_table_ttl(data_template: assertions.Template) -> None:
    data_template.has_resource_properties(
        "AWS::DynamoDB::Table",
        {
            "TimeToLiveSpecification": {
                "AttributeName": "ttl",
                "Enabled": True,
            }
        },
    )


def test_dynamodb_table_two_gsis(data_template: assertions.Template) -> None:
    resources = data_template.find_resources("AWS::DynamoDB::Table")
    assert len(resources) == 1
    table = list(resources.values())[0]
    gsis = table["Properties"]["GlobalSecondaryIndexes"]
    assert len(gsis) == 2
    index_names = {g["IndexName"] for g in gsis}
    assert index_names == {"gsi1", "gsi2"}


def test_dynamodb_gsi_projections(data_template: assertions.Template) -> None:
    resources = data_template.find_resources("AWS::DynamoDB::Table")
    table = list(resources.values())[0]
    for gsi in table["Properties"]["GlobalSecondaryIndexes"]:
        assert gsi["Projection"]["ProjectionType"] == "ALL"


def test_dynamodb_billing_mode(data_template: assertions.Template) -> None:
    data_template.has_resource_properties(
        "AWS::DynamoDB::Table",
        {"BillingMode": "PAY_PER_REQUEST"},
    )


# ---------------------------------------------------------------------------
# AuthStack
# ---------------------------------------------------------------------------


def test_user_pool_count(auth_template: assertions.Template) -> None:
    auth_template.resource_count_is("AWS::Cognito::UserPool", 1)


def test_user_pool_self_signup_disabled(auth_template: assertions.Template) -> None:
    auth_template.has_resource_properties(
        "AWS::Cognito::UserPool",
        {"AdminCreateUserConfig": {"AllowAdminCreateUserOnly": True}},
    )


def test_user_pool_exactly_two_clients(auth_template: assertions.Template) -> None:
    auth_template.resource_count_is("AWS::Cognito::UserPoolClient", 2)


def test_user_pool_three_idps(auth_template: assertions.Template) -> None:
    auth_template.resource_count_is("AWS::Cognito::UserPoolIdentityProvider", 3)


def test_user_pool_has_cognito_domain(auth_template: assertions.Template) -> None:
    auth_template.resource_count_is("AWS::Cognito::UserPoolDomain", 1)


def test_user_pool_password_policy(auth_template: assertions.Template) -> None:
    auth_template.has_resource_properties(
        "AWS::Cognito::UserPool",
        {
            "Policies": {
                "PasswordPolicy": {
                    "MinimumLength": 12,
                    "RequireLowercase": True,
                    "RequireUppercase": True,
                    "RequireNumbers": True,
                    "RequireSymbols": True,
                }
            }
        },
    )


# ---------------------------------------------------------------------------
# MediaPersistentStack
# ---------------------------------------------------------------------------


def test_s3_originals_bucket_is_private(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    persistent.has_resource_properties(
        "AWS::S3::Bucket",
        {
            "PublicAccessBlockConfiguration": {
                "BlockPublicAcls": True,
                "BlockPublicPolicy": True,
                "IgnorePublicAcls": True,
                "RestrictPublicBuckets": True,
            }
        },
    )


def test_s3_bucket_count(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    # originals + processed buckets (plus auto-delete custom resource bucket may add more,
    # but the two named buckets must exist)
    resources = persistent.find_resources("AWS::S3::Bucket")
    assert len(resources) >= 2


def test_cloudfront_distribution_exists(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    persistent.resource_count_is("AWS::CloudFront::Distribution", 1)


def test_cloudfront_has_key_group(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    persistent.resource_count_is("AWS::CloudFront::KeyGroup", 1)


def test_cloudfront_distribution_references_key_group(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    resources = persistent.find_resources("AWS::CloudFront::Distribution")
    assert len(resources) == 1
    dist = list(resources.values())[0]
    behaviors = dist["Properties"]["DistributionConfig"]["DefaultCacheBehavior"]
    assert "TrustedKeyGroups" in behaviors
    assert len(behaviors["TrustedKeyGroups"]) == 1


def test_cloudfront_redirect_to_https(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    persistent.has_resource_properties(
        "AWS::CloudFront::Distribution",
        {
            "DistributionConfig": {
                "DefaultCacheBehavior": {
                    "ViewerProtocolPolicy": "redirect-to-https",
                }
            }
        },
    )


# ---------------------------------------------------------------------------
# MediaPipelineStack
# ---------------------------------------------------------------------------


def test_image_process_lambda_exists(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    # CDK also adds its own singleton BucketNotificationsHandler Lambda to this
    # stack (the S3 event subscription is imported here to avoid a cyclic
    # dependency with MediaPersistentStack — see media_pipeline_stack.py), so we
    # match on the business function specifically rather than counting all
    # AWS::Lambda::Function resources.
    _, pipeline = media_templates
    pipeline.has_resource_properties(
        "AWS::Lambda::Function",
        {"FunctionName": "OpenNewsletter-ImageProcess-dev"},
    )


def test_image_process_lambda_arm64(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    _, pipeline = media_templates
    pipeline.has_resource_properties(
        "AWS::Lambda::Function",
        {"Architectures": ["arm64"]},
    )


def test_image_process_lambda_memory(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    _, pipeline = media_templates
    pipeline.has_resource_properties(
        "AWS::Lambda::Function",
        {"MemorySize": 1024},
    )
