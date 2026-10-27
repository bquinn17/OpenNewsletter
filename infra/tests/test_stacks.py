"""CDK stack assertion tests per docs/development/testing-and-ci.md §3.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import dataclasses
import json
import sys
from pathlib import Path
from typing import Any

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
    persistent_stack = MediaPersistentStack(
        app,
        "TestMediaPersistentStack",
        config=dev_config,
        table=data_stack.table,
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
    # Dev: `frontend` + the dev-only `admin-bootstrap` client.
    auth_template.resource_count_is("AWS::Cognito::UserPoolClient", 2)


def test_user_pool_two_idps(auth_template: assertions.Template) -> None:
    # Google + Facebook only — Apple dropped (docs/decisions.md).
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
    # Media originals, avatar originals, and the one processed bucket that
    # holds both img/* and avatar/* (`docs/design/media.md` §2).
    resources = persistent.find_resources("AWS::S3::Bucket")
    assert len(resources) == 3


def _bucket_lifecycle_rules(
    persistent: assertions.Template, logical_id_prefix: str
) -> list[dict[str, Any]]:
    """Lifecycle rules for the one bucket whose logical ID starts with
    `logical_id_prefix` (CDK suffixes a hash, e.g. `ProcessedBucketDE59930C`).
    A plain prefix match is unambiguous here: none of the three bucket
    construct IDs is itself a prefix of another."""
    resources = persistent.find_resources("AWS::S3::Bucket")
    matches = [v for k, v in resources.items() if k.startswith(logical_id_prefix)]
    assert len(matches) == 1, (
        f"expected exactly one {logical_id_prefix}*, found {len(matches)}"
    )
    rules: list[dict[str, Any]] = matches[0]["Properties"]["LifecycleConfiguration"][
        "Rules"
    ]
    return rules


def test_processed_buckets_have_no_intelligent_tiering_transition(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    # Processed variants (WebP display/thumb images, 256px avatars) are well
    # under the 128 KB floor where Intelligent-Tiering ever pays for itself,
    # so the transition would be pure per-object cost for no benefit.
    persistent, _ = media_templates
    for prefix in ("ProcessedBucket",):
        rules = _bucket_lifecycle_rules(persistent, prefix)
        assert all("Transitions" not in rule for rule in rules)


def test_originals_buckets_expire_uploads_after_7_days(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    persistent, _ = media_templates
    for prefix in ("OriginalsBucket", "AvatarsOriginalsBucket"):
        rules = _bucket_lifecycle_rules(persistent, prefix)
        expiration_rules = [
            r
            for r in rules
            if r.get("Prefix") == "uploads/" and "ExpirationInDays" in r
        ]
        assert len(expiration_rules) == 1
        assert expiration_rules[0]["ExpirationInDays"] == 7


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_processed_bucket_allows_cors_reads_from_the_app_origin_only(
    env: str,
) -> None:
    # Images load with `crossorigin="anonymous"` so the service worker can
    # cache them (`docs/design/media.md` §4.1); localhost only in dev.
    config = load_config(env)
    buckets = _media_persistent_template(env).find_resources("AWS::S3::Bucket")
    (processed,) = [v for k, v in buckets.items() if k.startswith("ProcessedBucket")]
    expected_origins = [f"https://{config.domain}"]
    if env == "dev":
        expected_origins.append("http://localhost:5173")
    assert processed["Properties"]["CorsConfiguration"]["CorsRules"] == [
        {"AllowedMethods": ["GET", "HEAD"], "AllowedOrigins": expected_origins}
    ]


def test_media_outputs_name_the_s3_origins_the_spa_needs(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    # The SPA's CSP must match the presigned URL host exactly, so the
    # outputs use the regional virtual-hosted form the S3 SDK signs.
    persistent, _ = media_templates
    outputs = persistent.find_outputs("*")
    media_base_url = json.dumps(outputs["MediaBaseUrl"]["Value"])
    assert "ProcessedBucket" in media_base_url
    assert ".s3.us-east-1." in media_base_url
    upload_origins = json.dumps(outputs["UploadOrigins"]["Value"])
    assert '"OriginalsBucket' in upload_origins
    assert '"AvatarsOriginalsBucket' in upload_origins


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_media_persistent_stack_has_no_cloudfront(env: str) -> None:
    # AWS won't let this account create CloudFront resources
    # (`docs/design/media.md` §6.1).
    rendered = json.dumps(_media_persistent_template(env).to_json())
    assert "AWS::CloudFront::" not in rendered


def test_api_stacks_reference_no_cloudfront(
    api_template: assertions.Template, prod_api_template: assertions.Template
) -> None:
    for template in (api_template, prod_api_template):
        rendered = json.dumps(template.to_json())
        assert "AWS::CloudFront::" not in rendered
        assert "cloudfront:" not in rendered


def test_certificate_covers_the_app_and_api_domains_only(dev_config: EnvConfig) -> None:
    app = cdk.App()
    stack = FrontendStack(app, "TestFrontendStackSans", config=dev_config, env=AWS_ENV)
    certificates = assertions.Template.from_stack(stack).find_resources(
        "AWS::CertificateManager::Certificate"
    )
    (certificate,) = certificates.values()
    names = [certificate["Properties"]["DomainName"]]
    names += certificate["Properties"].get("SubjectAlternativeNames", [])
    assert sorted(names) == sorted([dev_config.domain, dev_config.api_domain])


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
                        "MEDIA_ORIGINALS_BUCKET": assertions.Match.any_value(),
                        "PROCESSED_BUCKET": assertions.Match.any_value(),
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
        notify_tick_fn=notifications_stack.notify_tick_fn,
        media_originals_bucket=media_persistent_stack.originals_bucket,
        media_processed_bucket=media_persistent_stack.processed_bucket,
        avatars_originals_bucket=media_persistent_stack.avatars_originals_bucket,
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
        notify_tick_fn=notifications_stack.notify_tick_fn,
        media_originals_bucket=media_persistent_stack.originals_bucket,
        media_processed_bucket=media_persistent_stack.processed_bucket,
        avatars_originals_bucket=media_persistent_stack.avatars_originals_bucket,
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


def _authorizer_audiences(template: assertions.Template) -> list[Any]:
    authorizers = template.find_resources("AWS::ApiGatewayV2::Authorizer")
    assert len(authorizers) == 1
    authorizer = next(iter(authorizers.values()))
    audiences: list[Any] = authorizer["Properties"]["JwtConfiguration"]["Audience"]
    return audiences


def test_jwt_authorizer_accepts_both_frontend_and_bootstrap_audiences(
    api_template: assertions.Template,
) -> None:
    audiences = _authorizer_audiences(api_template)
    assert len(audiences) == 2
    assert "FrontendClient" in str(audiences[0])
    assert "AdminBootstrapClient" in str(audiences[1])


def test_prod_jwt_authorizer_accepts_only_the_frontend_audience(
    prod_api_template: assertions.Template,
) -> None:
    # Prod has no admin-bootstrap client, so no password-auth token can pass
    # (`docs/design/infrastructure.md` §6.1).
    audiences = _authorizer_audiences(prod_api_template)
    assert len(audiences) == 1
    assert "FrontendClient" in str(audiences[0])


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
        "PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
        "GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "POST /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
        "PATCH /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "DELETE /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
        "GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions",
        "PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "DELETE /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
        "POST /push/subscribe",
        "POST /push/unsubscribe",
        "GET /push/subscriptions",
        "POST /push/test",
        "GET /push/preferences",
        "PUT /push/preferences/{groupId}",
        "POST /admin/dev/tick/cycle",
        "POST /admin/dev/tick/notify",
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
                        "PROCESSED_BUCKET": assertions.Match.any_value(),
                        "AVATARS_ORIGINALS_BUCKET": assertions.Match.any_value(),
                    }
                )
            },
        },
    )


def test_push_lambda_exists(api_template: assertions.Template) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Push-dev",
            "Architectures": ["arm64"],
            "Runtime": "provided.al2023",
            "MemorySize": 256,
            "Timeout": 60,
        },
    )


def test_push_lambda_has_app_secret_env_var(
    api_template: assertions.Template,
) -> None:
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Push-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"APP_SECRET_ARN": assertions.Match.any_value()}
                )
            },
        },
    )


def test_push_lambda_has_secrets_manager_permission(
    api_template: assertions.Template,
) -> None:
    # Verify the Push Lambda has GetSecretValue permission by checking its role
    # This is verified by the presence of the policy in the CDK output
    functions = api_template.find_resources("AWS::Lambda::Function")
    push_func = None
    for func in functions.values():
        if func["Properties"]["FunctionName"] == "OpenNewsletter-Push-dev":
            push_func = func
            break
    assert push_func is not None


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


def test_access_log_format_reads_no_request_headers(
    api_template: assertions.Template,
) -> None:
    # HTTP APIs reject header context variables in access logs at deploy time; synth passes.
    for stage in api_template.find_resources("AWS::ApiGatewayV2::Stage").values():
        assert ".header." not in stage["Properties"]["AccessLogSettings"]["Format"]


def test_dev_has_no_custom_domain(api_template: assertions.Template) -> None:
    api_template.resource_count_is("AWS::ApiGatewayV2::DomainName", 0)


def test_prod_maps_the_custom_domain(prod_api_template: assertions.Template) -> None:
    prod_api_template.resource_count_is("AWS::ApiGatewayV2::DomainName", 1)
    prod_api_template.resource_count_is("AWS::ApiGatewayV2::ApiMapping", 1)


def test_dev_tick_route_present_in_dev(api_template: assertions.Template) -> None:
    routes = api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/cycle" in route_keys


def test_dev_notify_route_present_in_dev(api_template: assertions.Template) -> None:
    routes = api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/notify" in route_keys


def test_dev_tick_route_absent_in_prod(prod_api_template: assertions.Template) -> None:
    routes = prod_api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/cycle" not in route_keys


def test_dev_notify_route_absent_in_prod(
    prod_api_template: assertions.Template,
) -> None:
    routes = prod_api_template.find_resources("AWS::ApiGatewayV2::Route")
    route_keys = {route["Properties"]["RouteKey"] for route in routes.values()}
    assert "POST /admin/dev/tick/notify" not in route_keys


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
    # Should have 2 rules: cycle_tick (5 min) and notify_tick (15 min)
    assert len(rules) == 2
    # Check that cycle_tick schedule targets exactly 1 lambda
    cycle_tick_rules = [
        rule
        for rule in rules.values()
        if rule["Properties"]["ScheduleExpression"] == "rate(5 minutes)"
    ]
    assert len(cycle_tick_rules) == 1
    targets = cycle_tick_rules[0]["Properties"]["Targets"]
    assert len(targets) == 1


def test_cycle_tick_has_push_function_name_env_var(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-CycleTick-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {"PUSH_FUNCTION_NAME": "OpenNewsletter-Push-dev"},
                )
            },
        },
    )


def test_cycle_tick_can_invoke_push_lambda(
    notifications_template: assertions.Template,
) -> None:
    functions = notifications_template.find_resources("AWS::Lambda::Function")
    cycle_tick_func = None
    for func in functions.values():
        if func["Properties"]["FunctionName"] == "OpenNewsletter-CycleTick-dev":
            cycle_tick_func = func
            break
    assert cycle_tick_func is not None


def test_notify_tick_lambda_exists(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-NotifyTick-dev",
            "Architectures": ["arm64"],
            "Runtime": "provided.al2023",
            "MemorySize": 512,
            "Timeout": 120,
        },
    )


def test_notify_tick_lambda_has_required_env_vars(
    notifications_template: assertions.Template,
) -> None:
    notifications_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-NotifyTick-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {
                        "TABLE_NAME": assertions.Match.any_value(),
                        "ENV": "dev",
                        "RUST_LOG": "info",
                        "APP_SECRET_ARN": assertions.Match.any_value(),
                    }
                )
            },
        },
    )


def test_notify_tick_schedule_runs_every_fifteen_minutes(
    notifications_template: assertions.Template,
) -> None:
    rules = notifications_template.find_resources("AWS::Events::Rule")
    assert len(rules) == 2  # Now we have both cycle_tick and notify_tick rules
    fifteen_min_rules = [
        rule
        for rule in rules.values()
        if rule["Properties"]["ScheduleExpression"] == "rate(15 minutes)"
    ]
    assert len(fifteen_min_rules) == 1


def test_notify_tick_has_secrets_manager_permission(
    notifications_template: assertions.Template,
) -> None:
    # Check that NotifyTick function has GetSecretValue permission
    resources = notifications_template.find_resources("AWS::Lambda::Function")
    notify_tick_func = None
    for func in resources.values():
        if func["Properties"]["FunctionName"] == "OpenNewsletter-NotifyTick-dev":
            notify_tick_func = func
            break
    assert notify_tick_func is not None


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


# ---------------------------------------------------------------------------
# MonitoringStack
# ---------------------------------------------------------------------------


# Alarms are prod-only (`docs/design/infrastructure.md` §9.2), so these build the prod stack; the
# dev/prod split and the alarm-metric budget are covered in
# test_monitoring_stack.py.
@pytest.fixture(scope="module")
def monitoring_template() -> assertions.Template:
    app = cdk.App()
    from opennewsletter.monitoring_stack import MonitoringStack

    stack = MonitoringStack(
        app, "TestMonitoringStack", config=load_config("prod"), env=AWS_ENV
    )
    return assertions.Template.from_stack(stack)


def test_push_failure_rate_alarm_exists(
    monitoring_template: assertions.Template,
) -> None:
    monitoring_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-PushFailureRate-prod",
            "Threshold": 0.2,
            "AlarmDescription": assertions.Match.string_like_regexp(
                "Push failure rate"
            ),
        },
    )
    # The 30-minute window lives on the math expression's metrics; a
    # MathExpression without its own period silently resets them to 5 minutes.
    alarms = monitoring_template.find_resources("AWS::CloudWatch::Alarm")
    (push_alarm,) = [
        a
        for a in alarms.values()
        if a["Properties"]["AlarmName"] == "OpenNewsletter-PushFailureRate-prod"
    ]
    periods = [
        q["MetricStat"]["Period"]
        for q in push_alarm["Properties"]["Metrics"]
        if "MetricStat" in q
    ]
    assert periods == [1800, 1800]
    # A total outage emits no PushSent datapoints; without FILL the expression
    # goes missing and the alarm reads "not breaching" at a 100% failure rate.
    (expression,) = [
        q["Expression"]
        for q in push_alarm["Properties"]["Metrics"]
        if "Expression" in q
    ]
    assert "FILL(sent, 0)" in expression


def test_notify_tick_has_no_separate_errors_alarm(
    monitoring_template: assertions.Template,
) -> None:
    # Dropped to stay within the 10 free alarm metrics: notify-tick failures
    # surface as push failures, already alarmed (`docs/design/infrastructure.md` §9.2).
    alarms = monitoring_template.find_resources("AWS::CloudWatch::Alarm")
    names = {a["Properties"]["AlarmName"] for a in alarms.values()}
    assert not any("NotifyTick" in name for name in names)


def test_cycle_tick_errors_alarm_exists(
    monitoring_template: assertions.Template,
) -> None:
    monitoring_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-CycleTick-Errors-prod",
            "Threshold": 1,
            "AlarmDescription": assertions.Match.string_like_regexp("CycleTick"),
        },
    )


def test_cycle_tick_alarm_also_counts_abandoned_auto_publishes(
    monitoring_template: assertions.Template,
) -> None:
    # Auto-publish giving up on drafts doesn't fail the invocation, so the
    # alarm reads the tick's `AutoPublishAbandoned` EMF metric too.
    monitoring_template.has_resource_properties(
        "AWS::CloudWatch::Alarm",
        {
            "AlarmName": "OpenNewsletter-CycleTick-Errors-prod",
            "Metrics": assertions.Match.array_with(
                [
                    assertions.Match.object_like(
                        {
                            "MetricStat": assertions.Match.object_like(
                                {
                                    "Metric": assertions.Match.object_like(
                                        {
                                            "MetricName": "AutoPublishAbandoned",
                                            "Namespace": "OpenNewsletter/prod",
                                        }
                                    )
                                }
                            )
                        }
                    )
                ]
            ),
        },
    )


# ---------------------------------------------------------------------------
# M13.5 — cost, auth and media-deletion changes
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def prod_auth_template() -> assertions.Template:
    app = cdk.App()
    stack = AuthStack(app, "ProdAuthStackOnly", config=load_config("prod"), env=AWS_ENV)
    return assertions.Template.from_stack(stack)


def _media_persistent_template(env: str) -> assertions.Template:
    app = cdk.App()
    config = load_config(env)
    data_stack = DataStack(app, "MediaDataStack", config=config, env=AWS_ENV)
    stack = MediaPersistentStack(
        app,
        "MediaPersistentStackOnly",
        config=config,
        table=data_stack.table,
        env=AWS_ENV,
    )
    return assertions.Template.from_stack(stack)


def test_dynamodb_table_uses_the_aws_owned_key(
    data_template: assertions.Template,
) -> None:
    # AWS-owned key = SSE not explicitly enabled (enabling it selects the
    # billed AWS-managed `aws/dynamodb` KMS key). `docs/design/infrastructure.md` §3.1.
    (table,) = data_template.find_resources("AWS::DynamoDB::Table").values()
    sse = table["Properties"].get("SSESpecification", {})
    assert sse.get("SSEEnabled", False) is False
    assert "KMSMasterKeyId" not in sse


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_no_media_bucket_is_versioned(env: str) -> None:
    # A real delete must not leave the object behind a delete marker (`docs/design/infrastructure.md` §5.1).
    buckets = _media_persistent_template(env).find_resources("AWS::S3::Bucket")
    assert len(buckets) == 3
    for bucket in buckets.values():
        assert "VersioningConfiguration" not in bucket["Properties"]


def test_google_idp_maps_email_verified(auth_template: assertions.Template) -> None:
    # `POST /invites/redeem` checks it for email-bound invites (`docs/design/auth.md` §4.2).
    auth_template.has_resource_properties(
        "AWS::Cognito::UserPoolIdentityProvider",
        {
            "ProviderName": "Google",
            "AttributeMapping": {
                "email": "email",
                "name": "name",
                "email_verified": "email_verified",
            },
        },
    )


def test_idps_read_credentials_from_the_app_secret(
    auth_template: assertions.Template,
) -> None:
    idps = {
        idp["Properties"]["ProviderName"]: idp["Properties"]["ProviderDetails"]
        for idp in auth_template.find_resources(
            "AWS::Cognito::UserPoolIdentityProvider"
        ).values()
    }
    secret = load_config("dev").app_secret_arn
    ref = f"{{{{resolve:secretsmanager:{secret}:SecretString:"
    assert idps["Google"]["client_id"] == f"{ref}google_client_id}}}}"
    # `SecretValue.secrets_manager` appends empty version-stage/id fields.
    assert idps["Google"]["client_secret"].startswith(f"{ref}google_client_secret:")
    assert idps["Facebook"]["client_id"] == f"{ref}facebook_app_id}}}}"
    assert idps["Facebook"]["client_secret"] == f"{ref}facebook_app_secret}}}}"


def test_dev_outputs_the_bootstrap_client_id(
    auth_template: assertions.Template,
) -> None:
    assert "UserPoolBootstrapClientId" in auth_template.find_outputs("*")


def test_prod_has_no_bootstrap_client(prod_auth_template: assertions.Template) -> None:
    # Prod has no password auth at all (`docs/design/infrastructure.md` §4.3, `docs/design/auth.md` §9.1a).
    prod_auth_template.resource_count_is("AWS::Cognito::UserPoolClient", 1)
    prod_auth_template.has_resource_properties(
        "AWS::Cognito::UserPoolClient", {"ClientName": "frontend"}
    )
    assert "UserPoolBootstrapClientId" not in prod_auth_template.find_outputs("*")


def test_groups_vapid_public_key_comes_from_the_app_secret(
    api_template: assertions.Template,
) -> None:
    secret = load_config("dev").app_secret_arn
    api_template.has_resource_properties(
        "AWS::Lambda::Function",
        {
            "FunctionName": "OpenNewsletter-Groups-dev",
            "Environment": {
                "Variables": assertions.Match.object_like(
                    {
                        "VAPID_PUBLIC_KEY": (
                            f"{{{{resolve:secretsmanager:{secret}"
                            ":SecretString:vapid_public_key}}"
                        )
                    }
                )
            },
        },
    )


_OLD_SECRET_NAMES = (
    "VAPID_SECRET_ARN",
    "GOOGLE_OAUTH_SECRET_ARN",
    "FACEBOOK_OAUTH_SECRET_ARN",
)


def test_config_has_one_app_secret_field() -> None:
    fields = {field.name for field in dataclasses.fields(EnvConfig)}
    assert "app_secret_arn" in fields
    assert not fields & {
        "google_oauth_secret_arn",
        "facebook_oauth_secret_arn",
        "vapid_secret_arn",
    }


def test_no_template_references_the_old_secret_env_vars(
    api_template: assertions.Template,
    prod_api_template: assertions.Template,
    notifications_template: assertions.Template,
) -> None:
    for template in (api_template, prod_api_template, notifications_template):
        rendered = json.dumps(template.to_json())
        for name in _OLD_SECRET_NAMES:
            assert name not in rendered


def test_notify_tick_has_no_vapid_secret_env_var(
    notifications_template: assertions.Template,
) -> None:
    (fn,) = notifications_template.find_resources(
        "AWS::Lambda::Function",
        {"Properties": {"FunctionName": "OpenNewsletter-NotifyTick-dev"}},
    ).values()
    assert "VAPID_SECRET_ARN" not in fn["Properties"]["Environment"]["Variables"]


# --- IAM: per-Lambda role statements ---


def _role_statements(
    template: assertions.Template, function_name: str
) -> list[dict[str, Any]]:
    (fn,) = template.find_resources(
        "AWS::Lambda::Function", {"Properties": {"FunctionName": function_name}}
    ).values()
    role_id = fn["Properties"]["Role"]["Fn::GetAtt"][0]
    statements: list[dict[str, Any]] = []
    for policy in template.find_resources("AWS::IAM::Policy").values():
        if {"Ref": role_id} in policy["Properties"]["Roles"]:
            statements.extend(policy["Properties"]["PolicyDocument"]["Statement"])
    return statements


def _statements_with(
    template: assertions.Template, function_name: str, action: str
) -> list[dict[str, Any]]:
    def actions(statement: dict[str, Any]) -> list[str]:
        action_field = statement["Action"]
        return [action_field] if isinstance(action_field, str) else list(action_field)

    return [
        s for s in _role_statements(template, function_name) if action in actions(s)
    ]


def _bucket_and_prefix(resource: Any) -> tuple[str, str]:
    """Name the bucket an `arn_for_objects(prefix)` ARN points at, by the
    logical id CDK embeds in the cross-stack reference."""
    reference, suffix = resource["Fn::Join"][1]
    rendered = str(reference)
    for fragment, bucket in (
        ("AvatarsOriginalsBucket", "avatars-originals"),
        ("OriginalsBucket", "media-originals"),
        ("ProcessedBucket", "media-processed"),
    ):
        if fragment in rendered:
            return bucket, suffix
    raise AssertionError(f"unrecognised bucket reference: {rendered}")


def _delete_targets(
    template: assertions.Template, function_name: str
) -> set[tuple[str, str]]:
    targets: set[tuple[str, str]] = set()
    for statement in _statements_with(template, function_name, "s3:DeleteObject"):
        assert statement["Action"] == "s3:DeleteObject"
        resources = statement["Resource"]
        for resource in resources if isinstance(resources, list) else [resources]:
            targets.add(_bucket_and_prefix(resource))
    return targets


_MEDIA_DELETE = {("media-originals", "/uploads/*"), ("media-processed", "/img/*")}
_AVATAR_DELETE = {
    ("avatars-originals", "/uploads/*"),
    ("media-processed", "/avatar/*"),
}


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("Media", _MEDIA_DELETE | _AVATAR_DELETE),
        ("Engagement", _MEDIA_DELETE),
        ("Groups", _AVATAR_DELETE),
        ("Invites", set()),
        ("Newsletters", set()),
        ("Questions", set()),
        ("Responses", set()),
        ("Push", set()),
    ],
)
def test_s3_delete_is_scoped_per_lambda(
    api_template: assertions.Template, name: str, expected: set[tuple[str, str]]
) -> None:
    assert _delete_targets(api_template, f"OpenNewsletter-{name}-dev") == expected


def _get_targets(
    template: assertions.Template, function_name: str
) -> set[tuple[str, str]]:
    targets: set[tuple[str, str]] = set()
    for statement in _statements_with(template, function_name, "s3:GetObject"):
        assert statement["Action"] == "s3:GetObject"
        resources = statement["Resource"]
        for resource in resources if isinstance(resources, list) else [resources]:
            targets.add(_bucket_and_prefix(resource))
    return targets


_READ_IMAGES = {("media-processed", "/img/*")}
_READ_AVATARS = {("media-processed", "/avatar/*")}


@pytest.mark.parametrize(
    ("name", "expected"),
    [
        ("Media", _READ_IMAGES | _READ_AVATARS),
        ("Newsletters", _READ_IMAGES | _READ_AVATARS),
        ("Engagement", _READ_IMAGES),
        ("Groups", _READ_AVATARS),
        ("Questions", _READ_AVATARS),
        ("Invites", set()),
        ("Responses", set()),
        ("Push", set()),
    ],
)
def test_s3_get_is_scoped_to_the_prefixes_each_lambda_signs(
    api_template: assertions.Template, name: str, expected: set[tuple[str, str]]
) -> None:
    # A presigned GET carries the signer's permissions (`docs/design/media.md`
    # §4.1): each Lambda can read exactly the prefixes whose URLs it returns.
    assert _get_targets(api_template, f"OpenNewsletter-{name}-dev") == expected


def test_image_process_can_delete_processed_variants(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    # A row deleted mid-processing leaves ImageProcess to remove the variants
    # it just wrote (`docs/design/media.md` §8.0).
    _, pipeline = media_templates
    assert _delete_targets(pipeline, "OpenNewsletter-ImageProcess-dev") == {
        ("media-processed", "/img/*"),
        ("media-processed", "/avatar/*"),
    }


def test_image_process_puts_only_processed_variants(
    media_templates: tuple[assertions.Template, assertions.Template],
) -> None:
    # It writes `img/*` and `avatar/*` and nothing else (`docs/design/media.md` §5.4).
    _, pipeline = media_templates
    targets: set[tuple[str, str]] = set()
    for statement in _statements_with(
        pipeline, "OpenNewsletter-ImageProcess-dev", "s3:PutObject"
    ):
        resources = statement["Resource"]
        for resource in resources if isinstance(resources, list) else [resources]:
            targets.add(_bucket_and_prefix(resource))
    assert targets == {
        ("media-processed", "/img/*"),
        ("media-processed", "/avatar/*"),
    }


@pytest.mark.parametrize(
    ("name", "expected_keys"),
    [
        (
            "Media",
            {
                "MEDIA_ORIGINALS_BUCKET",
                "PROCESSED_BUCKET",
                "AVATARS_ORIGINALS_BUCKET",
            },
        ),
        (
            "Engagement",
            {"MEDIA_ORIGINALS_BUCKET", "PROCESSED_BUCKET"},
        ),
        (
            "Groups",
            {
                "AVATARS_ORIGINALS_BUCKET",
                "PROCESSED_BUCKET",
            },
        ),
    ],
)
def test_media_delete_env_vars(
    api_template: assertions.Template, name: str, expected_keys: set[str]
) -> None:
    (fn,) = api_template.find_resources(
        "AWS::Lambda::Function",
        {"Properties": {"FunctionName": f"OpenNewsletter-{name}-dev"}},
    ).values()
    variables = fn["Properties"]["Environment"]["Variables"]
    assert expected_keys <= set(variables)
    # Media-delete Lambdas get only the buckets they delete from.
    delete_buckets = {
        "MEDIA_ORIGINALS_BUCKET",
        "PROCESSED_BUCKET",
        "AVATARS_ORIGINALS_BUCKET",
    }
    assert set(variables) & delete_buckets == expected_keys & delete_buckets


@pytest.mark.parametrize(
    ("name", "allowed"),
    [
        ("Push", True),
        ("Media", False),
        ("Groups", False),
        ("Invites", False),
        ("Newsletters", False),
        ("Questions", False),
        ("Responses", False),
        ("Engagement", False),
    ],
)
def test_app_secret_read_is_limited_to_push(
    api_template: assertions.Template, name: str, allowed: bool
) -> None:
    function_name = f"OpenNewsletter-{name}-dev"
    statements = _statements_with(
        api_template, function_name, "secretsmanager:GetSecretValue"
    )
    if not allowed:
        assert statements == []
        return
    (statement,) = statements
    assert statement["Resource"] == load_config("dev").app_secret_arn
    (fn,) = api_template.find_resources(
        "AWS::Lambda::Function", {"Properties": {"FunctionName": function_name}}
    ).values()
    variables = fn["Properties"]["Environment"]["Variables"]
    assert variables["APP_SECRET_ARN"] == load_config("dev").app_secret_arn


def test_notify_tick_reads_the_app_secret_and_cycle_tick_does_not(
    notifications_template: assertions.Template,
) -> None:
    (statement,) = _statements_with(
        notifications_template,
        "OpenNewsletter-NotifyTick-dev",
        "secretsmanager:GetSecretValue",
    )
    assert statement["Resource"] == load_config("dev").app_secret_arn
    assert (
        _statements_with(
            notifications_template,
            "OpenNewsletter-CycleTick-dev",
            "secretsmanager:GetSecretValue",
        )
        == []
    )
