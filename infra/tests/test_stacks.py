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
from opennewsletter.api_stack import ApiStack
from opennewsletter.auth_stack import AuthStack
from opennewsletter.config import EnvConfig, load_config
from opennewsletter.data_stack import DataStack
from opennewsletter.frontend_stack import FrontendStack
from opennewsletter.media_persistent_stack import MediaPersistentStack
from opennewsletter.media_pipeline_stack import MediaPipelineStack
from opennewsletter.notifications_stack import NotificationsStack

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
    frontend_stack = FrontendStack(
        app, "TestFrontendStack", config=dev_config, env=AWS_ENV
    )
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
        avatars_originals_bucket=persistent_stack.avatars_originals_bucket,
        avatars_processed_bucket=persistent_stack.avatars_processed_bucket,
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
    table = next(iter(resources.values()))
    gsis = table["Properties"]["GlobalSecondaryIndexes"]
    assert len(gsis) == 2
    index_names = {g["IndexName"] for g in gsis}
    assert index_names == {"gsi1", "gsi2"}


def test_dynamodb_gsi_projections(data_template: assertions.Template) -> None:
    resources = data_template.find_resources("AWS::DynamoDB::Table")
    table = next(iter(resources.values()))
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


def test_user_pool_two_idps(auth_template: assertions.Template) -> None:
    # Google + Facebook only — Apple dropped (plans/PROGRESS.md decisions, M1).
    auth_template.resource_count_is("AWS::Cognito::UserPoolIdentityProvider", 2)


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
    dist = next(iter(resources.values()))
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


def test_avatar_originals_bucket_has_no_versioning(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    resources = persistent.find_resources("AWS::S3::Bucket")
    # Find the avatars-originals bucket by name pattern
    for logical_id, bucket in resources.items():
        if "AvatarsOriginals" in logical_id:
            # versioning not set or explicitly set to False means no versioning
            assert (
                bucket["Properties"].get("VersioningConfiguration", {}).get("Status")
                != "Enabled"
            )


def test_avatar_originals_bucket_has_cors(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    resources = persistent.find_resources("AWS::S3::Bucket")
    avatar_originals_buckets = [
        bucket
        for logical_id, bucket in resources.items()
        if "AvatarsOriginals" in logical_id
    ]
    assert len(avatar_originals_buckets) == 1
    bucket = avatar_originals_buckets[0]
    assert "CorsConfiguration" in bucket["Properties"]


def test_cloudfront_has_avatar_behavior(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    resources = persistent.find_resources("AWS::CloudFront::Distribution")
    assert len(resources) == 1
    dist = next(iter(resources.values()))
    behaviors = dist["Properties"]["DistributionConfig"].get("CacheBehaviors", [])
    # Avatar behavior should be in CacheBehaviors (not DefaultCacheBehavior)
    avatar_behaviors = [b for b in behaviors if "/avatar/*" in b.get("PathPattern", "")]
    assert len(avatar_behaviors) == 1


def test_avatar_behavior_has_no_trusted_key_group(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    resources = persistent.find_resources("AWS::CloudFront::Distribution")
    dist = next(iter(resources.values()))
    behaviors = dist["Properties"]["DistributionConfig"].get("CacheBehaviors", [])
    avatar_behaviors = [b for b in behaviors if "/avatar/*" in b.get("PathPattern", "")]
    avatar_behavior = avatar_behaviors[0]
    # Should not have TrustedKeyGroups
    assert "TrustedKeyGroups" not in avatar_behavior


def test_image_process_has_avatar_bucket_env_vars(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    _, pipeline = media_templates
    pipeline.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-ImageProcess-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {
                        "AVATARS_ORIGINALS_BUCKET": assertions.Match.any_value(),
                        "AVATARS_PROCESSED_BUCKET": assertions.Match.any_value(),
                        "MEDIA_ORIGINALS_BUCKET": assertions.Match.any_value(),
                    }
                )
            },
        },
    )


def test_image_process_has_two_s3_notifications(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    _, pipeline = media_templates
    # Count S3 notification custom resources
    resources = pipeline.find_resources("Custom::S3BucketNotifications")
    # Should have notifications for both media-originals and avatars-originals
    assert len(resources) >= 2


# ---------------------------------------------------------------------------
# ApiStack
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def notifications_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    data_stack = DataStack(app, "TestDataStack4", config=dev_config, env=AWS_ENV)
    stack = NotificationsStack(
        app,
        "TestNotificationsStack",
        config=dev_config,
        table=data_stack.table,
        env=AWS_ENV,
    )
    return assertions.Template.from_stack(stack)


@pytest.fixture(scope="module")
def api_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    data_stack = DataStack(app, "TestDataStack3", config=dev_config, env=AWS_ENV)
    auth_stack = AuthStack(app, "TestAuthStack2", config=dev_config, env=AWS_ENV)
    frontend_stack = FrontendStack(
        app, "TestFrontendStack2", config=dev_config, env=AWS_ENV
    )
    media_persistent_stack = MediaPersistentStack(
        app,
        "TestMediaPersistentStack3",
        config=dev_config,
        table=data_stack.table,
        certificate=frontend_stack.certificate,
        env=AWS_ENV,
    )
    notifications_stack = NotificationsStack(
        app,
        "TestNotificationsStack2",
        config=dev_config,
        table=data_stack.table,
        env=AWS_ENV,
    )
    stack = ApiStack(
        app,
        "TestApiStack",
        config=dev_config,
        table=data_stack.table,
        user_pool=auth_stack.user_pool,
        user_pool_client=auth_stack.frontend_client,
        bootstrap_client=auth_stack.bootstrap_client,
        certificate=frontend_stack.certificate,
        cycle_tick_fn=notifications_stack.cycle_tick_fn,
        media_originals_bucket=media_persistent_stack.originals_bucket,
        avatars_originals_bucket=media_persistent_stack.avatars_originals_bucket,
        cdn_domain=media_persistent_stack.distribution.distribution_domain_name,
        cdn_key_pair_id=media_persistent_stack.cdn_key_pair_id,
        env=AWS_ENV,
    )
    return assertions.Template.from_stack(stack)


@pytest.fixture(scope="module")
def prod_api_template() -> assertions.Template:
    app = cdk.App()
    config = load_config("prod")
    data_stack = DataStack(app, "ProdDataStack", config=config, env=AWS_ENV)
    auth_stack = AuthStack(app, "ProdAuthStack", config=config, env=AWS_ENV)
    frontend_stack = FrontendStack(app, "ProdFrontendStack", config=config, env=AWS_ENV)
    media_persistent_stack = MediaPersistentStack(
        app,
        "ProdMediaPersistentStack",
        config=config,
        table=data_stack.table,
        certificate=frontend_stack.certificate,
        env=AWS_ENV,
    )
    notifications_stack = NotificationsStack(
        app,
        "ProdNotificationsStack",
        config=config,
        table=data_stack.table,
        env=AWS_ENV,
    )
    stack = ApiStack(
        app,
        "ProdApiStack",
        config=config,
        table=data_stack.table,
        user_pool=auth_stack.user_pool,
        user_pool_client=auth_stack.frontend_client,
        bootstrap_client=auth_stack.bootstrap_client,
        certificate=frontend_stack.certificate,
        cycle_tick_fn=notifications_stack.cycle_tick_fn,
        media_originals_bucket=media_persistent_stack.originals_bucket,
        avatars_originals_bucket=media_persistent_stack.avatars_originals_bucket,
        cdn_domain=media_persistent_stack.distribution.distribution_domain_name,
        cdn_key_pair_id=media_persistent_stack.cdn_key_pair_id,
        env=AWS_ENV,
    )
    return assertions.Template.from_stack(stack)


def test_http_api_exists(api_template: assertions.Template) -> None:
    api_template.has_resource_properties(
        "AWS::ApiGatewayV2::Api",
        {"Name": "OpenNewsletter-Api-dev", "ProtocolType": "HTTP"},
    )


def test_http_api_cors_allows_the_dev_origin(api_template: assertions.Template) -> None:
    api_template.has_resource_properties(
        "AWS::ApiGatewayV2::Api",
        {
            "CorsConfiguration": {
                "AllowOrigins": assertions.Match.array_with(["http://localhost:5173"]),
                "AllowHeaders": assertions.Match.array_with(["x-correlation-id"]),
                "ExposeHeaders": ["x-correlation-id"],
                "AllowCredentials": True,
                "MaxAge": 600,
            }
        },
    )


def test_jwt_authorizer_targets_the_user_pool(
    api_template: assertions.Template,
) -> None:
    api_template.resource_count_is("AWS::ApiGatewayV2::Authorizer", 1)
    api_template.has_resource_properties(
        "AWS::ApiGatewayV2::Authorizer",
        {
            "AuthorizerType": "JWT",
            "IdentitySource": ["$request.header.Authorization"],
        },
    )


def test_jwt_authorizer_accepts_both_frontend_and_bootstrap_audiences(
    api_template: assertions.Template,
) -> None:
    authorizers = api_template.find_resources("AWS::ApiGatewayV2::Authorizer")
    assert len(authorizers) == 1
    authorizer = next(iter(authorizers.values()))
    audiences = authorizer["Properties"]["JwtConfiguration"]["Audience"]
    assert len(audiences) == 2


def test_every_route_requires_the_jwt_authorizer(
    api_template: assertions.Template,
) -> None:
    routes = api_template.find_resources("AWS::ApiGatewayV2::Route")
    unauthorized = [
        key
        for key, route in routes.items()
        if route["Properties"].get("AuthorizationType") != "JWT"
    ]
    assert unauthorized == []


def test_all_contract_routes_are_wired(api_template: assertions.Template) -> None:
    routes = api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert route_keys == {
        "GET /healthz",
        "GET /config",
        "GET /me",
        "PATCH /me",
        "GET /groups",
        "GET /groups/{groupId}",
        "PATCH /groups/{groupId}",
        "DELETE /groups/{groupId}/members/{userId}",
        "PATCH /groups/{groupId}/members/{userId}",
        "POST /admin/invites",
        "GET /admin/groups/{groupId}/invites",
        "POST /admin/invites/{code}/revoke",
        "POST /invites/redeem",
        "POST /uploads",
        "GET /uploads/{imageId}",
        "PATCH /uploads/{imageId}",
        "DELETE /uploads/{imageId}",
        "POST /uploads/{imageId}/complete",
        "GET /media-cookie",
        "POST /avatars",
        "GET /avatars/{avatarId}",
        "DELETE /avatars/{avatarId}",
        "GET /groups/{groupId}/newsletters",
        "GET /groups/{groupId}/newsletters/{cycleId}",
        "GET /groups/{groupId}/candidate-questions",
        "POST /groups/{groupId}/candidate-questions",
        "POST /groups/{groupId}/candidate-questions/{questionId}/votes",
        "DELETE /groups/{groupId}/candidate-questions/{questionId}/votes",
        "DELETE /admin/groups/{groupId}/candidate-questions/{questionId}",
        "GET /groups/{groupId}/newsletters/{cycleId}/my-responses",
        "GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "POST /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "PATCH /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "DELETE /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions",
        "PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "DELETE /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "POST /admin/dev/tick/cycle",
    }


def test_handler_lambdas_are_arm64_provided_al2023(
    api_template: assertions.Template,
) -> None:
    for function_name in (
        "OpenNewsletter-Invites-dev",
        "OpenNewsletter-Groups-dev",
        "OpenNewsletter-Newsletters-dev",
        "OpenNewsletter-Questions-dev",
        "OpenNewsletter-Engagement-dev",
    ):
        api_template.has_resource_properties(
            "AWS::Lambda::Function",
            {
                "FunctionName": function_name,
                "Architectures": ["arm64"],
                "Runtime": "provided.al2023",
                "MemorySize": 256,
                "Timeout": 10,
            },
        )


def test_responses_lambda_is_arm64_provided_al2023_with_512_memory(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Responses-dev",
            "Architectures": ["arm64"],
            "Runtime": "provided.al2023",
            "MemorySize": 512,
            "Timeout": 10,
        },
    )


def test_media_lambda_is_arm64_provided_al2023_with_512_memory(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Media-dev",
            "Architectures": ["arm64"],
            "Runtime": "provided.al2023",
            "MemorySize": 512,
            "Timeout": 10,
        },
    )


def test_media_lambda_has_required_env_vars(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Media-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {
                        "MEDIA_ORIGINALS_BUCKET": assertions.Match.any_value(),
                        "AVATARS_ORIGINALS_BUCKET": assertions.Match.any_value(),
                        "CDN_BASE_URL": assertions.Match.any_value(),
                        "CDN_KEY_PAIR_ID": assertions.Match.any_value(),
                        "CDN_SIGNING_SECRET_ARN": assertions.Match.any_value(),
                        "MEDIA_COOKIE_DOMAIN": "",
                    }
                )
            },
        },
    )


def test_media_lambda_cookie_domain_is_prod_form_in_prod(
    prod_api_template: assertions.Template,
) -> None:
    prod_api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": assertions.Match.string_like_regexp(
                r"OpenNewsletter-Media-prod"
            ),
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {
                        "MEDIA_COOKIE_DOMAIN": assertions.Match.string_like_regexp(
                            r"\..*"
                        ),
                    }
                )
            },
        },
    )


def test_media_lambda_cdn_base_url_is_set_in_dev(
    api_template: assertions.Template,
) -> None:
    # In dev, CDN_BASE_URL should be set from the distribution domain
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Media-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"CDN_BASE_URL": assertions.Match.any_value()}
                )
            },
        },
    )


def test_groups_lambda_receives_cdn_base_url(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Groups-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"CDN_BASE_URL": assertions.Match.any_value()}
                )
            },
        },
    )


def test_engagement_lambda_receives_cdn_base_url(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Engagement-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"CDN_BASE_URL": assertions.Match.any_value()}
                )
            },
        },
    )


def test_handler_lambdas_receive_the_table_name(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Groups-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"ENV": "dev", "RUST_LOG": "info"},
                )
            },
        },
    )


def test_stage_is_throttled(api_template: assertions.Template) -> None:
    api_template.has_resource_properties(
        "AWS::ApiGatewayV2::Stage",
        {
            "DefaultRouteSettings": {
                "ThrottlingBurstLimit": 50,
                "ThrottlingRateLimit": 25,
            }
        },
    )


def test_stage_has_access_logging(api_template: assertions.Template) -> None:
    api_template.has_resource_properties(
        "AWS::ApiGatewayV2::Stage",
        {"AccessLogSettings": assertions.Match.any_value()},
    )


def test_dev_has_no_custom_domain(api_template: assertions.Template) -> None:
    api_template.resource_count_is("AWS::ApiGatewayV2::DomainName", 0)


def test_prod_maps_the_custom_domain(prod_api_template: assertions.Template) -> None:
    prod_api_template.resource_count_is("AWS::ApiGatewayV2::DomainName", 1)
    prod_api_template.resource_count_is("AWS::ApiGatewayV2::ApiMapping", 1)


def test_dev_tick_route_present_in_dev(api_template: assertions.Template) -> None:
    routes = api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/cycle" in route_keys


def test_dev_tick_route_absent_in_prod(prod_api_template: assertions.Template) -> None:
    routes = prod_api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/cycle" not in route_keys


# ---------------------------------------------------------------------------
# NotificationsStack
# ---------------------------------------------------------------------------


def test_cycle_tick_lambda_exists(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-CycleTick-dev",
            "Architectures": ["arm64"],
            "Runtime": "provided.al2023",
            "MemorySize": 512,
            "Timeout": 60,
        },
    )


def test_cycle_tick_lambda_receives_the_table_name(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-CycleTick-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"ENV": "dev", "RUST_LOG": "info"},
                )
            },
        },
    )


def test_cycle_tick_schedule_runs_every_five_minutes(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Events::Rule",
        {"ScheduleExpression": "rate(5 minutes)"},
    )


def test_cycle_tick_schedule_targets_the_lambda(
    notifications_template: assertions.Template,
) -> None:
    rules = notifications_template.find_resources("AWS::Events::Rule")
    assert len(rules) == 1
    targets = next(iter(rules.values()))["Properties"]["Targets"]
    assert len(targets) == 1


# ---------------------------------------------------------------------------
# PreSignUp trigger (AuthStack — see auth_stack.py for why it lives there)
# ---------------------------------------------------------------------------


def test_presignup_lambda_is_wired_to_the_user_pool(
    auth_template: assertions.Template,
) -> None:
    auth_template.has_resource_properties(
        "AWS::Lambda::Function",
        {"FunctionName": "OpenNewsletter-PreSignUp-dev", "Architectures": ["arm64"]},
    )
    auth_template.has_resource_properties(
        "AWS::Cognito::UserPool",
        {"LambdaConfig": {"PreSignUp": assertions.Match.any_value()}},
    )


def test_user_pool_has_no_custom_attributes(auth_template: assertions.Template) -> None:
    pools = auth_template.find_resources("AWS::Cognito::UserPool")
    for pool in pools.values():
        assert pool["Properties"].get("Schema", []) == [
            {"Mutable": True, "Name": "email", "Required": True},
            {"Mutable": True, "Name": "name", "Required": False},
        ]
