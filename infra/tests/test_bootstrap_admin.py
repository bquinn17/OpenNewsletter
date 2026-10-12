"""Unit tests for `scripts/bootstrap_admin.py`'s DynamoDB item builders.

Covers prod mode (`plans/05-auth-flow.md` §9.1a): the one TransactWriteItems
with the Group, first Newsletter and email-bound admin Invite. Nothing here
touches AWS; the builders are pure, and boto3 is imported only inside the
script's `main` path.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import argparse
import dataclasses
import importlib.util
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import ModuleType
from typing import Any

import pytest

_SCRIPT = Path(__file__).parent.parent.parent / "scripts" / "bootstrap_admin.py"

# The serde fields of `domain::Invite` (backend/crates/domain/src/entities.rs)
# plus the attributes `persistence::invites::put_invite` adds. The bootstrap
# invite must have exactly these, or the backend can't read it back.
_INVITE_ATTRIBUTES = {
    "pk",
    "sk",
    "gsi1pk",
    "gsi1sk",
    "entity",
    "ttl",
    "code",
    "group_id",
    "created_by",
    "created_at",
    "expires_at",
    "status",
    "consumed_by",
    "consumed_at",
    "role_on_redeem",
    "bound_email",
}

_NOW = datetime(2026, 10, 7, 15, 30, 45, tzinfo=timezone.utc)
_GROUP_ID = "0b6f3c3e-1111-5222-8333-444455556666"
_CODE = "ABCDEFGHJKMNPQRS"


@pytest.fixture(scope="module")
def script() -> ModuleType:
    spec = importlib.util.spec_from_file_location("bootstrap_admin", _SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["bootstrap_admin"] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def prod_items(script: ModuleType) -> tuple[str, list[dict[str, Any]]]:
    cycle_id, items = script.prod_bootstrap_transact_items(
        table_name="OpenNewsletter-prod",
        group_id=_GROUP_ID,
        group_name="Founders",
        tz_name="America/New_York",
        bound_email="  Admin@Example.COM ",
        invite_code=_CODE,
        now=_NOW,
    )
    assert isinstance(cycle_id, str)
    return cycle_id, items


def _item(items: list[dict[str, Any]], entity: str) -> dict[str, Any]:
    matches: list[dict[str, Any]] = [
        put["Put"]["Item"]
        for put in items
        if put["Put"]["Item"]["entity"]["S"] == entity
    ]
    (item,) = matches
    return item


def test_prod_writes_group_newsletter_and_invite_conditionally(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    _, items = prod_items
    assert [put["Put"]["Item"]["entity"]["S"] for put in items] == [
        "Group",
        "Newsletter",
        "Invite",
    ]
    for put in items:
        assert put["Put"]["TableName"] == "OpenNewsletter-prod"
        # A re-run must never overwrite a live group's member count.
        assert put["Put"]["ConditionExpression"] == "attribute_not_exists(pk)"


def test_prod_group_has_no_members_yet(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    group = _item(prod_items[1], "Group")
    assert group["pk"] == {"S": f"GROUP#{_GROUP_ID}"}
    assert group["sk"] == {"S": "META"}
    assert group["member_count"] == {"N": "0"}
    assert group["created_by"] == {"S": "bootstrap"}
    assert group["cycle_settings"]["M"]["response_window_days"] == {"N": "7"}


def test_prod_first_cycle_is_voting(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    cycle_id, items = prod_items
    newsletter = _item(items, "Newsletter")
    # Next month's 1st in New York is 2026-11-01T04:00:00Z.
    assert cycle_id == "202611"
    assert newsletter["sk"] == {"S": "NL#202611"}
    assert newsletter["status"] == {"S": "voting"}
    assert newsletter["gsi2pk"] == {"S": "NL_STATUS#voting"}
    assert newsletter["gsi2sk"] == {"S": f"2026-11-01T04:00:00Z#{_GROUP_ID}#202611"}
    assert newsletter["response_close_at"] == {"S": "2026-11-08T04:00:00Z"}


def test_prod_invite_matches_the_backend_row_shape(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    invite = _item(prod_items[1], "Invite")
    assert set(invite) == _INVITE_ATTRIBUTES
    assert invite["pk"] == {"S": f"INVITE#{_CODE}"}
    assert invite["sk"] == {"S": "META"}
    assert invite["gsi1pk"] == {"S": f"GROUP#{_GROUP_ID}"}
    assert invite["gsi1sk"] == {"S": f"INVITE#{_CODE}"}
    assert invite["code"] == {"S": _CODE}
    assert invite["group_id"] == {"S": _GROUP_ID}
    assert invite["status"] == {"S": "pending"}
    assert invite["role_on_redeem"] == {"S": "admin"}
    assert invite["consumed_by"] == {"NULL": True}
    assert invite["consumed_at"] == {"NULL": True}


def test_prod_invite_is_bound_to_the_lowercased_email(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    invite = _item(prod_items[1], "Invite")
    assert invite["bound_email"] == {"S": "admin@example.com"}


def test_prod_invite_expires_in_30_days_and_ttl_adds_the_audit_window(
    prod_items: tuple[str, list[dict[str, Any]]],
) -> None:
    invite = _item(prod_items[1], "Invite")
    assert invite["created_at"] == {"S": "2026-10-07T15:30:45Z"}
    assert invite["expires_at"] == {"S": "2026-11-06T15:30:45Z"}
    ttl = _NOW + timedelta(days=30 + 30)
    assert invite["ttl"] == {"N": str(int(ttl.timestamp()))}


def test_invite_codes_use_the_backend_alphabet(script: ModuleType) -> None:
    code = script.generate_invite_code()
    assert len(code) == 16
    assert set(code) <= set("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
    assert script.format_invite_code(_CODE) == "ABCD-EFGH-JKMN-PQRS"


def test_dev_group_still_starts_with_its_admin(script: ModuleType) -> None:
    captured: list[dict[str, Any]] = []

    class FakeDynamo:
        def transact_write_items(self, TransactItems: list[dict[str, Any]]) -> None:
            captured.extend(TransactItems)

    script.write_group(
        FakeDynamo(),
        "OpenNewsletter-dev",
        user_id="u1",
        cognito_sub="sub1",
        email="a@example.com",
        display_name="A",
        group_id=_GROUP_ID,
        group_name="Dev",
        tz_name="America/New_York",
        now=_NOW,
    )
    entities = [put["Put"]["Item"]["entity"]["S"] for put in captured]
    assert entities == [
        "CognitoSubLookup",
        "User",
        "Group",
        "GroupMembership",
        "Newsletter",
    ]
    group = _item(captured, "Group")
    assert group["member_count"] == {"N": "1"}
    assert group["created_by"] == {"S": "u1"}
    # Dev keeps its unconditional, idempotent rewrite.
    assert all("ConditionExpression" not in put["Put"] for put in captured)


def _prod_args(**overrides: Any) -> argparse.Namespace:
    values: dict[str, Any] = {
        "env": "prod",
        "admin_email": None,
        "admin_display_name": None,
        "group_name": "Founders",
        "timezone": "America/New_York",
        "password": None,
    }
    values.update(overrides)
    return argparse.Namespace(**values)


def test_prod_refuses_without_bootstrap_admin_email(
    script: ModuleType, monkeypatch: pytest.MonkeyPatch
) -> None:
    config = dataclasses.replace(script.load_config("prod"), bootstrap_admin_email=None)
    monkeypatch.setattr(script, "load_config", lambda env: config)
    with pytest.raises(SystemExit, match="BOOTSTRAP_ADMIN_EMAIL is not set"):
        script.bootstrap_prod(_prod_args())


def test_prod_refuses_a_mismatched_admin_email_flag(
    script: ModuleType, monkeypatch: pytest.MonkeyPatch
) -> None:
    config = dataclasses.replace(
        script.load_config("prod"), bootstrap_admin_email="admin@example.com"
    )
    monkeypatch.setattr(script, "load_config", lambda env: config)
    with pytest.raises(SystemExit, match="does not match"):
        script.bootstrap_prod(_prod_args(admin_email="someone@else.com"))
