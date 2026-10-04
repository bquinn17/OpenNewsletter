//! Parses the compact duration strings used by `advanceCycleClosesBy`
//! (`plans/03-api-contract.md` §11a.1), e.g. `"5m"`, `"1h"`, `"4d"`.
//!
//! Mirrors `scripts/seed_dev_data.py`'s `parse_duration` so both dev tools
//! accept exactly the same syntax.

use chrono::Duration;

pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let raw = raw.trim();
    let invalid = || {
        format!(
            "invalid duration `{raw}`: expected a number followed by m/h/d (e.g. '5m', '2h', '4d')"
        )
    };
    // Split off the last *char*, not byte: input is user-supplied, and a
    // multi-byte trailing char would make a byte split panic.
    let (unit_at, unit) = raw.char_indices().next_back().ok_or_else(invalid)?;
    let num_part = &raw[..unit_at];
    if num_part.is_empty() {
        return Err(invalid());
    }
    let n: i64 = num_part
        .parse()
        .map_err(|_| format!("invalid duration `{raw}`: the numeric part must be an integer"))?;

    // The `try_` constructors reject amounts chrono can't represent instead of
    // panicking, and the cap keeps the caller's `DateTime - duration` far from
    // chrono's range limits (that subtraction panics on overflow too). A dev
    // fast-forward of a monthly cycle never needs more than a year.
    let duration = match unit {
        'm' => Duration::try_minutes(n),
        'h' => Duration::try_hours(n),
        'd' => Duration::try_days(n),
        _ => return Err(invalid()),
    };
    duration
        .filter(|d| *d >= Duration::zero() && *d <= Duration::days(MAX_ADVANCE_DAYS))
        .ok_or_else(|| {
            format!("invalid duration `{raw}`: must be between 0 and {MAX_ADVANCE_DAYS} days")
        })
}

/// Upper bound on one `advanceCycleClosesBy` step.
const MAX_ADVANCE_DAYS: i64 = 366;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minutes_hours_and_days() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::minutes(5));
        assert_eq!(parse_duration("1h").unwrap(), Duration::hours(1));
        assert_eq!(parse_duration("4d").unwrap(), Duration::days(4));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(parse_duration(" 5m ").unwrap(), Duration::minutes(5));
    }

    #[test]
    fn rejects_an_unknown_unit() {
        assert!(parse_duration("5s").is_err());
    }

    #[test]
    fn rejects_a_non_numeric_amount() {
        assert!(parse_duration("fivem").is_err());
    }

    #[test]
    fn rejects_a_missing_amount() {
        assert!(parse_duration("m").is_err());
    }

    #[test]
    fn rejects_an_empty_string() {
        assert!(parse_duration("").is_err());
    }

    #[test]
    fn rejects_a_multibyte_unit_without_panicking() {
        assert!(parse_duration("1µ").is_err());
        assert!(parse_duration("µ").is_err());
        assert!(parse_duration("5mµ").is_err());
    }

    #[test]
    fn rejects_an_out_of_range_amount_without_panicking() {
        assert!(parse_duration("99999999999999999d").is_err());
        assert!(parse_duration("9223372036854775807m").is_err());
        assert!(parse_duration("367d").is_err());
        assert!(parse_duration("-5d").is_err());
        assert_eq!(parse_duration("366d").unwrap(), Duration::days(366));
    }
}
