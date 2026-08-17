"""AuthStack — Cognito User Pool with federated IdPs, app clients, hosted UI.

The `PreSignUp` trigger Lambda is built here rather than in ApiStack: CDK attaches
trigger wiring to the user pool's own stack, so defining the function elsewhere
would make this stack depend on that one while ApiStack already depends on this one
for the JWT authorizer. There is no `PostConfirmation` trigger
(`plans/05-auth-flow.md` §5).

Placeholder ARNs are used for IdP secrets until M1 operator tasks are complete.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_cognito as cognito
from aws_cdk import aws_lambda as aws_lambda
from aws_cdk import aws_logs as logs
from constructs import Construct

from .config import EnvConfig
from .lambda_assets import lambda_code


# CloudFormation dynamic reference; resolves at deploy time, never logged.
def _sm_ref(secret_arn: str, json_field: str) -> str:
    return f"{{{{resolve:secretsmanager:{secret_arn}:SecretString:{json_field}}}}}"


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
            # parameter, not on the user record (`plans/05-auth-flow.md` §1).
            # Keep the user pool in dev too — recreating it loses all users.
            removal_policy=cdk.RemovalPolicy.RETAIN,
        )

        # Pass-through that auto-confirms federated accounts. It never sees the
        # invite code and never touches DynamoDB (`plans/05-auth-flow.md` §3).
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
            client_id=_sm_ref(config.google_oauth_secret_arn, "clientId"),
            client_secret_value=cdk.SecretValue.secrets_manager(
                config.google_oauth_secret_arn, json_field="clientSecret"
            ),
            scopes=["openid", "email", "profile"],
            attribute_mapping=cognito.AttributeMapping(
                email=cognito.ProviderAttribute.GOOGLE_EMAIL,
                fullname=cognito.ProviderAttribute.GOOGLE_NAME,
            ),
        )

        apple_idp = cognito.UserPoolIdentityProviderApple(
            self,
            "AppleIdP",
            user_pool=self.user_pool,
            client_id=_sm_ref(config.apple_oauth_secret_arn, "clientId"),
            team_id=_sm_ref(config.apple_oauth_secret_arn, "teamId"),
            key_id=_sm_ref(config.apple_oauth_secret_arn, "keyId"),
            private_key_value=cdk.SecretValue.secrets_manager(
                config.apple_oauth_secret_arn, json_field="privateKey"
            ),
            scopes=["openid", "email", "name"],
            attribute_mapping=cognito.AttributeMapping(
                email=cognito.ProviderAttribute.other("email"),
                fullname=cognito.ProviderAttribute.other("name"),
            ),
        )

        facebook_idp = cognito.UserPoolIdentityProviderFacebook(
            self,
            "FacebookIdP",
            user_pool=self.user_pool,
            client_id=_sm_ref(config.facebook_oauth_secret_arn, "clientId"),
            client_secret=_sm_ref(config.facebook_oauth_secret_arn, "clientSecret"),
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
                cognito.UserPoolClientIdentityProvider.APPLE,
                cognito.UserPoolClientIdentityProvider.FACEBOOK,
            ],
            id_token_validity=cdk.Duration.minutes(60),
            access_token_validity=cdk.Duration.minutes(60),
            refresh_token_validity=cdk.Duration.days(30),
        )
        self.frontend_client.node.add_dependency(google_idp)
        self.frontend_client.node.add_dependency(apple_idp)
        self.frontend_client.node.add_dependency(facebook_idp)

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
        cdk.CfnOutput(
            self,
            "UserPoolBootstrapClientId",
            value=self.bootstrap_client.user_pool_client_id,
        )
        cdk.CfnOutput(self, "HostedUiDomain", value=self.domain.base_url())
