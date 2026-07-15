"""Environment configuration for the CDK app.

Values come from .env.local (dev) or environment variables (CI/prod).
Placeholder ARNs are used when M1 operator tasks are not yet complete.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

_REPO_ROOT = Path(__file__).parent.parent.parent
_PLACEHOLDER_ARN = "arn:aws:secretsmanager:us-east-1:123456789012:secret:placeholder"


@dataclass(frozen=True)
class EnvConfig:
    env: str
    domain: str
    api_domain: str
    hosted_zone_id: str | None
    hosted_zone_name: str | None
    cognito_domain_prefix: str
    # Each ARN points to a Secrets Manager secret storing JSON with IdP credentials.
    # Google/Facebook: {"clientId":"...","clientSecret":"..."}
    # Apple: {"teamId":"...","keyId":"...","privateKey":"...","clientId":"..."}
    google_oauth_secret_arn: str
    apple_oauth_secret_arn: str
    facebook_oauth_secret_arn: str
    vapid_secret_arn: str
    cdn_signing_secret_arn: str
    log_retention_days: int
    alarm_email: str | None


def load_config(env: str) -> EnvConfig:
    """Load EnvConfig for the given environment name (dev|prod)."""
    raw: dict[str, str] = {}

    env_file = _REPO_ROOT / ".env.local"
    if env_file.exists():
        # Parse manually to avoid a hard dep on python-dotenv at import time
        # in contexts where only the package is installed without extras.
        try:
            from dotenv import dotenv_values  # type: ignore[import-untyped]

            raw = {k: v for k, v in dotenv_values(env_file).items() if v is not None}
        except ImportError:
            for line in env_file.read_text().splitlines():
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, _, v = line.partition("=")
                    raw[k.strip()] = v.strip()

    def get(key: str, default: str = "") -> str:
        value = raw.get(key)
        return value if value else os.environ.get(key, default)

    def get_opt(key: str) -> str | None:
        v = raw.get(key) or os.environ.get(key)
        return v if v else None

    return EnvConfig(
        env=env,
        domain=get("ROOT_DOMAIN", f"{env}.opennewsletter.example.com"),
        api_domain=get("API_DOMAIN", f"api-{env}.opennewsletter.example.com"),
        hosted_zone_id=get_opt("HOSTED_ZONE_ID"),
        hosted_zone_name=get_opt("HOSTED_ZONE_NAME"),
        cognito_domain_prefix=get("COGNITO_DOMAIN_PREFIX", f"opennewsletter-{env}"),
        google_oauth_secret_arn=get("GOOGLE_OAUTH_SECRET_ARN", _PLACEHOLDER_ARN),
        apple_oauth_secret_arn=get("APPLE_OAUTH_SECRET_ARN", _PLACEHOLDER_ARN),
        facebook_oauth_secret_arn=get("FACEBOOK_OAUTH_SECRET_ARN", _PLACEHOLDER_ARN),
        vapid_secret_arn=get("VAPID_SECRET_ARN", _PLACEHOLDER_ARN),
        cdn_signing_secret_arn=get("CDN_SIGNING_SECRET_ARN", _PLACEHOLDER_ARN),
        log_retention_days=30 if env == "dev" else 90,
        alarm_email=get_opt("ALARM_EMAIL"),
    )
