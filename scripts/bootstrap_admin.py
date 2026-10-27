#!/usr/bin/env python3
"""Create the very first group and its admin, without an invite code.

Invites can only be minted by an admin of an existing group, so the first group
has to come from outside the API. This script is that door. Dev and prod use
different flows (`docs/design/auth.md` §9):

dev  (§9.1)  — creates a password-auth Cognito user, then writes the user, the
               group, an admin membership and the first `voting` cycle.
               Idempotent: re-running with the same `--admin-email` and
               `--group-name` reuses the existing Cognito user and rewrites the
               same DynamoDB rows. A different `--group-name` creates an
               additional group with the same admin.
prod (§9.1a) — creates no Cognito user. Writes the group (`memberCount = 0`), its
               first `voting` cycle and an admin invite bound to
               BOOTSTRAP_ADMIN_EMAIL, then prints the `/join?code=…` URL. The
               operator redeems it by signing in with Google as that email.
               Refuses to overwrite an existing group of the same name.

Usage:
    python3 scripts/bootstrap_admin.py --env dev \\
        --admin-email me@example.com --group-name "Test"
    BOOTSTRAP_ADMIN_EMAIL=me@example.com \\
        python3 scripts/bootstrap_admin.py --env prod --group-name "Test"

Requires: boto3 installed, AWS credentials for the target account.
Reads:    infra/cdk.out/{env}-outputs.json (written by 'cdk deploy --outputs-file').
          In prod the file is optional: prod deploys from CI, so the script falls
          back to DataStack's fixed table name. BOOTSTRAP_ADMIN_EMAIL and
          ROOT_DOMAIN come from the environment or .env.local, through
          infra/opennewsletter/config.py.
"""

from __future__ import annotations

import argparse
import getpass
import json
import secrets
import sys
import uuid
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any, TypedDict
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

REPO_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(REPO_ROOT / "infra"))

# Resolved through the `sys.path` entry above at runtime. mypy only finds it
# when run from `infra/`, so the ignore must tolerate being unused there.
from opennewsletter.config import (  # type: ignore[import-not-found, unused-ignore]
    load_config,
)

# Mirrors AVATAR_COLOR_SLUGS + derive_avatar_color in
# backend/crates/shared/src/config.rs. Both sides must agree so a bootstrapped
# admin renders the same colour the API would have given them.
AVATAR_COLOR_SLUGS = [
    "red",
    "orange",
    "amber",
    "green",
    "teal",
    "blue",
    "violet",
    "pink",
]
DEFAULT_GRADIENT = "grape-sky"
DEFAULT_TIMEZONE = "America/New_York"
DEFAULT_CYCLE_SETTINGS = {
    "questions_per_cycle": 5,
    "votes_per_user_per_cycle": 3,
    "response_window_days": 7,
    "auto_publish": True,
}


class NotificationSettings(TypedDict):
    offsets_hours_before_close: list[int]
    on_cycle_open: bool


DEFAULT_NOTIFICATION_SETTINGS: NotificationSettings = {
    "offsets_hours_before_close": [96, 48, 24],
    "on_cycle_open": True,
}
DEFAULT_MEMBER_SOFT_CAP = 50

# Invite codes, mirroring backend/crates/lambda-invites/src/code.rs: Crockford
# base32 without I, L, O or U; 16 characters (INVITE_CODE_LEN); shown as
# hyphenated quads.
INVITE_CODE_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
INVITE_CODE_LEN = 16
# `ttlDays = 30` for the bootstrap invite (`docs/design/auth.md` §9.1a), and the row lingers
# INVITE_AUDIT_RETENTION_DAYS past expiry before DynamoDB TTL purges it
# (`docs/design/data-model.md` §2.4, backend/crates/shared/src/config.rs).
BOOTSTRAP_INVITE_TTL_DAYS = 30
INVITE_AUDIT_RETENTION_DAYS = 30
# `created_by` on the prod Group and Invite rows: there is no user yet, since
# the admin's user row is minted when they redeem the invite.
BOOTSTRAP_CREATOR = "bootstrap"


def derive_avatar_color(user_id: str) -> str:
    return AVATAR_COLOR_SLUGS[sum(user_id.encode()) % len(AVATAR_COLOR_SLUGS)]


def iso(dt: datetime) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


# --- Cycle schedule math, mirroring backend/crates/shared/src/cycle_time.rs ---
#
# Only the "very first cycle" case matters here (`after` is always "now"), so
# this doesn't need chrono-tz's ambiguous/nonexistent-local-midnight handling:
# `zoneinfo` never raises for those, it just resolves them per PEP 495, which
# is close enough for a bootstrap script that only ever runs against today's
# date in an active IANA zone.


def first_day_of_next_month_local(after_utc: datetime, tz: ZoneInfo) -> datetime:
    local = after_utc.astimezone(tz)
    year, month = (
        (local.year + 1, 1) if local.month == 12 else (local.year, local.month + 1)
    )
    local_midnight = datetime(year, month, 1, 0, 0, 0, tzinfo=tz)
    return local_midnight.astimezone(UTC)


def cycle_id_for(response_open_at: datetime, tz: ZoneInfo) -> str:
    """cycleId = yyyymm of responseOpenAt in the group's local time zone (§4.1)."""
    local = response_open_at.astimezone(tz)
    return f"{local.year:04d}{local.month:02d}"


def next_cycle_schedule(
    after: datetime, tz: ZoneInfo, response_window_days: int
) -> tuple[str, datetime, datetime]:
    """Returns (cycle_id, response_open_at, response_close_at) per §4.2/§5.4."""
    response_open_at = first_day_of_next_month_local(after, tz)
    response_close_at = response_open_at + timedelta(days=response_window_days)
    return cycle_id_for(response_open_at, tz), response_open_at, response_close_at


def deterministic_id(seed: str) -> str:
    """UUID5 under a private namespace — stable across runs for the same seed."""
    namespace = uuid.UUID("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
    return str(uuid.uuid5(namespace, seed))


def generate_invite_code() -> str:
    return "".join(secrets.choice(INVITE_CODE_ALPHABET) for _ in range(INVITE_CODE_LEN))


def format_invite_code(code: str) -> str:
    """`ABCDEFGHJKMNPQRS` -> `ABCD-EFGH-JKMN-PQRS`, as the API displays it."""
    return "-".join(code[i : i + 4] for i in range(0, len(code), 4))


def parse_timezone(tz_name: str) -> ZoneInfo:
    try:
        return ZoneInfo(tz_name)
    except ZoneInfoNotFoundError:
        sys.exit(f"`{tz_name}` is not a valid IANA time zone")


def load_outputs(env: str, *, required: bool = True) -> dict[str, Any]:
    path = REPO_ROOT / "infra" / "cdk.out" / f"{env}-outputs.json"
    if not path.exists():
        if not required:
            return {}
        sys.exit(
            f"Outputs file not found: {path}\n"
            f"Run 'cd infra && cdk deploy --context env={env} --all "
            f"--outputs-file cdk.out/{env}-outputs.json' first."
        )
    with path.open() as f:
        outputs: dict[str, Any] = json.load(f)
        return outputs


def get_output(outputs: dict[str, Any], stack: str, key: str, env: str) -> str:
    value = outputs.get(f"{stack}-{env}", {}).get(key, "")
    if not value:
        sys.exit(
            f"Missing CDK output {stack}-{env}.{key} — redeploy the {stack} stack."
        )
    return str(value)


def boto3_session() -> Any:
    # Imported here rather than at module top so the pure item builders below
    # can be unit-tested (infra/tests) without boto3 installed.
    try:
        import boto3  # type: ignore[import-untyped]
    except ImportError:
        sys.exit(
            "boto3 not installed. Run: infra/.venv/bin/pip install -r scripts/requirements.txt"
        )
    return boto3.Session()


# --- DynamoDB items (low-level AttributeValue form, for TransactWriteItems) ---


def group_meta_item(
    *,
    group_id: str,
    group_name: str,
    tz_name: str,
    member_count: int,
    created_by: str,
    timestamp: str,
) -> dict[str, Any]:
    """The Group META row (`docs/design/data-model.md` §2.3)."""
    return {
        "pk": {"S": f"GROUP#{group_id}"},
        "sk": {"S": "META"},
        "entity": {"S": "Group"},
        "group_id": {"S": group_id},
        "name": {"S": group_name},
        "timezone": {"S": tz_name},
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
                "auto_publish": {"BOOL": bool(DEFAULT_CYCLE_SETTINGS["auto_publish"])},
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
                    "BOOL": bool(DEFAULT_NOTIFICATION_SETTINGS["on_cycle_open"])
                },
            }
        },
        "member_count": {"N": str(member_count)},
        "member_soft_cap": {"N": str(DEFAULT_MEMBER_SOFT_CAP)},
        "gradient": {"S": DEFAULT_GRADIENT},
        "created_at": {"S": timestamp},
        "created_by": {"S": created_by},
    }


def first_newsletter_item(
    *, group_id: str, tz: ZoneInfo, now: datetime
) -> tuple[str, dict[str, Any]]:
    """The group's very first `voting` Newsletter (`docs/design/lifecycle.md` §4.2's "very first
    cycle" case — `after` is always "now" here). Returns (cycle_id, item)."""
    cycle_id, response_open_at, response_close_at = next_cycle_schedule(
        now, tz, DEFAULT_CYCLE_SETTINGS["response_window_days"]
    )
    response_open_at_iso = iso(response_open_at)
    return cycle_id, {
        "pk": {"S": f"GROUP#{group_id}"},
        "sk": {"S": f"NL#{cycle_id}"},
        "gsi2pk": {"S": "NL_STATUS#voting"},
        "gsi2sk": {"S": f"{response_open_at_iso}#{group_id}#{cycle_id}"},
        "entity": {"S": "Newsletter"},
        "group_id": {"S": group_id},
        "cycle_id": {"S": cycle_id},
        "status": {"S": "voting"},
        "vote_window_open_at": {"S": iso(now)},
        "vote_window_close_at": {"S": response_open_at_iso},
        "response_open_at": {"S": response_open_at_iso},
        "response_close_at": {"S": iso(response_close_at)},
        "published_at": {"NULL": True},
        "next_transition_at": {"S": response_open_at_iso},
        "locked_question_ids": {"L": []},
        "notified_offsets_hours": {"L": []},
        "notified_on_open": {"BOOL": False},
    }


def bound_admin_invite_item(
    *, code: str, group_id: str, bound_email: str, now: datetime
) -> dict[str, Any]:
    """An admin Invite bound to one email, in exactly the shape
    `persistence::invites::put_invite` writes (`docs/design/data-model.md` §2.4): the serde fields of
    `domain::Invite` plus pk/sk, the GSI1 keys, `entity` and `ttl`."""
    expires_at = now + timedelta(days=BOOTSTRAP_INVITE_TTL_DAYS)
    ttl = expires_at + timedelta(days=INVITE_AUDIT_RETENTION_DAYS)
    return {
        "pk": {"S": f"INVITE#{code}"},
        "sk": {"S": "META"},
        "gsi1pk": {"S": f"GROUP#{group_id}"},
        "gsi1sk": {"S": f"INVITE#{code}"},
        "entity": {"S": "Invite"},
        "code": {"S": code},
        "group_id": {"S": group_id},
        "created_by": {"S": BOOTSTRAP_CREATOR},
        "created_at": {"S": iso(now)},
        "expires_at": {"S": iso(expires_at)},
        "status": {"S": "pending"},
        "consumed_by": {"NULL": True},
        "consumed_at": {"NULL": True},
        "role_on_redeem": {"S": "admin"},
        "bound_email": {"S": bound_email.strip().lower()},
        "ttl": {"N": str(int(ttl.timestamp()))},
    }


def _put(table_name: str, item: dict[str, Any], *, new: bool) -> dict[str, Any]:
    put: dict[str, Any] = {"TableName": table_name, "Item": item}
    if new:
        put["ConditionExpression"] = "attribute_not_exists(pk)"
    return {"Put": put}


def prod_bootstrap_transact_items(
    *,
    table_name: str,
    group_id: str,
    group_name: str,
    tz_name: str,
    bound_email: str,
    invite_code: str,
    now: datetime,
) -> tuple[str, list[dict[str, Any]]]:
    """The one TransactWriteItems for `--env prod` (`docs/design/auth.md` §9.1a): Group META with
    `memberCount = 0`, the first `voting` Newsletter, and the bound admin
    Invite. Every put is conditional on the row not existing, so a re-run can
    never reset a live group's member count. Returns (cycle_id, items)."""
    timestamp = iso(now)
    cycle_id, newsletter = first_newsletter_item(
        group_id=group_id, tz=parse_timezone(tz_name), now=now
    )
    group = group_meta_item(
        group_id=group_id,
        group_name=group_name,
        tz_name=tz_name,
        member_count=0,
        created_by=BOOTSTRAP_CREATOR,
        timestamp=timestamp,
    )
    invite = bound_admin_invite_item(
        code=invite_code, group_id=group_id, bound_email=bound_email, now=now
    )
    return cycle_id, [
        _put(table_name, group, new=True),
        _put(table_name, newsletter, new=True),
        _put(table_name, invite, new=True),
    ]


# --- dev (§9.1) ---


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
        return str(sub)
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
    return str(sub)


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
    tz_name: str,
    now: datetime,
) -> str:
    """Write the User, sub lookup, Group, admin GroupMembership, and the
    group's very first `voting` Newsletter cycle, atomically.
    """
    timestamp = iso(now)
    cycle_id, newsletter = first_newsletter_item(
        group_id=group_id, tz=parse_timezone(tz_name), now=now
    )
    items: list[dict[str, Any]] = [
        {
            "pk": {"S": f"COGNITO_SUB#{cognito_sub}"},
            "sk": {"S": "USER_ID"},
            "entity": {"S": "CognitoSubLookup"},
            "cognito_sub": {"S": cognito_sub},
            "user_id": {"S": user_id},
        },
        {
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
        group_meta_item(
            group_id=group_id,
            group_name=group_name,
            tz_name=tz_name,
            member_count=1,
            created_by=user_id,
            timestamp=timestamp,
        ),
        {
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
        newsletter,
    ]
    dynamodb.transact_write_items(
        TransactItems=[_put(table_name, item, new=False) for item in items]
    )
    return cycle_id


def bootstrap_dev(args: argparse.Namespace) -> None:
    if not args.admin_email:
        sys.exit("--admin-email is required for --env dev.")
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

    session = boto3_session()
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

    cycle_id = write_group(
        dynamodb,
        table_name,
        user_id=user_id,
        cognito_sub=cognito_sub,
        email=args.admin_email,
        display_name=display_name,
        group_id=group_id,
        group_name=args.group_name,
        tz_name=args.timezone,
        now=datetime.now(UTC),
    )

    api_endpoint = outputs.get(f"ApiStack-{args.env}", {}).get("ApiEndpoint", "")

    print("\nDone.")
    print(f"  Group    : {args.group_name} ({group_id})")
    print(f"  Admin    : {args.admin_email} ({user_id})")
    print(f"  Cycle    : {cycle_id} (voting)")
    print(f"  Table    : {table_name}")
    print("\nSign in for a token:")
    print(
        f"  aws cognito-idp initiate-auth --auth-flow USER_PASSWORD_AUTH \\\n"
        f"    --client-id {bootstrap_client_id} \\\n"
        f"    --auth-parameters USERNAME={args.admin_email},PASSWORD='<password>' \\\n"
        f"    --query 'AuthenticationResult.IdToken' --output text"
    )
    if api_endpoint:
        print(
            f'\nThen call the API (send the ID token):\n  curl -H "Authorization: Bearer $TOKEN" {api_endpoint}/me'
        )
    else:
        print("\nDeploy ApiStack to get an endpoint for the /me smoke test.")


# --- prod (§9.1a) ---


def bootstrap_prod(args: argparse.Namespace) -> None:
    config = load_config("prod")
    if not config.bootstrap_admin_email:
        sys.exit(
            "BOOTSTRAP_ADMIN_EMAIL is not set. Set it (environment or .env.local) "
            "to the email the prod admin will sign in with via Google."
        )
    bound_email = config.bootstrap_admin_email.strip().lower()
    # The email comes from config, not the flag (`docs/design/auth.md` §9.1a); the flag is only
    # accepted as a cross-check.
    if args.admin_email and args.admin_email.strip().lower() != bound_email:
        sys.exit(
            f"--admin-email {args.admin_email} does not match "
            f"BOOTSTRAP_ADMIN_EMAIL {config.bootstrap_admin_email}."
        )
    if args.password or args.admin_display_name:
        print(
            "Note: --password and --admin-display-name are ignored in prod; "
            "the admin's profile comes from Google when they redeem the invite."
        )

    outputs = load_outputs(args.env, required=False)
    # DataStack names the table explicitly, so the fallback can't drift.
    table_name = str(
        outputs.get(f"DataStack-{args.env}", {}).get(
            "TableName", f"OpenNewsletter-{args.env}"
        )
    )
    # Keyed on the bound email + group name, so re-running for the same group
    # hits the existence conditions instead of creating a duplicate.
    group_id = deterministic_id(f"group:{bound_email}:{args.group_name}")
    invite_code = generate_invite_code()

    cycle_id, items = prod_bootstrap_transact_items(
        table_name=table_name,
        group_id=group_id,
        group_name=args.group_name,
        tz_name=args.timezone,
        bound_email=bound_email,
        invite_code=invite_code,
        now=datetime.now(UTC),
    )

    dynamodb = boto3_session().client("dynamodb")
    print(f"Bootstrapping group + bound admin invite in '{args.env}'…")
    try:
        dynamodb.transact_write_items(TransactItems=items)
    except dynamodb.exceptions.TransactionCanceledException as err:
        sys.exit(
            f"Transaction cancelled — group '{args.group_name}' ({group_id}) "
            f"probably exists already, so nothing was written.\n{err}"
        )

    join_url = f"https://{config.domain}/join?code={format_invite_code(invite_code)}"
    print("\nDone.")
    print(f"  Group    : {args.group_name} ({group_id})")
    print(f"  Cycle    : {cycle_id} (voting)")
    print(
        f"  Invite   : admin, bound to {bound_email}, "
        f"expires in {BOOTSTRAP_INVITE_TTL_DAYS} days"
    )
    print(f"  Table    : {table_name}")
    print(f"\nOpen this and sign in with Google as {bound_email}:")
    print(f"  {join_url}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--env",
        default="dev",
        choices=["dev", "prod"],
        help="CDK environment name (default: dev)",
    )
    parser.add_argument(
        "--admin-email",
        help="dev: bootstrap admin email (required). "
        "prod: optional cross-check against BOOTSTRAP_ADMIN_EMAIL.",
    )
    parser.add_argument(
        "--admin-display-name",
        help="dev only: display name for the admin "
        "(defaults to the local part of the email)",
    )
    parser.add_argument("--group-name", required=True, help="Name of the first group")
    parser.add_argument(
        "--timezone",
        default=DEFAULT_TIMEZONE,
        help=f"IANA time zone for the group (default: {DEFAULT_TIMEZONE})",
    )
    parser.add_argument(
        "--password",
        help="dev only: permanent password for the admin. Prompted for if omitted.",
    )
    args = parser.parse_args()

    if args.env == "prod":
        bootstrap_prod(args)
    else:
        bootstrap_dev(args)


if __name__ == "__main__":
    main()
