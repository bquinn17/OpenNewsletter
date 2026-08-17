#!/usr/bin/env python3
"""Seed the dev DynamoDB table and S3 buckets with fixture data.

Destructive + idempotent: scans and batch-deletes the DynamoDB table, empties
the originals and processed S3 buckets, then writes a small deterministic
fixture (one group, the bootstrap admin as sole member, one voting cycle).

Cognito users created by the real invite/signup flow survive across seed runs —
only the admin's Cognito user is managed here (created on first run, reused
thereafter).

Usage:
    python3 scripts/seed_dev_data.py [--env dev]
        [--admin-email EMAIL] [--admin-password PASSWORD]
        [--admin-sub SUB]        # skip Cognito creation, use this sub directly
        [--cycle-close-in DUR]   # duration until response window closes, e.g. '5m', '2h', '4d'

Requires: boto3 installed, AWS credentials configured for the dev account.
Reads:    infra/cdk.out/{env}-outputs.json  (written by 'cdk deploy --outputs-file').
"""

from __future__ import annotations

import argparse
import json
import sys
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

try:
    import boto3
    from botocore.exceptions import ClientError
except ImportError:
    sys.exit("boto3 not installed. Run: pip install boto3")

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
        return json.load(f)


def get_output(outputs: dict[str, Any], stack: str, key: str, env: str) -> str:
    full_name = f"{stack}-{env}"
    val = outputs.get(full_name, {}).get(key, "")
    if not val:
        sys.exit(
            f"Missing CDK output {full_name}.{key} — run 'make deploy-dev' first."
        )
    return val


def parse_duration(s: str) -> timedelta:
    """Parse a compact duration string like '5m', '2h', '4d' into a timedelta."""
    unit_map = {"m": "minutes", "h": "hours", "d": "days"}
    if not s or s[-1] not in unit_map:
        sys.exit(f"Invalid duration '{s}': expected a number followed by m/h/d (e.g. '5m', '2h', '4d')")
    try:
        n = int(s[:-1])
    except ValueError:
        sys.exit(f"Invalid duration '{s}': the numeric part must be an integer")
    return timedelta(**{unit_map[s[-1]]: n})


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
                Delete={"Objects": [{"Key": v.key, "VersionId": v.id} for v in versions]}
            )
            deleted += len(versions)
    except ClientError:
        # Non-versioned bucket
        objects = list(bucket.objects.all())
        if objects:
            bucket.delete_objects(
                Delete={"Objects": [{"Key": o.key} for o in objects]}
            )
            deleted += len(objects)
    return deleted


# ---------------------------------------------------------------------------
# Cognito bootstrap
# ---------------------------------------------------------------------------

def ensure_cognito_user(cognito: Any, user_pool_id: str, email: str, password: str) -> str:
    """Idempotently create the bootstrap admin in the Cognito user pool.

    Returns the Cognito sub (used to derive the internal user_id).
    Does NOT reset the password if the user already exists — callers rely on
    the stable sub to keep DynamoDB rows consistent across seed runs.
    """
    try:
        resp = cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
        sub = next(a["Value"] for a in resp["UserAttributes"] if a["Name"] == "sub")
        print(f"  Cognito user already exists — {email} (sub={sub[:8]}…)")
        return sub
    except cognito.exceptions.UserNotFoundException:
        pass

    # Create the user and immediately set a permanent password to avoid FORCE_CHANGE.
    cognito.admin_create_user(
        UserPoolId=user_pool_id,
        Username=email,
        TemporaryPassword=f"{password}_tmp",
        MessageAction="SUPPRESS",
        UserAttributes=[
            {"Name": "email", "Value": email},
            {"Name": "email_verified", "Value": "true"},
            {"Name": "name", "Value": "Dev Admin"},
        ],
    )
    cognito.admin_set_user_password(
        UserPoolId=user_pool_id,
        Username=email,
        Password=password,
        Permanent=True,
    )
    resp = cognito.admin_get_user(UserPoolId=user_pool_id, Username=email)
    sub = next(a["Value"] for a in resp["UserAttributes"] if a["Name"] == "sub")
    print(f"  Created Cognito user — {email} (sub={sub[:8]}…)")
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
    cycle_id: str,
    now: datetime,
    vote_close: datetime,
    response_close: datetime,
) -> None:
    response_open = vote_close
    gsi2sk = f"{iso(vote_close)}#{group_id}#{cycle_id}"

    items = [
        # --- Cognito sub → internal user_id lookup (AP1 reverse) ---
        {
            "pk": f"COGNITO_SUB#{cognito_sub}",
            "sk": "USER_ID",
            "entity": "CognitoSubLookup",
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
        {
            "pk": f"GROUP#{group_id}",
            "sk": "META",
            "entity": "Group",
            "group_id": group_id,
            "name": "Dev Group",
            "timezone": "America/New_York",
            "cycle_settings": {
                "questions_per_cycle": 5,
                "votes_per_user_per_cycle": 3,
                "response_window_days": 4,
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
        # --- Newsletter in voting status (AP7/AP8) ---
        {
            "pk": f"GROUP#{group_id}",
            "sk": f"NL#{cycle_id}",
            "gsi2pk": "NL_STATUS#voting",
            "gsi2sk": gsi2sk,
            "entity": "Newsletter",
            "group_id": group_id,
            "cycle_id": cycle_id,
            "status": "voting",
            "vote_window_open_at": iso(now),
            "vote_window_close_at": iso(vote_close),
            "response_open_at": iso(response_open),
            "response_close_at": iso(response_close),
            "locked_question_ids": [],
            "notified_offsets_hours": [],
            "notified_on_open": False,
        },
    ]

    with table.batch_writer() as batch:
        for item in items:
            batch.put_item(Item=item)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--env", default="dev", help="CDK environment name (default: dev)")
    parser.add_argument("--admin-email", default="dev-admin@example.com", help="Bootstrap admin email")
    parser.add_argument("--admin-password", default="DevAdmin123!", help="Bootstrap admin password")
    parser.add_argument(
        "--admin-sub",
        help="Cognito sub to use directly (skip Cognito user creation/lookup)",
    )
    parser.add_argument(
        "--cycle-close-in",
        default="4d",
        metavar="DUR",
        help="Time until the response window closes, relative to now (e.g. '5m', '2h', '4d'). Default: 4d",
    )
    args = parser.parse_args()

    outputs = load_outputs(args.env)
    table_name = get_output(outputs, "DataStack", "TableName", args.env)
    user_pool_id = get_output(outputs, "AuthStack", "UserPoolId", args.env)
    user_pool_bootstrap_client = get_output(outputs, "AuthStack", "UserPoolBootstrapClientId", args.env)
    originals_bucket = get_output(outputs, "MediaPersistentStack", "OriginalsBucketName", args.env)
    processed_bucket = get_output(outputs, "MediaPersistentStack", "ProcessedBucketName", args.env)

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
            cognito, user_pool_id, args.admin_email, args.admin_password
        )

    print("[5/5] Writing fixture data…")
    now = datetime.now(timezone.utc)
    vote_close = now + timedelta(days=27)          # ~1-month voting window
    response_close = vote_close + parse_duration(args.cycle_close_in)

    # Derive stable IDs from known seeds so re-running seed gives the same IDs.
    user_id = deterministic_id(f"seed:user:{cognito_sub}")
    group_id = deterministic_id("seed:dev-group")
    cycle_id = deterministic_id(f"seed:cycle:{now.strftime('%Y%m')}")

    write_fixture(
        table,
        user_id=user_id,
        cognito_sub=cognito_sub,
        admin_email=args.admin_email,
        group_id=group_id,
        cycle_id=cycle_id,
        now=now,
        vote_close=vote_close,
        response_close=response_close,
    )

    print(
        f"""
=== Fixture written ===
  User ID:   {user_id}
  Group ID:  {group_id}
  Cycle ID:  {cycle_id}

=== Sign-in credentials (bootstrap admin) ===
  Email:     {args.admin_email}
  Password:  {args.admin_password}

  Sign in via the /admin/bootstrap-login page (VITE_ENV=dev),
  or use the AWS CLI:

    aws cognito-idp admin-initiate-auth \\
      --user-pool-id {user_pool_id} \\
      --client-id {user_pool_bootstrap_client} \\
      --auth-flow ADMIN_USER_PASSWORD_AUTH \\
      --auth-parameters USERNAME={args.admin_email},PASSWORD={args.admin_password}
"""
    )


if __name__ == "__main__":
    main()
