"""ApiStack — HTTP API, request-handler Lambdas, and their routes.

Per `docs/design/infrastructure.md` §6. The route table is driven from
`docs/design/api.md` §13, which is authoritative when the two disagree.

M4 wires `lambda-invites` and `lambda-groups`. M5 adds `lambda-newsletters` and
`lambda-questions`, M6 `lambda-responses`, M8 `lambda-media`, M10
`lambda-engagement`, and M11 `lambda-push`.
The remaining handler Lambdas join this stack as their milestones land; `_ROUTES`
is the single place to add them.

The `PreSignUp` trigger Lambda lives in AuthStack, not here: `add_trigger` attaches
the wiring to the user pool's own stack, so building it here would make AuthStack
depend on ApiStack while ApiStack already depends on AuthStack for the authorizer.

The dev-only `POST /admin/dev/tick/cycle` and `POST /admin/dev/tick/notify` routes
integrate `NotificationsStack`'s cycle-tick and notify-tick Lambdas, passed in via
constructor parameters — see `notifications_stack.py` for why that makes this stack
depend on `NotificationsStack` rather than the other way around.
"""

from __future__ import annotations

import json
from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_apigatewayv2 as apigw
from aws_cdk import aws_apigatewayv2_authorizers as apigw_authorizers
from aws_cdk import aws_apigatewayv2_integrations as apigw_integrations
from aws_cdk import aws_certificatemanager as acm
from aws_cdk import aws_cognito as cognito
from aws_cdk import aws_dynamodb as dynamodb
from aws_cdk import aws_iam as iam
from aws_cdk import aws_lambda
from aws_cdk import aws_logs as logs
from aws_cdk import aws_s3 as s3
from aws_cdk import aws_ssm as ssm
from constructs import Construct

from .config import APP_SECRET_VAPID_PUBLIC_KEY, EnvConfig, secret_ref
from .lambda_assets import lambda_code
from .ssm_refs import http_api_id_parameter_name

# (handler key, method, path). Every route carries the JWT authorizer — there are
# no public routes (`docs/design/infrastructure.md` §6.5).
_ROUTES: list[tuple[str, str, str]] = [
    ("groups", "GET", "/healthz"),
    ("groups", "GET", "/config"),
    ("groups", "GET", "/me"),
    ("groups", "PATCH", "/me"),
    ("groups", "GET", "/groups"),
    ("groups", "GET", "/groups/{groupId}"),
    ("groups", "PATCH", "/groups/{groupId}"),
    ("groups", "DELETE", "/groups/{groupId}/members/{userId}"),
    ("groups", "PATCH", "/groups/{groupId}/members/{userId}"),
    ("invites", "POST", "/admin/invites"),
    ("invites", "GET", "/admin/groups/{groupId}/invites"),
    ("invites", "POST", "/admin/invites/{code}/revoke"),
    ("invites", "POST", "/invites/redeem"),
    ("media", "POST", "/uploads"),
    ("media", "GET", "/uploads/{imageId}"),
    ("media", "PATCH", "/uploads/{imageId}"),
    ("media", "DELETE", "/uploads/{imageId}"),
    ("media", "POST", "/uploads/{imageId}/complete"),
    ("media", "POST", "/avatars"),
    ("media", "GET", "/avatars/{avatarId}"),
    ("media", "DELETE", "/avatars/{avatarId}"),
    ("newsletters", "GET", "/groups/{groupId}/newsletters"),
    ("newsletters", "GET", "/groups/{groupId}/newsletters/{cycleId}"),
    ("questions", "GET", "/groups/{groupId}/candidate-questions"),
    ("questions", "POST", "/groups/{groupId}/candidate-questions"),
    ("questions", "POST", "/groups/{groupId}/candidate-questions/{questionId}/votes"),
    ("questions", "DELETE", "/groups/{groupId}/candidate-questions/{questionId}/votes"),
    ("questions", "DELETE", "/admin/groups/{groupId}/candidate-questions/{questionId}"),
    ("responses", "GET", "/groups/{groupId}/newsletters/{cycleId}/my-responses"),
    (
        "responses",
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
    ),
    (
        "responses",
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response",
    ),
    (
        "engagement",
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
    ),
    (
        "engagement",
        "POST",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments",
    ),
    (
        "engagement",
        "PATCH",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
    ),
    (
        "engagement",
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/comments/{commentId}",
    ),
    (
        "engagement",
        "GET",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions",
    ),
    (
        "engagement",
        "PUT",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
    ),
    (
        "engagement",
        "DELETE",
        "/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/reactions/{emoji}",
    ),
    ("push", "POST", "/push/subscribe"),
    ("push", "POST", "/push/unsubscribe"),
    ("push", "GET", "/push/subscriptions"),
    ("push", "POST", "/push/test"),
    ("push", "GET", "/push/preferences"),
    ("push", "PUT", "/push/preferences/{groupId}"),
]


class ApiStack(cdk.Stack):
    def __init__(
        self,
        scope: Construct,
        id: str,
        *,
        config: EnvConfig,
        table: dynamodb.Table,
        user_pool: cognito.IUserPool,
        user_pool_client: cognito.IUserPoolClient,
        bootstrap_client: cognito.IUserPoolClient | None,
        certificate: acm.ICertificate,
        cycle_tick_fn: aws_lambda.IFunction,
        notify_tick_fn: aws_lambda.IFunction,
        media_originals_bucket: s3.Bucket,
        media_processed_bucket: s3.Bucket,
        avatars_originals_bucket: s3.Bucket,
        **kwargs: Any,
    ) -> None:
        super().__init__(scope, id, **kwargs)

        self._config = config
        self._table = table

        log_retention = (
            logs.RetentionDays.ONE_MONTH
            if config.env == "dev"
            else logs.RetentionDays.THREE_MONTHS
        )
        self._log_retention = log_retention
        self._removal_policy = (
            cdk.RemovalPolicy.DESTROY
            if config.env == "dev"
            else cdk.RemovalPolicy.RETAIN
        )
        media_delete_env = {
            "MEDIA_ORIGINALS_BUCKET": media_originals_bucket.bucket_name,
            "PROCESSED_BUCKET": media_processed_bucket.bucket_name,
        }
        avatar_delete_env = {
            "AVATARS_ORIGINALS_BUCKET": avatars_originals_bucket.bucket_name,
            "PROCESSED_BUCKET": media_processed_bucket.bucket_name,
        }

        # Plain `s3:DeleteObject` rather than `grant_delete`, whose
        # `s3:DeleteObject*` also covers version and tagging deletes.
        delete_media = iam.PolicyStatement(
            actions=["s3:DeleteObject"],
            resources=[
                # `docs/design/media.md` §2 key layout.
                media_originals_bucket.arn_for_objects("uploads/*"),
                media_processed_bucket.arn_for_objects("img/*"),
            ],
        )
        delete_avatars = iam.PolicyStatement(
            actions=["s3:DeleteObject"],
            resources=[
                # `docs/design/media.md` §11 key layout.
                avatars_originals_bucket.arn_for_objects("uploads/*"),
                media_processed_bucket.arn_for_objects("avatar/*"),
            ],
        )
        # Image URLs are presigned GETs, which carry the signer's permissions
        # (`docs/design/media.md` §4.1), so each Lambda that returns image or
        # avatar URLs can read exactly the prefix it signs.
        media_urls_env = {"PROCESSED_BUCKET": media_processed_bucket.bucket_name}
        read_images = iam.PolicyStatement(
            actions=["s3:GetObject"],
            resources=[media_processed_bucket.arn_for_objects("img/*")],
        )
        read_avatars = iam.PolicyStatement(
            actions=["s3:GetObject"],
            resources=[media_processed_bucket.arn_for_objects("avatar/*")],
        )

        self.invites_fn = self._handler_lambda(
            "Invites",
            binary_name="invites-api",
            description="Invite create/list/revoke/redeem",
        )
        self.groups_fn = self._handler_lambda(
            "Groups",
            binary_name="groups-api",
            description="Config, profile, group and member routes",
            extra_environment={
                "VAPID_PUBLIC_KEY": secret_ref(
                    config.app_secret_arn, APP_SECRET_VAPID_PUBLIC_KEY
                ),
                # Old-avatar cleanup on `PATCH /me`.
                **avatar_delete_env,
                **media_urls_env,
            },
        )
        self.groups_fn.add_to_role_policy(read_avatars)
        self.groups_fn.add_to_role_policy(delete_avatars)
        self.newsletters_fn = self._handler_lambda(
            "Newsletters",
            binary_name="newsletters-api",
            description="Newsletter list + detail",
            extra_environment=media_urls_env,
        )
        # Answer and comment images, plus asker and comment-author avatars.
        self.newsletters_fn.add_to_role_policy(read_images)
        self.newsletters_fn.add_to_role_policy(read_avatars)
        self.questions_fn = self._handler_lambda(
            "Questions",
            binary_name="questions-api",
            description="Candidate question CRUD + votes",
            extra_environment=media_urls_env,
        )
        self.questions_fn.add_to_role_policy(read_avatars)
        self.responses_fn = self._handler_lambda(
            "Responses",
            binary_name="responses-api",
            description="My response get/list/put",
            memory_size=512,
        )
        self.engagement_fn = self._handler_lambda(
            "Engagement",
            binary_name="engagement-api",
            description="Comments + reactions",
            extra_environment={
                # Comment-image release on soft-delete/replace.
                **media_delete_env,
                **media_urls_env,
            },
        )
        self.engagement_fn.add_to_role_policy(read_images)
        self.engagement_fn.add_to_role_policy(delete_media)
        self.media_fn = self._handler_lambda(
            "Media",
            binary_name="media-api",
            description="Uploads and avatars",
            memory_size=512,
            extra_environment={
                **media_delete_env,
                **avatar_delete_env,
            },
        )
        # Presigning a PUT needs s3:PutObject on the signer's role.
        media_originals_bucket.grant_put(self.media_fn)
        avatars_originals_bucket.grant_put(self.media_fn)
        self.media_fn.add_to_role_policy(read_images)
        self.media_fn.add_to_role_policy(read_avatars)
        # `DELETE /uploads/{id}` and `DELETE /avatars/{id}`.
        self.media_fn.add_to_role_policy(delete_media)
        self.media_fn.add_to_role_policy(delete_avatars)

        self.push_fn = self._handler_lambda(
            "Push",
            binary_name="push-api",
            description="Web Push subscriptions and test messages",
            timeout_secs=60,
            extra_environment={
                # VAPID keys are its `vapid_*` JSON keys.
                "APP_SECRET_ARN": config.app_secret_arn,
            },
        )
        self.push_fn.add_to_role_policy(_read_app_secret(config))

        # --- HTTP API ---

        allowed_origins = [f"https://{config.domain}"]
        if config.env == "dev":
            allowed_origins.append("http://localhost:5173")

        # The dev-only bootstrap client is included so password-auth tokens from
        # /admin/bootstrap-login pass the same authorizer (docs/development/environments.md §6). Prod has no
        # such client, so its audience is the frontend client alone (docs/design/infrastructure.md §6.1).
        jwt_audience = [user_pool_client.user_pool_client_id]
        if bootstrap_client is not None:
            jwt_audience.append(bootstrap_client.user_pool_client_id)
        authorizer = apigw_authorizers.HttpJwtAuthorizer(
            "CognitoJwtAuthorizer",
            f"https://cognito-idp.{self.region}.amazonaws.com/{user_pool.user_pool_id}",
            identity_source=["$request.header.Authorization"],
            jwt_audience=jwt_audience,
        )

        self.api = apigw.HttpApi(
            self,
            "HttpApi",
            api_name=f"OpenNewsletter-Api-{config.env}",
            default_authorizer=authorizer,
            cors_preflight=apigw.CorsPreflightOptions(
                allow_origins=allowed_origins,
                allow_methods=[
                    apigw.CorsHttpMethod.GET,
                    apigw.CorsHttpMethod.POST,
                    apigw.CorsHttpMethod.PUT,
                    apigw.CorsHttpMethod.PATCH,
                    apigw.CorsHttpMethod.DELETE,
                    apigw.CorsHttpMethod.OPTIONS,
                ],
                allow_headers=["Authorization", "Content-Type", "x-correlation-id"],
                expose_headers=["x-correlation-id"],
                max_age=cdk.Duration.seconds(600),
            ),
        )

        handlers = {
            "invites": self.invites_fn,
            "groups": self.groups_fn,
            "media": self.media_fn,
            "newsletters": self.newsletters_fn,
            "questions": self.questions_fn,
            "responses": self.responses_fn,
            "engagement": self.engagement_fn,
            "push": self.push_fn,
        }
        for handler_key, method, path in _ROUTES:
            handler = handlers[handler_key]
            self.api.add_routes(
                path=path,
                methods=[apigw.HttpMethod[method]],
                integration=apigw_integrations.HttpLambdaIntegration(
                    f"{handler_key.title()}{method.title()}{_route_id(path)}",
                    handler,
                ),
            )

        # Dev-only fast-forward routes (`docs/design/api.md` §11a.1, §11a.2);
        # refuse with 404 in prod simply by not existing there.
        if config.env == "dev":
            self.api.add_routes(
                path="/admin/dev/tick/cycle",
                methods=[apigw.HttpMethod.POST],
                integration=apigw_integrations.HttpLambdaIntegration(
                    "AdminDevTickCycle", cycle_tick_fn
                ),
            )
            self.api.add_routes(
                path="/admin/dev/tick/notify",
                methods=[apigw.HttpMethod.POST],
                integration=apigw_integrations.HttpLambdaIntegration(
                    "AdminDevTickNotify", notify_tick_fn
                ),
            )

        self._add_access_logging()
        self._add_throttling()

        # Dev has no custom domains — the raw execute-api endpoint is used
        # (`docs/development/environments.md` §2).
        self.domain_name: apigw.DomainName | None = None
        if config.env != "dev":
            self.domain_name = apigw.DomainName(
                self,
                "ApiDomain",
                domain_name=config.api_domain,
                certificate=certificate,
            )
            apigw.ApiMapping(
                self,
                "ApiMapping",
                api=self.api,
                domain_name=self.domain_name,
                stage=self.api.default_stage,
            )

        cdk.CfnOutput(self, "ApiId", value=self.api.api_id)
        cdk.CfnOutput(self, "ApiEndpoint", value=self.api_endpoint)

        # Published for MonitoringStack to read via a CloudFormation dynamic
        # reference (`ssm_refs.py`) rather than a cross-stack construct ref —
        # the api id isn't known until deploy, and MonitoringStack is volatile.
        ssm.StringParameter(
            self,
            "HttpApiIdParam",
            parameter_name=http_api_id_parameter_name(config.env),
            string_value=self.api.api_id,
        )

    @property
    def api_endpoint(self) -> str:
        """Base URL clients should call: the custom domain in prod, raw API in dev."""
        if self.domain_name is not None:
            return f"https://{self._config.api_domain}"
        return self.api.api_endpoint

    def _log_group(self, name: str) -> logs.LogGroup:
        return logs.LogGroup(
            self,
            f"{name}Logs",
            log_group_name=f"/aws/lambda/OpenNewsletter-{name}-{self._config.env}",
            retention=self._log_retention,
            removal_policy=self._removal_policy,
        )

    def _handler_lambda(
        self,
        name: str,
        *,
        binary_name: str,
        description: str,
        memory_size: int = 256,
        timeout_secs: int = 10,
        extra_environment: dict[str, str] | None = None,
    ) -> aws_lambda.Function:
        environment = {
            "TABLE_NAME": self._table.table_name,
            "RUST_LOG": "info",
            "ENV": self._config.env,
            "API_BASE_URL": f"https://{self._config.api_domain}",
            **(extra_environment or {}),
        }

        function = aws_lambda.Function(
            self,
            f"{name}Fn",
            function_name=f"OpenNewsletter-{name}-{self._config.env}",
            description=description,
            runtime=aws_lambda.Runtime.PROVIDED_AL2023,
            architecture=aws_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=lambda_code(binary_name),
            memory_size=memory_size,
            timeout=cdk.Duration.seconds(timeout_secs),
            environment=environment,
            log_group=self._log_group(name),
        )
        # grant_read_write_data covers the table and its indexes.
        self._table.grant_read_write_data(function)
        return function

    def _add_access_logging(self) -> None:
        access_logs = logs.LogGroup(
            self,
            "ApiAccessLogs",
            log_group_name=f"/aws/http-api/OpenNewsletter-{self._config.env}",
            retention=self._log_retention,
            removal_policy=self._removal_policy,
        )
        stage = self.api.default_stage
        if stage is None:
            return
        cfn_stage = stage.node.default_child
        if not isinstance(cfn_stage, apigw.CfnStage):
            return
        cfn_stage.access_log_settings = apigw.CfnStage.AccessLogSettingsProperty(
            destination_arn=access_logs.log_group_arn,
            format=json.dumps(
                {
                    "requestId": "$context.requestId",
                    "correlationId": "$context.requestHeaderOverride.header.x-correlation-id",
                    "ip": "$context.identity.sourceIp",
                    "requestTime": "$context.requestTime",
                    "httpMethod": "$context.httpMethod",
                    "routeKey": "$context.routeKey",
                    "status": "$context.status",
                    "protocol": "$context.protocol",
                    "responseLength": "$context.responseLength",
                    "integrationErrorMessage": "$context.integrationErrorMessage",
                }
            ),
        )

    def _add_throttling(self) -> None:
        stage = self.api.default_stage
        if stage is None:
            return
        cfn_stage = stage.node.default_child
        if not isinstance(cfn_stage, apigw.CfnStage):
            return
        cfn_stage.default_route_settings = apigw.CfnStage.RouteSettingsProperty(
            throttling_burst_limit=50,
            throttling_rate_limit=25,
        )


def _read_app_secret(config: EnvConfig) -> iam.PolicyStatement:
    """Read access to the one per-env app secret (`docs/design/infrastructure.md` §7.1). Granted only to
    the Lambdas that read a private key from it at runtime: media, push and
    notify-tick (NotificationsStack builds its own copy)."""
    return iam.PolicyStatement(
        actions=["secretsmanager:GetSecretValue"],
        resources=[config.app_secret_arn],
    )


def _route_id(path: str) -> str:
    """A construct-id-safe slug for a route path."""
    cleaned = path.replace("{", "").replace("}", "")
    return "".join(part.title() for part in cleaned.split("/") if part) or "Root"
