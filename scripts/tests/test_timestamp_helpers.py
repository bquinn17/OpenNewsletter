"""Unit tests for timestamp helper functions in scripts.

Tests the RFC 3339 formatting for both key attributes (whole seconds) and
data attributes (milliseconds, truncated).

Run with: pytest scripts/tests
"""

from __future__ import annotations

import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from bootstrap_admin import iso_data as bootstrap_iso_data
from bootstrap_admin import iso_key as bootstrap_iso_key
from seed_dev_data import iso_data as seed_iso_data
from seed_dev_data import iso_key as seed_iso_key

# Not `datetime.UTC`: that needs Python 3.11, and the WSL2 venv runs 3.8.
UTC = timezone.utc


def test_key_format_whole_seconds() -> None:
    """Key attributes use RFC 3339 with whole seconds only."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 123456, tzinfo=UTC)
    # Microseconds are ignored in key format
    assert seed_iso_key(dt) == "2026-11-15T10:40:13Z"
    assert bootstrap_iso_key(dt) == "2026-11-15T10:40:13Z"


def test_data_format_milliseconds_truncated() -> None:
    """Data attributes use RFC 3339 with milliseconds (truncated, not rounded)."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 123456, tzinfo=UTC)
    # 123456 microseconds = 123 milliseconds (truncated, not 124)
    assert seed_iso_data(dt) == "2026-11-15T10:40:13.123Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.123Z"


def test_data_format_whole_second_has_000_millis() -> None:
    """Data attributes for a whole second timestamp show .000Z."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 0, tzinfo=UTC)
    assert seed_iso_data(dt) == "2026-11-15T10:40:13.000Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.000Z"


def test_data_format_one_microsecond_truncates() -> None:
    """Data attributes truncate microseconds, not round them."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 1, tzinfo=UTC)
    # 1 microsecond = 0 milliseconds (truncated)
    assert seed_iso_data(dt) == "2026-11-15T10:40:13.000Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.000Z"


def test_data_format_999_microseconds_truncates() -> None:
    """Data attributes truncate 999 microseconds to 0, not round to 1."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 999, tzinfo=UTC)
    # 999 microseconds = 0 milliseconds (truncated, not 1)
    assert seed_iso_data(dt) == "2026-11-15T10:40:13.000Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.000Z"


def test_data_format_999999_microseconds_truncates() -> None:
    """Data attributes truncate 999999 microseconds to 999, not round to 1000."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 999999, tzinfo=UTC)
    # 999999 microseconds = 999 milliseconds (truncated, not 1000)
    assert seed_iso_data(dt) == "2026-11-15T10:40:13.999Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.999Z"


def test_datetime_with_utc_tzinfo() -> None:
    """Data attributes format UTC datetimes correctly."""
    dt = datetime(2026, 11, 15, 10, 40, 13, 123456, tzinfo=UTC)
    result = seed_iso_data(dt)
    # Should have milliseconds format
    assert result.endswith("Z")
    assert "." in result
    # Extract the millisecond part
    millis_part = result.split(".")[1][:-1]  # Remove 'Z' at the end
    assert millis_part == "123"


def test_non_utc_timezone_converted_to_utc() -> None:
    """Non-UTC timezones are converted to UTC before formatting."""
    from datetime import timedelta

    # Create a datetime in UTC+5 (5 hours ahead of UTC)
    plus_5_hours = timezone(timedelta(hours=5))
    dt = datetime(2026, 11, 15, 15, 40, 13, 123456, tzinfo=plus_5_hours)
    # 15:40:13 UTC+5 = 10:40:13 UTC
    result = seed_iso_data(dt)
    assert result == "2026-11-15T10:40:13.123Z"
    assert bootstrap_iso_data(dt) == "2026-11-15T10:40:13.123Z"
