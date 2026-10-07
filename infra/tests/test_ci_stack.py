"""CiStack assertions per 11-testing-ci-cd.md / M13 (GitHub OIDC deploy roles).

Run with: pytest infra/tests/
"""

from __future__ import annotations

import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.ci_stack import CiStack
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


def test_deploy_role_trust_is_scoped_to_main_branch(
    ci_template: assertions.Template,
) -> None:
    deploy_role = _role_by_name(ci_template, "OpenNewsletter-GithubDeploy")
    statement = deploy_role["Properties"]["AssumeRolePolicyDocument"]["Statement"][0]
    assert (
        statement["Condition"]["StringEquals"][
            "token.actions.githubusercontent.com:sub"
        ]
        == "repo:bquinn17/OpenNewsletter:ref:refs/heads/main"
    )
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


def test_diff_role_policy_only_assumes_lookup_role(
    ci_template: assertions.Template,
) -> None:
    policies = ci_template.find_resources("AWS::IAM::Policy")
    diff_role_logical_id = _role_logical_id(ci_template, "OpenNewsletter-GithubDiff")
    (diff_policy,) = [
        p
        for p in policies.values()
        if any(r["Ref"] == diff_role_logical_id for r in p["Properties"]["Roles"])
    ]
    actions = {
        statement["Action"]
        if isinstance(statement["Action"], str)
        else tuple(statement["Action"])
        for statement in diff_policy["Properties"]["PolicyDocument"]["Statement"]
    }
    assert "sts:AssumeRole" in actions
    # No action in the diff role's policy may be a write/deploy action.
    for statement in diff_policy["Properties"]["PolicyDocument"]["Statement"]:
        action = statement["Action"]
        actions_list = [action] if isinstance(action, str) else action
        for a in actions_list:
            assert a in (
                "sts:AssumeRole",
                "cloudformation:DescribeStacks",
                "cloudformation:GetTemplate",
            )


def test_outputs_both_role_arns(ci_template: assertions.Template) -> None:
    outputs = ci_template.find_outputs("*")
    assert "GithubDeployRoleArn" in outputs
    assert "GithubDiffRoleArn" in outputs
