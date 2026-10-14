"""`load_config` assertions for the M13.5 config changes
(`docs/design/infrastructure.md` §2).

Each test points `config._REPO_ROOT` at an empty directory, so a developer's
`.env.local` can't leak into the result.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

import pytest

from opennewsletter import config

_ENV_KEYS = (
    "APP_SECRET_ARN",
    "BOOTSTRAP_ADMIN_EMAIL",
    "ROOT_DOMAIN",
    "COGNITO_DOMAIN_PREFIX",
)


@pytest.fixture(autouse=True)
def isolated_env(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(config, "_REPO_ROOT", tmp_path)
    for key in _ENV_KEYS:
        monkeypatch.delenv(key, raising=False)


def test_app_secret_arn_is_read_from_the_environment(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    arn = (
        "arn:aws:secretsmanager:us-east-1:111111111111:secret:opennewsletter/dev-AbCdEf"
    )
    monkeypatch.setenv("APP_SECRET_ARN", arn)
    assert config.load_config("dev").app_secret_arn == arn


def test_app_secret_arn_falls_back_to_a_placeholder() -> None:
    assert config.load_config("dev").app_secret_arn.endswith(":secret:placeholder")


def test_an_empty_variable_counts_as_unset(monkeypatch: pytest.MonkeyPatch) -> None:
    # GitHub Actions renders an unset `vars.X` as "".
    monkeypatch.setenv("ROOT_DOMAIN", "")
    monkeypatch.setenv("COGNITO_DOMAIN_PREFIX", "")
    loaded = config.load_config("prod")
    assert loaded.domain == "prod.opennewsletter.example.com"
    assert loaded.cognito_domain_prefix == "opennewsletter-prod"


def test_bootstrap_admin_email_is_prod_only(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("BOOTSTRAP_ADMIN_EMAIL", "Admin@Example.com")
    assert config.load_config("prod").bootstrap_admin_email == "Admin@Example.com"
    assert config.load_config("dev").bootstrap_admin_email is None


def test_bootstrap_admin_email_defaults_to_none() -> None:
    assert config.load_config("prod").bootstrap_admin_email is None


def test_secret_ref_is_a_cloudformation_dynamic_reference() -> None:
    assert config.secret_ref("arn:x", config.APP_SECRET_VAPID_PUBLIC_KEY) == (
        "{{resolve:secretsmanager:arn:x:SecretString:vapid_public_key}}"
    )
