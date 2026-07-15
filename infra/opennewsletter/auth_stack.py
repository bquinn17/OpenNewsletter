"""AuthStack — Cognito User Pool with federated IdPs, app clients, hosted UI.

Lambda triggers (PreSignUp, PostConfirmation) are wired in M4 once the
lambda-invites binary exists. Placeholder ARNs are used for IdP secrets
until M1 operator tasks are complete.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_cognito as cognito
from constructs import Construct

from .config import EnvConfig

# CloudFormation dynamic reference; resolves at deploy time, never logged.
def _sm_ref(secret_arn: str, json_field: str) -> str:
    return f"{{{{resolve:secretsmanager:{secret_arn}:SecretString:{json_field}}}}}"


class AuthStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any) -> None:
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
            custom_attributes={
                "pendingInvite": cognito.StringAttribute(min_len=0, max_len=64, mutable=True),
            },
            # Keep the user pool in dev too — recreating it loses all users.
            removal_policy=cdk.RemovalPolicy.RETAIN,
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
        cdk.CfnOutput(self, "UserPoolClientId", value=self.frontend_client.user_pool_client_id)
        cdk.CfnOutput(
            self,
            "UserPoolBootstrapClientId",
            value=self.bootstrap_client.user_pool_client_id,
        )
        cdk.CfnOutput(self, "HostedUiDomain", value=self.domain.base_url())
