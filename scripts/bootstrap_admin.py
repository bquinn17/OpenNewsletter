#!/usr/bin/env python3
"""Create the very first group and its admin, without an invite code.

Invites can only be minted by an admin of an existing group, so the first group
has to come from outside the API. This script is that door
(`plans/05-auth-flow.md` §9.1).

Idempotent: re-running with the same `--admin-email` and `--group-name` reuses
the existing Cognito user and rewrites the same DynamoDB rows. Re-running with a
different `--group-name` creates an additional group with the same admin.

Usage:
    python3 scripts/bootstrap_admin.py --env dev \
        --admin-email me@example.com --group-name "Test"

Requires: boto3 installed, AWS credentials for the target account.
Reads:    infra/cdk.out/{env}-outputs.json (written by 'cdk deploy --outputs-file').
"""

from __future__ import annotations

import argparse
import getpass
import json
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

try:
    import boto3
except ImportError:
    sys.exit("boto3 not installed. Run: pip install boto3")

REPO_ROOT = Path(__file__).parent.parent

# Mirrors AVATAR_COLOR_SLUGS + derive_avatar_color in
# backend/crates/shared/src/config.rs. Both sides must agree so a bootstrapped
# admin renders the same colour the API would have given them.
AVATAR_COLOR_SLUGS = ["red", "orange", "amber", "green", "teal", "blue", "violet", "pink"]
DEFAULT_GRADIENT = "grape-sky"
DEFAULT_TIMEZONE = "America/New_York"
DEFAULT_CYCLE_SETTINGS = {
    "questions_per_cycle": 5,
    "votes_per_user_per_cycle": 3,
    "response_window_days": 4,
    "auto_publish": True,
}
DEFAULT_NOTIFICATION_SETTINGS = {
    "offsets_hours_before_close": [96, 48, 24],
    "on_cycle_open": True,
}
DEFAULT_MEMBER_SOFT_CAP = 50


def derive_avatar_color(user_id: str) -> str:
    return AVATAR_COLOR_SLUGS[sum(user_id.encode()) % len(AVATAR_COLOR_SLUGS)]


def iso(dt: datetime) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def deterministic_id(seed: str) -> str:
    """UUID5 under a private namespace — stable across runs for the same seed."""
    namespace = uuid.UUID("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
    return str(uuid.uuid5(namespace, seed))


def load_outputs(env: str) -> dict[str, Any]:
    path = REPO_ROOT / "infra" / "cdk.out" / f"{env}-outputs.json"
    if not path.exists():
        sys.exit(
            f"Outputs file not found: {path}\n"
            f"Run 'cd infra && cdk deploy --context env={env} --all "
            f"--outputs-file cdk.out/{env}-outputs.json' first."
        )
    with path.open() as f:
        return json.load(f)


def get_output(outputs: dict[str, Any], stack: str, key: str, env: str) -> str:
    value = outputs.get(f"{stack}-{env}", {}).get(key, "")
    if not value:
        sys.exit(f"Missing CDK output {stack}-{env}.{key} — redeploy the {stack} stack.")
    return str(value)


def ensure_cognito_user(
    cognito: Any,
    user_pool_id: str,
    email: str,
    display_name: str,
    password: str,
) -> str:
    """Create (or find) the admin's Cognito user. Returns their `sub`."""
    try:
        existing = cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
        sub = next(a["Value"] for a in existing["UserAttributes"] if a["Name"] == "sub")
        print(f"  Cognito user already exists — {email} (sub={sub[:8]}…)")
        return sub
    except cognito.exceptions.UserNotFoundException:
        pass

    cognito.admin_create_user(
        UserPoolId=user_pool_id,
        Username=email,
        TemporaryPassword=f"{password}Tmp1!",
        MessageAction="SUPPRESS",
        UserAttributes=[
            {"Name": "email", "Value": email},
            {"Name": "email_verified", "Value": "true"},
            {"Name": "name", "Value": display_name},
        ],
    )
    # Set a permanent password immediately so the account skips FORCE_CHANGE and
    # can authenticate through the admin-bootstrap client straight away.
    cognito.admin_set_user_password(
        UserPoolId=user_pool_id,
        Username=email,
        Password=password,
        Permanent=True,
    )
    created = cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
    sub = next(a["Value"] for a in created["UserAttributes"] if a["Name"] == "sub")
    print(f"  Created Cognito user — {email} (sub={sub[:8]}…)")
    return sub


def ensure_cognito_admin_group(cognito: Any, user_pool_id: str, email: str) -> None:
    """Add the user to a Cognito-side `admins` group.

    Informational only — the authoritative role lives on the DynamoDB membership
    row, which is what every handler checks.
    """
    try:
        cognito.create_group(
            UserPoolId=user_pool_id,
            GroupName="admins",
            Description="Bootstrapped administrators (informational)",
        )
    except cognito.exceptions.GroupExistsException:
        pass
    cognito.admin_add_user_to_group(
        UserPoolId=user_pool_id, Username=email, GroupName="admins"
    )


def write_group(
    dynamodb: Any,
    table_name: str,
    *,
    user_id: str,
    cognito_sub: str,
    email: str,
    display_name: str,
    group_id: str,
    group_name: str,
    tz: str,
    now: datetime,
) -> None:
    """Write the User, sub lookup, Group and admin GroupMembership atomically."""
    timestamp = iso(now)
    dynamodb.transact_write_items(
        TransactItems=[
            {
                "Put": {
                    "TableName": table_name,
                    "Item": {
                        "pk": {"S": f"COGNITO_SUB#{cognito_sub}"},
                        "sk": {"S": "USER_ID"},
                        "entity": {"S": "CognitoSubLookup"},
                        "cognito_sub": {"S": cognito_sub},
                        "user_id": {"S": user_id},
                    },
                }
            },
            {
                "Put": {
                    "TableName": table_name,
                    "Item": {
                        "pk": {"S": f"USER#{user_id}"},
                        "sk": {"S": "PROFILE"},
                        "entity": {"S": "User"},
                        "user_id": {"S": user_id},
                        "cognito_sub": {"S": cognito_sub},
                        "email": {"S": email},
                        "display_name": {"S": display_name},
                        "avatar_color": {"S": derive_avatar_color(user_id)},
                        "avatar_media_id": {"NULL": True},
                        "created_at": {"S": timestamp},
                        "last_login_at": {"S": timestamp},
                    },
                }
            },
            {
                "Put": {
                    "TableName": table_name,
                    "Item": {
                        "pk": {"S": f"GROUP#{group_id}"},
                        "sk": {"S": "META"},
                        "entity": {"S": "Group"},
                        "group_id": {"S": group_id},
                        "name": {"S": group_name},
                        "timezone": {"S": tz},
                        "cycle_settings": {
                            "M": {
                                "questions_per_cycle": {
                                    "N": str(DEFAULT_CYCLE_SETTINGS["questions_per_cycle"])
                                },
                                "votes_per_user_per_cycle": {
                                    "N": str(DEFAULT_CYCLE_SETTINGS["votes_per_user_per_cycle"])
                                },
                                "response_window_days": {
                                    "N": str(DEFAULT_CYCLE_SETTINGS["response_window_days"])
                                },
                                "auto_publish": {
                                    "BOOL": bool(DEFAULT_CYCLE_SETTINGS["auto_publish"])
                                },
                            }
                        },
                        "notification_settings": {
                            "M": {
                                "offsets_hours_before_close": {
                                    "L": [
                                        {"N": str(h)}
                                        for h in DEFAULT_NOTIFICATION_SETTINGS[
                                            "offsets_hours_before_close"
                                        ]
                                    ]
                                },
                                "on_cycle_open": {
                                    "BOOL": bool(
                                        DEFAULT_NOTIFICATION_SETTINGS["on_cycle_open"]
                                    )
                                },
                            }
                        },
                        "member_count": {"N": "1"},
                        "member_soft_cap": {"N": str(DEFAULT_MEMBER_SOFT_CAP)},
                        "gradient": {"S": DEFAULT_GRADIENT},
                        "created_at": {"S": timestamp},
                        "created_by": {"S": user_id},
                    },
                }
            },
            {
                "Put": {
                    "TableName": table_name,
                    "Item": {
                        "pk": {"S": f"USER#{user_id}"},
                        "sk": {"S": f"GROUP#{group_id}"},
                        "gsi1pk": {"S": f"GROUP#{group_id}"},
                        "gsi1sk": {"S": f"MEMBER#{user_id}"},
                        "entity": {"S": "GroupMembership"},
                        "user_id": {"S": user_id},
                        "group_id": {"S": group_id},
                        "role": {"S": "admin"},
                        "joined_at": {"S": timestamp},
                        "editions_answered": {"N": "0"},
                    },
                }
            },
        ]
    )


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--env", default="dev", help="CDK environment name (default: dev)")
    parser.add_argument("--admin-email", required=True, help="Bootstrap admin email")
    parser.add_argument(
        "--admin-display-name",
        help="Display name for the admin (defaults to the local part of the email)",
    )
    parser.add_argument("--group-name", required=True, help="Name of the first group")
    parser.add_argument(
        "--timezone",
        default=DEFAULT_TIMEZONE,
        help=f"IANA time zone for the group (default: {DEFAULT_TIMEZONE})",
    )
    parser.add_argument(
        "--password",
        help="Permanent password for the admin. Prompted for if omitted.",
    )
    args = parser.parse_args()

    display_name = args.admin_display_name or args.admin_email.split("@", 1)[0]
    password = args.password or getpass.getpass(f"Password for {args.admin_email}: ")
    if not password:
        sys.exit("A password is required.")

    outputs = load_outputs(args.env)
    table_name = get_output(outputs, "DataStack", "TableName", args.env)
    user_pool_id = get_output(outputs, "AuthStack", "UserPoolId", args.env)
    bootstrap_client_id = get_output(
        outputs, "AuthStack", "UserPoolBootstrapClientId", args.env
    )

    session = boto3.Session()
    cognito = session.client("cognito-idp")
    dynamodb = session.client("dynamodb")

    print(f"Bootstrapping admin + group in '{args.env}'…")
    cognito_sub = ensure_cognito_user(
        cognito, user_pool_id, args.admin_email, display_name, password
    )
    ensure_cognito_admin_group(cognito, user_pool_id, args.admin_email)

    # Derived from the sub so re-running produces the same rows rather than a
    # second orphaned user.
    user_id = deterministic_id(f"user:{cognito_sub}")
    group_id = deterministic_id(f"group:{cognito_sub}:{args.group_name}")

    write_group(
        dynamodb,
        table_name,
        user_id=user_id,
        cognito_sub=cognito_sub,
        email=args.admin_email,
        display_name=display_name,
        group_id=group_id,
        group_name=args.group_name,
        tz=args.timezone,
        now=datetime.now(timezone.utc),
    )

    api_endpoint = outputs.get(f"ApiStack-{args.env}", {}).get("ApiEndpoint", "")

    print("\nDone.")
    print(f"  Group    : {args.group_name} ({group_id})")
    print(f"  Admin    : {args.admin_email} ({user_id})")
    print(f"  Table    : {table_name}")
    print("\nSign in for a token:")
    print(
        f"  aws cognito-idp initiate-auth --auth-flow USER_PASSWORD_AUTH \\\n"
        f"    --client-id {bootstrap_client_id} \\\n"
        f"    --auth-parameters USERNAME={args.admin_email},PASSWORD='<password>' \\\n"
        f"    --query 'AuthenticationResult.IdToken' --output text"
    )
    if api_endpoint:
        print(f"\nThen call the API (send the ID token):\n  curl -H \"Authorization: Bearer $TOKEN\" {api_endpoint}/me")
    else:
        print("\nDeploy ApiStack to get an endpoint for the /me smoke test.")

    # The next voting cycle is created by the lifecycle engine in M5; until then a
    # freshly bootstrapped group has no newsletter.
    print("\nNote: this group has no newsletter cycle yet — that lands with M5's cycle engine.")


if __name__ == "__main__":
    main()
