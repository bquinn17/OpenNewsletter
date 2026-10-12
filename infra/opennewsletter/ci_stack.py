"""CiStack — GitHub Actions OIDC trust, the deploy/diff roles it assumes, and
the one account-wide cost budget.

Account-wide, not suffixed by env: `app.py` instantiates this once as the
plain id "CiStack", so either `--context env=dev` or `env=prod` includes it,
and deploying from either context manages the same CloudFormation stack.

The real deploy/lookup privileges stay on the CDK bootstrap roles
(`cdk-hnb659fds-*`, created by `cdk bootstrap`); these roles are granted only
`sts:AssumeRole` onto those, so a compromised Actions run is bounded by
whatever the bootstrap roles themselves allow — this stack never hands out
direct resource permissions.

The $5/mo AWS Budget lives here because it's account-wide too: dev and prod
share the account, and a per-env budget would only see part of the bill
(`plans/01-infrastructure-cdk.md` §9.2a).
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_budgets as budgets
from aws_cdk import aws_iam as iam
from constructs import Construct

from .config import EnvConfig

_GITHUB_OIDC_HOST = "token.actions.githubusercontent.com"
_GITHUB_OIDC_AUDIENCE = "sts.amazonaws.com"

MONTHLY_BUDGET_USD = 5


class CiStack(cdk.Stack):
    def __init__(
        self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any
    ) -> None:
        super().__init__(scope, id, **kwargs)

        provider = iam.OpenIdConnectProvider(
            self,
            "GithubOidcProvider",
            url=f"https://{_GITHUB_OIDC_HOST}",
            client_ids=[_GITHUB_OIDC_AUDIENCE],
        )

        bootstrap_roles_arn = self.format_arn(
            service="iam",
            region="",
            resource="role",
            resource_name="cdk-hnb659fds-*",
        )
        lookup_role_arn = self.format_arn(
            service="iam",
            region="",
            resource="role",
            resource_name="cdk-hnb659fds-lookup-role-*",
        )

        # Assumed by both deploy.yml jobs on push to main. GitHub sets the
        # OIDC `sub` claim per job: the `dev` job's is the `main` ref, but a
        # job that declares an environment (`promote-prod` runs under
        # `production`) gets `environment:production` instead of its ref. So
        # the trust names exactly those two subjects: a PR build (even from
        # this repo) can never assume it, and a prod promotion can only come
        # through the `production` environment's required reviewer.
        self.deploy_role = iam.Role(
            self,
            "GithubDeployRole",
            role_name="OpenNewsletter-GithubDeploy",
            description=(
                "Assumed by GitHub Actions on push to main (dev job) and by "
                "the production environment (promote-prod); can only assume "
                "the CDK bootstrap roles, which do the real deploy work."
            ),
            max_session_duration=cdk.Duration.hours(1),
            assumed_by=iam.OpenIdConnectPrincipal(
                provider,
                conditions={
                    "StringEquals": {
                        f"{_GITHUB_OIDC_HOST}:aud": _GITHUB_OIDC_AUDIENCE,
                        f"{_GITHUB_OIDC_HOST}:sub": [
                            f"repo:{config.github_repo}:ref:refs/heads/main",
                            f"repo:{config.github_repo}:environment:production",
                        ],
                    },
                },
            ),
        )
        self.deploy_role.add_to_policy(
            iam.PolicyStatement(
                actions=["sts:AssumeRole"],
                resources=[bootstrap_roles_arn],
            )
        )

        # Assumed by infra-ci's PR job to post a read-only `cdk diff` against
        # prod. Only the lookup role (read-only) is assumable, plus the two
        # describe calls diff issues directly against the live stacks, scoped
        # to prod stack ARNs rather than "*".
        self.diff_role = iam.Role(
            self,
            "GithubDiffRole",
            role_name="OpenNewsletter-GithubDiff",
            description=(
                "Assumed by GitHub Actions on pull requests for a read-only "
                "cdk diff against prod; cannot deploy anything."
            ),
            max_session_duration=cdk.Duration.hours(1),
            assumed_by=iam.OpenIdConnectPrincipal(
                provider,
                conditions={
                    "StringEquals": {
                        f"{_GITHUB_OIDC_HOST}:aud": _GITHUB_OIDC_AUDIENCE,
                    },
                    "StringLike": {
                        f"{_GITHUB_OIDC_HOST}:sub": f"repo:{config.github_repo}:pull_request",
                    },
                },
            ),
        )
        self.diff_role.add_to_policy(
            iam.PolicyStatement(
                actions=["sts:AssumeRole"],
                resources=[lookup_role_arn],
            )
        )
        self.diff_role.add_to_policy(
            iam.PolicyStatement(
                actions=["cloudformation:DescribeStacks", "cloudformation:GetTemplate"],
                resources=[
                    self.format_arn(
                        service="cloudformation",
                        resource="stack",
                        resource_name="*-prod/*",
                    )
                ],
            )
        )

        self._add_budget(config)

        cdk.CfnOutput(
            self,
            "GithubDeployRoleArn",
            value=self.deploy_role.role_arn,
            description="Paste into the repo variable AWS_DEPLOY_ROLE_ARN",
        )
        cdk.CfnOutput(
            self,
            "GithubDiffRoleArn",
            value=self.diff_role.role_arn,
            description="Paste into the repo variable AWS_DIFF_ROLE_ARN",
        )

    def _add_budget(self, config: EnvConfig) -> None:
        # Alerts at 80% of actual and 100% of forecasted spend. The budget
        # exists even without an alarm email; it just has no subscribers then.
        # `config.alarm_email` is read from whichever env context deploys this
        # stack, so both contexts must agree on ALARM_EMAIL.
        def notify(
            kind: str, threshold: int, address: str
        ) -> budgets.CfnBudget.NotificationWithSubscribersProperty:
            return budgets.CfnBudget.NotificationWithSubscribersProperty(
                notification=budgets.CfnBudget.NotificationProperty(
                    notification_type=kind,
                    comparison_operator="GREATER_THAN",
                    threshold=threshold,
                    threshold_type="PERCENTAGE",
                ),
                subscribers=[
                    budgets.CfnBudget.SubscriberProperty(
                        subscription_type="EMAIL", address=address
                    )
                ],
            )

        email = config.alarm_email
        notifications = (
            [notify("ACTUAL", 80, email), notify("FORECASTED", 100, email)]
            if email
            else []
        )

        budgets.CfnBudget(
            self,
            "MonthlyCostBudget",
            budget=budgets.CfnBudget.BudgetDataProperty(
                budget_type="COST",
                time_unit="MONTHLY",
                budget_limit=budgets.CfnBudget.SpendProperty(
                    amount=MONTHLY_BUDGET_USD, unit="USD"
                ),
                budget_name="OpenNewsletter-account-monthly",
            ),
            notifications_with_subscribers=notifications,
        )
