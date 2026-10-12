"""CiStack assertions per 11-testing-ci-cd.md / M13 (GitHub OIDC deploy roles)
and 01-infrastructure-cdk.md §9.2a (the one account-wide budget, M13.5).

Run with: pytest infra/tests/
"""

from __future__ import annotations

import dataclasses
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.ci_stack import MONTHLY_BUDGET_USD, CiStack
from opennewsletter.config import EnvConfig, load_config

AWS_ENV = cdk.Environment(region="us-east-1")


@pytest.fixture(scope="module")
def dev_config() -> EnvConfig:
    return load_config("dev")


@pytest.fixture(scope="module")
def ci_template(dev_config: EnvConfig) -> assertions.Template:
    app = cdk.App()
    stack = CiStack(app, "TestCiStack", config=dev_config, env=AWS_ENV)
    return assertions.Template.from_stack(stack)


def test_oidc_provider_exists_exactly_once(ci_template: assertions.Template) -> None:
    ci_template.resource_count_is("Custom::AWSCDKOpenIdConnectProvider", 1)


def test_oidc_provider_targets_github(ci_template: assertions.Template) -> None:
    ci_template.has_resource_properties(
        "Custom::AWSCDKOpenIdConnectProvider",
        {
            "ClientIDList": ["sts.amazonaws.com"],
            "Url": "https://token.actions.githubusercontent.com",
        },
    )


def _role_by_name(template: assertions.Template, role_name: str) -> Mapping[str, Any]:
    roles = template.find_resources("AWS::IAM::Role")
    (role,) = [
        r for r in roles.values() if r["Properties"].get("RoleName") == role_name
    ]
    return role


def _role_logical_id(template: assertions.Template, role_name: str) -> str:
    roles = template.find_resources("AWS::IAM::Role")
    return next(
        k for k, v in roles.items() if v["Properties"].get("RoleName") == role_name
    )


def test_exactly_two_named_github_roles(ci_template: assertions.Template) -> None:
    # A third, unnamed role belongs to the OpenIdConnectProvider's own custom
    # resource provider (CDK-managed, not ours to assert the shape of).
    roles = ci_template.find_resources("AWS::IAM::Role")
    named = [
        r
        for r in roles.values()
        if r["Properties"].get("RoleName", "").startswith("OpenNewsletter-Github")
    ]
    assert len(named) == 2


def test_deploy_role_trust_is_scoped_to_main_and_the_production_environment(
    ci_template: assertions.Template,
) -> None:
    # A job with `environment: production` (deploy.yml's promote-prod) gets an
    # `environment:production` OIDC subject instead of its ref.
    deploy_role = _role_by_name(ci_template, "OpenNewsletter-GithubDeploy")
    statement = deploy_role["Properties"]["AssumeRolePolicyDocument"]["Statement"][0]
    assert statement["Condition"]["StringEquals"][
        "token.actions.githubusercontent.com:sub"
    ] == [
        "repo:bquinn17/OpenNewsletter:ref:refs/heads/main",
        "repo:bquinn17/OpenNewsletter:environment:production",
    ]
    assert "StringLike" not in statement["Condition"]
    assert (
        statement["Condition"]["StringEquals"][
            "token.actions.githubusercontent.com:aud"
        ]
        == "sts.amazonaws.com"
    )


def test_deploy_role_max_session_duration_is_one_hour(
    ci_template: assertions.Template,
) -> None:
    ci_template.has_resource_properties(
        "AWS::IAM::Role",
        {
            "RoleName": "OpenNewsletter-GithubDeploy",
            "MaxSessionDuration": 3600,
        },
    )


def test_deploy_role_policy_only_assumes_bootstrap_roles(
    ci_template: assertions.Template,
) -> None:
    deploy_role_logical_id = _role_logical_id(
        ci_template, "OpenNewsletter-GithubDeploy"
    )
    policies = ci_template.find_resources("AWS::IAM::Policy")
    (deploy_policy,) = [
        p
        for p in policies.values()
        if any(r["Ref"] == deploy_role_logical_id for r in p["Properties"]["Roles"])
    ]
    statements = deploy_policy["Properties"]["PolicyDocument"]["Statement"]
    assert len(statements) == 1
    assert statements[0]["Action"] == "sts:AssumeRole"
    (resource,) = (
        statements[0]["Resource"]
        if isinstance(statements[0]["Resource"], list)
        else [statements[0]["Resource"]]
    )
    # The Fn::Join'd ARN always ends in the bootstrap-role glob, never "*".
    joined = resource["Fn::Join"][1] if isinstance(resource, dict) else resource
    assert joined[-1] == ":role/cdk-hnb659fds-*"


def test_diff_role_trust_is_pull_request_only(ci_template: assertions.Template) -> None:
    diff_role = _role_by_name(ci_template, "OpenNewsletter-GithubDiff")
    statement = diff_role["Properties"]["AssumeRolePolicyDocument"]["Statement"][0]
    assert (
        statement["Condition"]["StringLike"]["token.actions.githubusercontent.com:sub"]
        == "repo:bquinn17/OpenNewsletter:pull_request"
    )
    # The diff role's trust must not accept the main-branch ref condition —
    # that would let a PR build assume the role meant for pushes to main.
    assert "ref:refs/heads/main" not in str(statement["Condition"])


def test_diff_role_policy_only_reads_stack_templates(
    ci_template: assertions.Template,
) -> None:
    policies = ci_template.find_resources("AWS::IAM::Policy")
    diff_role_logical_id = _role_logical_id(ci_template, "OpenNewsletter-GithubDiff")
    (diff_policy,) = [
        p
        for p in policies.values()
        if any(r["Ref"] == diff_role_logical_id for r in p["Properties"]["Roles"])
    ]
    # The PR job runs untrusted branch code with this role, so it may only read
    # stack templates. In particular no `sts:AssumeRole`: the CDK lookup role
    # carries `ReadOnlyAccess`, which can scan the table and read media.
    for statement in diff_policy["Properties"]["PolicyDocument"]["Statement"]:
        action = statement["Action"]
        actions_list = [action] if isinstance(action, str) else action
        for a in actions_list:
            assert a in (
                "cloudformation:DescribeStacks",
                "cloudformation:GetTemplate",
            )


def test_outputs_both_role_arns(ci_template: assertions.Template) -> None:
    outputs = ci_template.find_outputs("*")
    assert "GithubDeployRoleArn" in outputs
    assert "GithubDiffRoleArn" in outputs


# ---------------------------------------------------------------------------
# Account-wide budget (`01` §9.2a)
# ---------------------------------------------------------------------------


def _budget_template(alarm_email: str | None) -> assertions.Template:
    config = dataclasses.replace(load_config("dev"), alarm_email=alarm_email)
    app = cdk.App()
    stack = CiStack(app, "TestCiStackBudget", config=config, env=AWS_ENV)
    return assertions.Template.from_stack(stack)


def test_one_five_dollar_monthly_budget(ci_template: assertions.Template) -> None:
    ci_template.resource_count_is("AWS::Budgets::Budget", 1)
    ci_template.has_resource_properties(
        "AWS::Budgets::Budget",
        {
            "Budget": {
                "BudgetType": "COST",
                "TimeUnit": "MONTHLY",
                "BudgetLimit": {"Amount": 5, "Unit": "USD"},
            }
        },
    )
    assert MONTHLY_BUDGET_USD == 5


def test_budget_alerts_at_80_actual_and_100_forecasted() -> None:
    template = _budget_template("ops@example.com")
    (budget,) = template.find_resources("AWS::Budgets::Budget").values()
    notifications = budget["Properties"]["NotificationsWithSubscribers"]
    assert sorted(
        (n["Notification"]["NotificationType"], n["Notification"]["Threshold"])
        for n in notifications
    ) == [("ACTUAL", 80), ("FORECASTED", 100)]
    for n in notifications:
        assert n["Subscribers"] == [
            {"SubscriptionType": "EMAIL", "Address": "ops@example.com"}
        ]


def test_budget_exists_without_an_alarm_email() -> None:
    template = _budget_template(None)
    (budget,) = template.find_resources("AWS::Budgets::Budget").values()
    assert not budget["Properties"].get("NotificationsWithSubscribers")
