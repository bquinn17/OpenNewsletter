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
    "CYCLE_FREQUENCY",
    "CYCLE_WEEKLY_START",
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


def test_cycle_frequency_defaults_to_weekly_in_dev() -> None:
    loaded = config.load_config("dev")
    assert loaded.cycle_frequency == "weekly"


def test_cycle_frequency_defaults_to_monthly_in_prod() -> None:
    loaded = config.load_config("prod")
    assert loaded.cycle_frequency == "monthly"


def test_cycle_frequency_is_read_from_the_environment(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CYCLE_FREQUENCY", "WEEKLY")
    loaded = config.load_config("prod")
    assert loaded.cycle_frequency == "weekly"  # normalized to lowercase


def test_cycle_frequency_rejects_invalid_values(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CYCLE_FREQUENCY", "biweekly")
    with pytest.raises(ValueError, match="expected 'monthly' or 'weekly'"):
        config.load_config("dev")


def test_cycle_weekly_start_defaults_to_sun_21_00() -> None:
    loaded = config.load_config("dev")
    assert loaded.cycle_weekly_start == "sun 21:00"


def test_cycle_weekly_start_is_read_from_the_environment(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CYCLE_WEEKLY_START", "MON 09:30")
    loaded = config.load_config("dev")
    # Stored as given; the Lambdas parse it case-insensitively.
    assert loaded.cycle_weekly_start == "MON 09:30"


@pytest.mark.parametrize(
    "value",
    [
        "xyz 09:00",
        "sun 25:00",
        "sun 24:00",
        "sun 09:61",
        "sun 9:00",
        "sun21:00",
        "sun  21:00",
        "sun\t21:00",
    ],
)
def test_cycle_weekly_start_rejects_malformed_values(
    monkeypatch: pytest.MonkeyPatch, value: str
) -> None:
    monkeypatch.setenv("CYCLE_WEEKLY_START", value)
    with pytest.raises(ValueError, match="Invalid CYCLE_WEEKLY_START"):
        config.load_config("dev")


@pytest.mark.parametrize(
    ("value", "parsed"),
    [("sun 21:00", (6, 21, 0)), ("Monday 09:05", (0, 9, 5)), ("SAT 00:00", (5, 0, 0))],
)
def test_parse_weekly_start_returns_weekday_hour_minute(
    value: str, parsed: tuple[int, int, int]
) -> None:
    assert config.parse_weekly_start(value) == parsed


def test_cycle_weekly_start_accepts_full_weekday_names(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("CYCLE_WEEKLY_START", "monday 09:00")
    loaded = config.load_config("dev")
    assert loaded.cycle_weekly_start == "monday 09:00"
