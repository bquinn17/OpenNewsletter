//! Parses the compact duration strings used by `advanceCycleClosesBy`
//! (`plans/03-api-contract.md` §11a.1), e.g. `"5m"`, `"1h"`, `"4d"`.
//!
//! Mirrors `scripts/seed_dev_data.py`'s `parse_duration` so both dev tools
//! accept exactly the same syntax.

use chrono::Duration;

pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let raw = raw.trim();
    if raw.len() < 2 {
        return Err(format!(
            "invalid duration `{raw}`: expected a number followed by m/h/d (e.g. '5m', '2h', '4d')"
        ));
    }
    let (num_part, unit) = raw.split_at(raw.len() - 1);
    let n: i64 = num_part
        .parse()
        .map_err(|_| format!("invalid duration `{raw}`: the numeric part must be an integer"))?;

    match unit {
        "m" => Ok(Duration::minutes(n)),
        "h" => Ok(Duration::hours(n)),
        "d" => Ok(Duration::days(n)),
        _ => Err(format!(
            "invalid duration `{raw}`: expected a number followed by m/h/d (e.g. '5m', '2h', '4d')"
        )),
    }
}

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
}
