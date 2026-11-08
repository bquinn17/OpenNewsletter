#!/usr/bin/env python3
"""Seed the dev DynamoDB table and S3 buckets with fixture data.

Destructive + idempotent: scans and batch-deletes the DynamoDB table, empties
the originals and processed S3 buckets, then writes a small deterministic
fixture (one group, the bootstrap admin as sole member).

Cognito users created by the real invite/signup flow survive across seed runs —
only the admin's Cognito user is managed here (created on first run, reused
thereafter). Its password is reset on every run, to a fresh random one unless
--admin-password is given, and printed at the end.

The first `voting` cycle is created by the next cycle tick (or immediately via
`POST /admin/dev/tick/cycle`).

Usage:
    python3 scripts/seed_dev_data.py [--env dev]
        [--admin-email EMAIL] [--admin-password PASSWORD]
        [--admin-sub SUB]        # skip Cognito creation, use this sub directly

Requires: boto3 installed, AWS credentials configured for the dev account.
Reads:    infra/cdk.out/{env}-outputs.json  (written by 'cdk deploy --outputs-file').
"""

from __future__ import annotations

import argparse
import json
import secrets
import string
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Not `datetime.UTC`: that needs Python 3.11, and the WSL2 venv runs 3.8.
UTC = timezone.utc

try:
    import boto3  # type: ignore[import-untyped]
    from botocore.exceptions import ClientError  # type: ignore[import-untyped]
except ImportError:
    sys.exit(
        "boto3 not installed. Run: infra/.venv/bin/pip install -r scripts/requirements.txt"
    )

REPO_ROOT = Path(__file__).parent.parent


def load_outputs(env: str) -> dict[str, Any]:
    path = REPO_ROOT / "infra" / "cdk.out" / f"{env}-outputs.json"
    if not path.exists():
        sys.exit(
            f"Outputs file not found: {path}\n"
            "Run 'make deploy-dev' (or 'cd infra && cdk deploy --context env=dev "
            "--all --outputs-file cdk.out/dev-outputs.json') first."
        )
    with path.open() as f:
        outputs: dict[str, Any] = json.load(f)
    return outputs


def get_output(outputs: dict[str, Any], stack: str, key: str, env: str) -> str:
    full_name = f"{stack}-{env}"
    val = outputs.get(full_name, {}).get(key, "")
    if not val:
        sys.exit(f"Missing CDK output {full_name}.{key} — run 'make deploy-dev' first.")
    return str(val)


def iso(dt: datetime) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def deterministic_id(seed: str) -> str:
    """UUID5 under a private namespace — stable across seed runs for the same seed string."""
    _NS = uuid.UUID("6ba7b812-9dad-11d1-80b4-00c04fd430c8")
    return str(uuid.uuid5(_NS, seed))


# ---------------------------------------------------------------------------
# Data clearing
# ---------------------------------------------------------------------------


def clear_table(table: Any) -> int:
    """Scan + batch-delete every item in the DynamoDB table. Returns count deleted."""
    key_names = [k["AttributeName"] for k in table.key_schema]
    proj = ", ".join(f"#k{i}" for i in range(len(key_names)))
    attr_names = {f"#k{i}": key_names[i] for i in range(len(key_names))}
    deleted = 0
    scan_kwargs: dict[str, Any] = {
        "ProjectionExpression": proj,
        "ExpressionAttributeNames": attr_names,
    }
    while True:
        resp = table.scan(**scan_kwargs)
        items = resp.get("Items", [])
        if items:
            with table.batch_writer() as batch:
                for item in items:
                    batch.delete_item(Key={k: item[k] for k in key_names})
            deleted += len(items)
        if "LastEvaluatedKey" not in resp:
            break
        scan_kwargs["ExclusiveStartKey"] = resp["LastEvaluatedKey"]
    return deleted


def clear_bucket(s3_resource: Any, bucket_name: str) -> int:
    """Delete every object (and version) from a bucket. Returns count deleted."""
    bucket = s3_resource.Bucket(bucket_name)
    deleted = 0
    try:
        # Versioned bucket: delete all versions + delete markers
        versions = list(bucket.object_versions.all())
        if versions:
            bucket.delete_objects(
                Delete={
                    "Objects": [{"Key": v.key, "VersionId": v.id} for v in versions]
                }
            )
            deleted += len(versions)
    except ClientError:
        # Non-versioned bucket
        objects = list(bucket.objects.all())
        if objects:
            bucket.delete_objects(Delete={"Objects": [{"Key": o.key} for o in objects]})
            deleted += len(objects)
    return deleted


# ---------------------------------------------------------------------------
# Cognito bootstrap
# ---------------------------------------------------------------------------


# Shell-safe, so the printed CLI command works unquoted; Cognito accepts all four.
PASSWORD_SYMBOLS = "-_.%"


def random_password(length: int = 20) -> str:
    """A password meeting the dev pool's policy (auth_stack.py): 12+ chars with
    lowercase, uppercase, a digit and a symbol."""
    classes = [
        string.ascii_lowercase,
        string.ascii_uppercase,
        string.digits,
        PASSWORD_SYMBOLS,
    ]
    alphabet = "".join(classes)
    chars = [secrets.choice(c) for c in classes]
    chars += [secrets.choice(alphabet) for _ in range(length - len(chars))]
    secrets.SystemRandom().shuffle(chars)
    return "".join(chars)


def ensure_cognito_user(
    cognito: Any, user_pool_id: str, email: str, password: str
) -> str:
    """Create the bootstrap admin if missing, then set its password.

    Returns the Cognito sub (used to derive the internal user_id). An existing
    user is reused, never recreated, so its sub and the DynamoDB rows derived
    from it stay stable across seed runs.
    """
    try:
        cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
        created = False
    except cognito.exceptions.UserNotFoundException:
        cognito.admin_create_user(
            UserPoolId=user_pool_id,
            Username=email,
            TemporaryPassword=random_password(),
            MessageAction="SUPPRESS",
            UserAttributes=[
                {"Name": "email", "Value": email},
                {"Name": "email_verified", "Value": "true"},
                {"Name": "name", "Value": "Dev Admin"},
            ],
        )
        created = True

    # Permanent, so the first sign-in doesn't hit FORCE_CHANGE_PASSWORD.
    cognito.admin_set_user_password(
        UserPoolId=user_pool_id,
        Username=email,
        Password=password,
        Permanent=True,
    )
    resp = cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
    sub: str = next(a["Value"] for a in resp["UserAttributes"] if a["Name"] == "sub")
    verb = "Created" if created else "Reset password for existing"
    print(f"  {verb} Cognito user — {email} (sub={sub[:8]}…)")
    return sub


# ---------------------------------------------------------------------------
# DynamoDB fixture
# ---------------------------------------------------------------------------


def write_fixture(
    table: Any,
    *,
    user_id: str,
    cognito_sub: str,
    admin_email: str,
    group_id: str,
    now: datetime,
) -> None:
    """Write the fixture group and admin membership.

    The first `voting` cycle is created by the next cycle tick (or immediately
    via the dev tick route).
    """
    items = [
        # --- Cognito sub → internal user_id lookup (AP1 reverse) ---
        {
            "pk": f"COGNITO_SUB#{cognito_sub}",
            "sk": "USER_ID",
            "entity": "CognitoSubLookup",
            "cognito_sub": cognito_sub,
            "user_id": user_id,
        },
        # --- User profile (AP1) ---
        {
            "pk": f"USER#{user_id}",
            "sk": "PROFILE",
            "entity": "User",
            "user_id": user_id,
            "cognito_sub": cognito_sub,
            "email": admin_email,
            "display_name": "Dev Admin",
            "avatar_color": "blue",
            "created_at": iso(now),
            "last_login_at": iso(now),
        },
        # --- Group metadata (AP4) ---
        # Marked as awaiting its first cycle, which is created by the cycle tick Lambda.
        {
            "pk": f"GROUP#{group_id}",
            "sk": "META",
            "gsi2pk": "GROUP_STATUS#awaiting_first_cycle",
            "gsi2sk": f"{iso(now)}#{group_id}",
            "entity": "Group",
            "group_id": group_id,
            "name": "Dev Group",
            "timezone": "America/New_York",
            "cycle_settings": {
                "questions_per_cycle": 5,
                "votes_per_user_per_cycle": 3,
                "response_window_days": 7,
                "auto_publish": True,
            },
            "notification_settings": {
                "offsets_hours_before_close": [96, 48, 24],
                "on_cycle_open": True,
            },
            "member_count": 1,
            "member_soft_cap": 50,
            "gradient": "grape-sky",
            "created_at": iso(now),
            "created_by": user_id,
        },
        # --- GroupMembership (AP2 — stored under the user partition) ---
        {
            "pk": f"USER#{user_id}",
            "sk": f"GROUP#{group_id}",
            "gsi1pk": f"GROUP#{group_id}",
            "gsi1sk": f"MEMBER#{user_id}",
            "entity": "GroupMembership",
            "user_id": user_id,
            "group_id": group_id,
            "role": "admin",
            "joined_at": iso(now),
            "editions_answered": 0,
        },
    ]

    with table.batch_writer() as batch:
        for item in items:
            batch.put_item(Item=item)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--env", default="dev", help="CDK environment name (default: dev)"
    )
    parser.add_argument(
        "--admin-email", default="dev-admin@example.com", help="Bootstrap admin email"
    )
    parser.add_argument(
        "--admin-password",
        help="Bootstrap admin password (default: a fresh random 20-character one)",
    )
    parser.add_argument(
        "--admin-sub",
        help="Cognito sub to use directly (skip Cognito user creation/lookup)",
    )
    args = parser.parse_args()
    password = args.admin_password or random_password()
    # With --admin-sub, Cognito is untouched, so `password` was never set on anyone.
    shown_password = "EXISTING_PASSWORD" if args.admin_sub else password

    outputs = load_outputs(args.env)
    table_name = get_output(outputs, "DataStack", "TableName", args.env)
    user_pool_id = get_output(outputs, "AuthStack", "UserPoolId", args.env)
    user_pool_bootstrap_client = get_output(
        outputs, "AuthStack", "UserPoolBootstrapClientId", args.env
    )
    originals_bucket = get_output(
        outputs, "MediaPersistentStack", "OriginalsBucketName", args.env
    )
    processed_bucket = get_output(
        outputs, "MediaPersistentStack", "ProcessedBucketName", args.env
    )

    region = "us-east-1"
    ddb = boto3.resource("dynamodb", region_name=region)
    s3 = boto3.resource("s3", region_name=region)
    cognito = boto3.client("cognito-idp", region_name=region)

    table = ddb.Table(table_name)

    print(f"\n=== Seeding '{args.env}' environment ===")
    print(f"  Table:    {table_name}")
    print(f"  Buckets:  {originals_bucket}, {processed_bucket}")

    print("\n[1/5] Clearing DynamoDB table…")
    n = clear_table(table)
    print(f"  Deleted {n} items.")

    print("[2/5] Clearing S3 originals bucket…")
    n = clear_bucket(s3, originals_bucket)
    print(f"  Deleted {n} objects.")

    print("[3/5] Clearing S3 processed bucket…")
    n = clear_bucket(s3, processed_bucket)
    print(f"  Deleted {n} objects.")

    print("[4/5] Ensuring bootstrap admin Cognito user…")
    if args.admin_sub:
        cognito_sub = args.admin_sub
        print(f"  Using provided sub: {cognito_sub[:8]}…")
    else:
        cognito_sub = ensure_cognito_user(
            cognito, user_pool_id, args.admin_email, password
        )

    print("[5/5] Writing fixture data…")
    now = datetime.now(UTC)

    # Derive stable IDs from known seeds so re-running seed gives the same IDs.
    user_id = deterministic_id(f"seed:user:{cognito_sub}")
    group_id = deterministic_id("seed:dev-group")

    write_fixture(
        table,
        user_id=user_id,
        cognito_sub=cognito_sub,
        admin_email=args.admin_email,
        group_id=group_id,
        now=now,
    )

    print(
        f"""
=== Fixture written ===
  User ID:   {user_id}
  Group ID:  {group_id}

The first `voting` cycle appears on the next cycle tick (within 5 minutes),
or immediately via: POST /admin/dev/tick/cycle

=== Sign-in credentials (bootstrap admin) ===
  Email:     {args.admin_email}
  Password:  {password if not args.admin_sub else "(unchanged: --admin-sub skips Cognito)"}

  Sign in via the /admin/bootstrap-login page (VITE_ENV=dev),
  or use the AWS CLI:

    aws cognito-idp initiate-auth \\
      --client-id {user_pool_bootstrap_client} \\
      --auth-flow USER_PASSWORD_AUTH \\
      --auth-parameters USERNAME={args.admin_email},PASSWORD={shown_password} \\
      --query AuthenticationResult.IdToken --output text
"""
    )


if __name__ == "__main__":
    main()
