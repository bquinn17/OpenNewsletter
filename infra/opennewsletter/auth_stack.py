"""AuthStack — Cognito User Pool with federated IdPs, app clients, hosted UI.

The `PreSignUp` trigger Lambda is built here rather than in ApiStack: CDK attaches
trigger wiring to the user pool's own stack, so defining the function elsewhere
would make this stack depend on that one while ApiStack already depends on this one
for the JWT authorizer. There is no `PostConfirmation` trigger
(`docs/design/auth.md` §5).

IdP credentials come from the one per-env app secret (`config.app_secret_arn`),
via CloudFormation dynamic references; a placeholder ARN is used until the
operator creates it.

The `admin-bootstrap` password client exists only in dev
(`docs/design/infrastructure.md` §4.3): prod bootstraps its first admin through
an email-bound invite redeemed via Google instead (`docs/design/auth.md` §9.1a).
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_cognito as cognito
from aws_cdk import aws_lambda
from aws_cdk import aws_logs as logs
from constructs import Construct

from .config import (
    APP_SECRET_FACEBOOK_APP_ID,
    APP_SECRET_FACEBOOK_APP_SECRET,
    APP_SECRET_GOOGLE_CLIENT_ID,
    APP_SECRET_GOOGLE_CLIENT_SECRET,
    EnvConfig,
    secret_ref,
)
from .lambda_assets import lambda_code


class AuthStack(cdk.Stack):
    def __init__(
        self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any
    ) -> None:
        super().__init__(scope, id, **kwargs)

        self.user_pool = cognito.UserPool(
            self,
            "UserPool",
            user_pool_name=f"OpenNewsletter-{config.env}",
            sign_in_aliases=cognito.SignInAliases(email=True),
            self_sign_up_enabled=False,
            password_policy=cognito.PasswordPolicy(
                min_length=12,
                require_lowercase=True,
                require_uppercase=True,
                require_digits=True,
                require_symbols=True,
            ),
            mfa=cognito.Mfa.OPTIONAL,
            mfa_second_factor=cognito.MfaSecondFactor(otp=True, sms=False),
            account_recovery=cognito.AccountRecovery.EMAIL_ONLY,
            standard_attributes=cognito.StandardAttributes(
                email=cognito.StandardAttribute(required=True, mutable=True),
                fullname=cognito.StandardAttribute(required=False, mutable=True),
            ),
            # No custom attributes: the invite code rides in the OIDC `state`
            # parameter, not on the user record (`docs/design/auth.md` §1).
            # Keep the user pool in dev too — recreating it loses all users.
            removal_policy=cdk.RemovalPolicy.RETAIN,
        )

        # Pass-through that auto-confirms federated accounts. It never sees the
        # invite code and never touches DynamoDB (`docs/design/auth.md` §3).
        self.presignup_fn = aws_lambda.Function(
            self,
            "PreSignUpFn",
            function_name=f"OpenNewsletter-PreSignUp-{config.env}",
            description="Cognito PreSignUp pass-through (auto-confirm)",
            runtime=aws_lambda.Runtime.PROVIDED_AL2023,
            architecture=aws_lambda.Architecture.ARM_64,
            handler="bootstrap",
            code=lambda_code("invites-presignup"),
            memory_size=128,
            timeout=cdk.Duration.seconds(5),
            environment={"RUST_LOG": "info", "ENV": config.env},
            log_group=logs.LogGroup(
                self,
                "PreSignUpLogs",
                log_group_name=f"/aws/lambda/OpenNewsletter-PreSignUp-{config.env}",
                retention=logs.RetentionDays.ONE_MONTH
                if config.env == "dev"
                else logs.RetentionDays.THREE_MONTHS,
                removal_policy=cdk.RemovalPolicy.DESTROY
                if config.env == "dev"
                else cdk.RemovalPolicy.RETAIN,
            ),
        )
        self.user_pool.add_trigger(
            cognito.UserPoolOperation.PRE_SIGN_UP,
            self.presignup_fn,
        )

        # --- Federated IdPs ---
        # Uses CFN dynamic references so secrets stay out of the synthesised template.

        google_idp = cognito.UserPoolIdentityProviderGoogle(
            self,
            "GoogleIdP",
            user_pool=self.user_pool,
            # client_id is public-facing; resolved from the same secret for consistency.
            client_id=secret_ref(config.app_secret_arn, APP_SECRET_GOOGLE_CLIENT_ID),
            client_secret_value=cdk.SecretValue.secrets_manager(
                config.app_secret_arn, json_field=APP_SECRET_GOOGLE_CLIENT_SECRET
            ),
            scopes=["openid", "email", "profile"],
            attribute_mapping=cognito.AttributeMapping(
                email=cognito.ProviderAttribute.GOOGLE_EMAIL,
                fullname=cognito.ProviderAttribute.GOOGLE_NAME,
                # `POST /invites/redeem` checks this claim for an email-bound
                # invite (`docs/design/auth.md` §4.2). CDK has no named field
                # for it, so it goes through `custom`. Facebook doesn't supply
                # it, which is why a bound invite can only be redeemed via Google.
                custom={
                    "email_verified": cognito.ProviderAttribute.other("email_verified"),
                },
            ),
        )

        # No Apple IdP: Sign In with Apple requires a paid ($99/yr) Apple
        # Developer membership, which the owner declined (decision recorded
        # in `docs/decisions.md`). Google + Facebook only.

        facebook_idp = cognito.UserPoolIdentityProviderFacebook(
            self,
            "FacebookIdP",
            user_pool=self.user_pool,
            client_id=secret_ref(config.app_secret_arn, APP_SECRET_FACEBOOK_APP_ID),
            client_secret=secret_ref(
                config.app_secret_arn, APP_SECRET_FACEBOOK_APP_SECRET
            ),
            scopes=["public_profile", "email"],
            attribute_mapping=cognito.AttributeMapping(
                email=cognito.ProviderAttribute.FACEBOOK_EMAIL,
                fullname=cognito.ProviderAttribute.FACEBOOK_NAME,
            ),
        )

        # --- App clients ---

        callback_urls = [f"https://{config.domain}/auth/callback"]
        logout_urls = [f"https://{config.domain}/"]
        if config.env == "dev":
            callback_urls.append("http://localhost:5173/auth/callback")
            logout_urls.append("http://localhost:5173/")

        self.frontend_client = self.user_pool.add_client(
            "FrontendClient",
            user_pool_client_name="frontend",
            generate_secret=False,
            o_auth=cognito.OAuthSettings(
                flows=cognito.OAuthFlows(authorization_code_grant=True),
                scopes=[
                    cognito.OAuthScope.OPENID,
                    cognito.OAuthScope.EMAIL,
                    cognito.OAuthScope.PROFILE,
                ],
                callback_urls=callback_urls,
                logout_urls=logout_urls,
            ),
            supported_identity_providers=[
                cognito.UserPoolClientIdentityProvider.GOOGLE,
                cognito.UserPoolClientIdentityProvider.FACEBOOK,
            ],
            id_token_validity=cdk.Duration.minutes(60),
            access_token_validity=cdk.Duration.minutes(60),
            refresh_token_validity=cdk.Duration.days(30),
        )
        self.frontend_client.node.add_dependency(google_idp)
        self.frontend_client.node.add_dependency(facebook_idp)

        # Dev only: prod has no password auth at all (module docstring).
        self.bootstrap_client: cognito.UserPoolClient | None = None
        if config.env == "dev":
            self.bootstrap_client = self.user_pool.add_client(
                "AdminBootstrapClient",
                user_pool_client_name="admin-bootstrap",
                generate_secret=False,
                auth_flows=cognito.AuthFlow(user_password=True),
                supported_identity_providers=[
                    cognito.UserPoolClientIdentityProvider.COGNITO,
                ],
                id_token_validity=cdk.Duration.minutes(60),
                access_token_validity=cdk.Duration.minutes(60),
                refresh_token_validity=cdk.Duration.days(30),
            )

        # --- Hosted UI domain ---

        self.domain = self.user_pool.add_domain(
            "CognitoDomain",
            cognito_domain=cognito.CognitoDomainOptions(
                domain_prefix=config.cognito_domain_prefix,
            ),
        )

        # --- Outputs ---

        cdk.CfnOutput(self, "UserPoolId", value=self.user_pool.user_pool_id)
        cdk.CfnOutput(self, "UserPoolArn", value=self.user_pool.user_pool_arn)
        cdk.CfnOutput(
            self, "UserPoolClientId", value=self.frontend_client.user_pool_client_id
        )
        if self.bootstrap_client is not None:
            cdk.CfnOutput(
                self,
                "UserPoolBootstrapClientId",
                value=self.bootstrap_client.user_pool_client_id,
            )
        cdk.CfnOutput(self, "HostedUiDomain", value=self.domain.base_url())
