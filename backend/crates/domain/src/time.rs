//! The one timestamp type (`docs/design/data-model.md` §1).
//!
//! Every instant the system stores or sends is a [`Timestamp`]: RFC 3339 (the
//! internet profile of ISO 8601), always UTC with a `Z` suffix.
//! - Data attributes and API fields use `YYYY-MM-DDTHH:MM:SS.sssZ`, exactly
//!   three fractional digits, which is also what JavaScript's
//!   `Date.prototype.toISOString()` produces. The width is fixed, so string
//!   order is time order, which a condition such as `response_close_at > :now`
//!   relies on.
//! - Key attributes use [`Timestamp::to_key_string`], the same format at whole
//!   seconds: `YYYY-MM-DDTHH:MM:SSZ`.
//!
//! A `Timestamp` holds whole milliseconds. [`Timestamp::now`] and every parse
//! truncate to them, so a value read back from DynamoDB equals the one written.

use jiff::fmt::temporal::DateTimePrinter;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

pub use jiff::SignedDuration;

const ATTRIBUTE_PRINTER: DateTimePrinter = DateTimePrinter::new().precision(Some(3));
// Precision 0 truncates the fraction rather than rounding it.
const KEY_PRINTER: DateTimePrinter = DateTimePrinter::new().precision(Some(0));

/// A UTC instant at millisecond precision. See the module docs for its formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(jiff::Timestamp);

impl Timestamp {
    /// The Unix epoch, `1970-01-01T00:00:00.000Z`.
    pub const UNIX_EPOCH: Timestamp = Timestamp(jiff::Timestamp::UNIX_EPOCH);

    /// The current instant, truncated to the millisecond.
    pub fn now() -> Self {
        Self::from_jiff(jiff::Timestamp::now())
    }

    /// Wraps a jiff instant, truncating it to the millisecond.
    pub fn from_jiff(ts: jiff::Timestamp) -> Self {
        // `as_millisecond` truncates, and the whole milliseconds of a valid
        // instant are always a valid instant, so the fallback is never taken.
        Self(jiff::Timestamp::from_millisecond(ts.as_millisecond()).unwrap_or(ts))
    }

    pub fn as_jiff(self) -> jiff::Timestamp {
        self.0
    }

    /// Whole seconds since the Unix epoch, as DynamoDB TTL attributes store them.
    pub fn as_unix_seconds(self) -> i64 {
        self.0.as_second()
    }

    pub fn as_unix_millis(self) -> i64 {
        self.0.as_millisecond()
    }

    /// `None` when `seconds` is outside the range jiff represents.
    pub fn from_unix_seconds(seconds: i64) -> Option<Self> {
        jiff::Timestamp::from_second(seconds).ok().map(Self)
    }

    /// `None` when `millis` is outside the range jiff represents.
    pub fn from_unix_millis(millis: i64) -> Option<Self> {
        jiff::Timestamp::from_millisecond(millis).ok().map(Self)
    }

    /// `YYYY-MM-DDTHH:MM:SSZ`, the whole-second form every timestamp in a key
    /// attribute uses (`persistence::keys::key_timestamp`). Sub-second digits
    /// are dropped, not rounded.
    pub fn to_key_string(self) -> String {
        KEY_PRINTER.timestamp_to_string(&self.0)
    }

    /// `None` on overflow, which no real schedule reaches.
    pub fn checked_add(self, duration: SignedDuration) -> Option<Self> {
        self.0.checked_add(duration).ok().map(Self::from_jiff)
    }

    /// `None` on overflow, which no real schedule reaches.
    pub fn checked_sub(self, duration: SignedDuration) -> Option<Self> {
        self.0.checked_sub(duration).ok().map(Self::from_jiff)
    }

    /// `self - earlier`; negative when `earlier` is later.
    pub fn duration_since(self, earlier: Timestamp) -> SignedDuration {
        self.0.duration_since(earlier.0)
    }
}

/// Saturates at jiff's range limits instead of panicking.
impl Add<SignedDuration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: SignedDuration) -> Timestamp {
        Self::from_jiff(self.0.saturating_add(rhs).unwrap_or(self.0))
    }
}

/// Saturates at jiff's range limits instead of panicking.
impl Sub<SignedDuration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: SignedDuration) -> Timestamp {
        Self::from_jiff(self.0.saturating_sub(rhs).unwrap_or(self.0))
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = SignedDuration;

    fn sub(self, rhs: Timestamp) -> SignedDuration {
        self.duration_since(rhs)
    }
}

impl From<jiff::Timestamp> for Timestamp {
    fn from(ts: jiff::Timestamp) -> Self {
        Self::from_jiff(ts)
    }
}

impl From<Timestamp> for jiff::Timestamp {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

/// `YYYY-MM-DDTHH:MM:SS.sssZ`.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ATTRIBUTE_PRINTER.timestamp_to_string(&self.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not an RFC 3339 timestamp: {0}")]
pub struct TimestampParseError(String);

/// Accepts any RFC 3339 instant: any offset (`Z`, `+00:00`, `-04:00`) and any
/// number of fractional digits, so rows written before this format was fixed
/// still read. The result is truncated to the millisecond.
impl FromStr for Timestamp {
    type Err = TimestampParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<jiff::Timestamp>()
            .map(Self::from_jiff)
            .map_err(|e| TimestampParseError(format!("{s:?}: {e}")))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn displays_utc_with_exactly_three_fractional_digits() {
        assert_eq!(
            ts("2026-11-15T10:40:13Z").to_string(),
            "2026-11-15T10:40:13.000Z"
        );
        assert_eq!(
            ts("2026-11-15T10:40:13.5Z").to_string(),
            "2026-11-15T10:40:13.500Z"
        );
        assert_eq!(
            ts("2026-11-15T10:40:13.123456789Z").to_string(),
            "2026-11-15T10:40:13.123Z",
            "truncated, not rounded"
        );
        assert_eq!(
            ts("2026-11-15T10:40:13.999999999Z").to_string(),
            "2026-11-15T10:40:13.999Z"
        );
    }

    #[test]
    fn parses_the_older_stored_forms() {
        // chrono's `to_rfc3339()` and an offset other than UTC.
        assert_eq!(
            ts("2026-11-15T10:40:13.123456+00:00"),
            ts("2026-11-15T10:40:13.123Z")
        );
        assert_eq!(ts("2026-11-15T05:40:13-05:00"), ts("2026-11-15T10:40:13Z"));
    }

    #[test]
    fn rejects_what_is_not_an_instant() {
        for bad in ["", "2026-11-15", "2026-11-15T10:40:13", "yesterday"] {
            assert!(bad.parse::<Timestamp>().is_err(), "{bad:?} parsed");
        }
    }

    #[test]
    fn key_string_is_whole_seconds() {
        assert_eq!(
            ts("2026-11-15T10:40:13.999Z").to_key_string(),
            "2026-11-15T10:40:13Z"
        );
        assert_eq!(
            ts("2026-11-15T10:40:13Z").to_key_string(),
            "2026-11-15T10:40:13Z"
        );
    }

    #[test]
    fn now_is_whole_milliseconds_and_round_trips() {
        let now = Timestamp::now();
        assert_eq!(now.as_jiff().subsec_nanosecond() % 1_000_000, 0);
        assert_eq!(now.to_string().parse::<Timestamp>().unwrap(), now);
    }

    #[test]
    fn string_order_is_time_order() {
        let a = ts("2026-11-15T10:40:13Z");
        let b = ts("2026-11-15T10:40:13.001Z");
        let c = ts("2026-11-15T10:40:14Z");
        assert!(a.to_string() < b.to_string() && b.to_string() < c.to_string());
    }

    #[test]
    fn serde_uses_the_display_form() {
        let t = ts("2026-11-15T10:40:13.5Z");
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            "\"2026-11-15T10:40:13.500Z\""
        );
        let back: Timestamp = serde_json::from_str("\"2026-11-15T10:40:13.5+00:00\"").unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn arithmetic_keeps_millisecond_precision() {
        let t = ts("2026-11-15T10:40:13Z");
        assert_eq!(
            (t + SignedDuration::from_hours(24)).to_string(),
            "2026-11-16T10:40:13.000Z"
        );
        assert_eq!(
            (t + SignedDuration::from_nanos(1_500_000)).to_string(),
            "2026-11-15T10:40:13.001Z"
        );
        assert_eq!(
            ts("2026-11-16T10:40:13Z") - t,
            SignedDuration::from_hours(24)
        );
        assert_eq!(t.as_unix_seconds(), 1_794_739_213);
        assert_eq!(Timestamp::from_unix_seconds(1_794_739_213), Some(t));
    }
}
