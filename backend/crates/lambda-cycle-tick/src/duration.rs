//! Parses the compact duration strings used by `advanceCycleClosesBy`
//! (`docs/design/api.md` §11a.1), e.g. `"5m"`, `"1h"`, `"4d"`.
//!
//! Mirrors `scripts/seed_dev_data.py`'s `parse_duration` so both dev tools
//! accept exactly the same syntax.

use jiff::SignedDuration;

pub fn parse_duration(raw: &str) -> Result<SignedDuration, String> {
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

    // The `try_` constructors reject amounts jiff can't represent instead of
    // panicking, and the cap keeps the caller's `Timestamp - duration` far from
    // jiff's range limits (that subtraction panics on overflow too). A dev
    // fast-forward of a cycle never needs more than a year, under either
    // cadence (`docs/design/lifecycle.md` §13). `'d'` converts to hours via a
    // checked multiply first, so a huge day count can't overflow `i64` before
    // `try_from_hours` gets a chance to reject it.
    let duration = match unit {
        'm' => SignedDuration::try_from_mins(n),
        'h' => SignedDuration::try_from_hours(n),
        'd' => n.checked_mul(24).and_then(SignedDuration::try_from_hours),
        _ => return Err(invalid()),
    };
    duration
        .filter(|d| {
            *d >= SignedDuration::ZERO && *d <= SignedDuration::from_hours(MAX_ADVANCE_DAYS * 24)
        })
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
        assert_eq!(parse_duration("5m").unwrap(), SignedDuration::from_mins(5));
        assert_eq!(parse_duration("1h").unwrap(), SignedDuration::from_hours(1));
        assert_eq!(
            parse_duration("4d").unwrap(),
            SignedDuration::from_hours(4 * 24)
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            parse_duration(" 5m ").unwrap(),
            SignedDuration::from_mins(5)
        );
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
        assert_eq!(
            parse_duration("366d").unwrap(),
            SignedDuration::from_hours(366 * 24)
        );
    }
}
