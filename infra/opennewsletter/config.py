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

# JSON keys inside the app secret (`docs/design/infrastructure.md` §7.1).
# The backend reads the private ones at runtime; CDK only ever emits
# CloudFormation dynamic references to them, never their values.
APP_SECRET_GOOGLE_CLIENT_ID = "google_client_id"
APP_SECRET_GOOGLE_CLIENT_SECRET = "google_client_secret"
APP_SECRET_FACEBOOK_APP_ID = "facebook_app_id"
APP_SECRET_FACEBOOK_APP_SECRET = "facebook_app_secret"
APP_SECRET_VAPID_PUBLIC_KEY = "vapid_public_key"
APP_SECRET_VAPID_PRIVATE_KEY = "vapid_private_key"
APP_SECRET_VAPID_SUBJECT = "vapid_subject"
APP_SECRET_CDN_SIGNING_PRIVATE_KEY = "cdn_signing_private_key"


def secret_ref(secret_arn: str, json_field: str) -> str:
    """CloudFormation dynamic reference to one JSON key of a Secrets Manager
    secret. Resolves at deploy time, so the value never lands in the template."""
    return f"{{{{resolve:secretsmanager:{secret_arn}:SecretString:{json_field}}}}}"


@dataclass(frozen=True)
class EnvConfig:
    env: str
    domain: str
    api_domain: str
    hosted_zone_id: str | None
    hosted_zone_name: str | None
    cognito_domain_prefix: str
    # The one per-env Secrets Manager secret, `opennewsletter/{env}`
    # (`docs/design/infrastructure.md` §2, §7.1). A JSON object whose keys
    # are the `APP_SECRET_*` constants below. No Apple IdP — dropped, see
    # docs/decisions.md ("Sign in with Apple is not offered").
    app_secret_arn: str
    log_retention_days: int
    alarm_email: str | None
    # Prod only: the email `scripts/bootstrap_admin.py --env prod` binds the
    # first admin invite to (`docs/design/auth.md` §9.1a). No stack reads it.
    bootstrap_admin_email: str | None
    # "owner/repo" slug the GitHub OIDC trust conditions in CiStack match
    # against (`token.actions.githubusercontent.com:sub`).
    github_repo: str


def load_config(env: str) -> EnvConfig:
    """Load EnvConfig for the given environment name (dev|prod)."""
    raw: dict[str, str] = {}

    env_file = _REPO_ROOT / ".env.local"
    if env_file.exists():
        # Parse manually to avoid a hard dep on python-dotenv at import time
        # in contexts where only the package is installed without extras.
        try:
            from dotenv import dotenv_values

            raw = {k: v for k, v in dotenv_values(env_file).items() if v is not None}
        except ImportError:
            for line in env_file.read_text().splitlines():
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, _, v = line.partition("=")
                    raw[k.strip()] = v.strip()

    def get(key: str, default: str = "") -> str:
        # An empty value counts as unset: GitHub Actions renders an unset
        # `vars.X` as "", which must fall back to the default, not override it.
        value = raw.get(key)
        return value if value else (os.environ.get(key) or default)

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
        app_secret_arn=get("APP_SECRET_ARN", _PLACEHOLDER_ARN),
        log_retention_days=30 if env == "dev" else 90,
        alarm_email=get_opt("ALARM_EMAIL"),
        bootstrap_admin_email=(
            get_opt("BOOTSTRAP_ADMIN_EMAIL") if env == "prod" else None
        ),
        github_repo=get("GITHUB_REPO", "bquinn17/OpenNewsletter"),
    )
