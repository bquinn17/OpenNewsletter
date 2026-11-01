//! Cycle scheduling time math (`docs/design/lifecycle.md` §4, §5.4, §5.5, §13).
//!
//! All wall-clock math for "when does the next cycle start" lives here so
//! `persistence` and the lambdas share one implementation, for either cadence
//! a deployment picks (`docs/design/lifecycle.md` §13): `monthly` (`cycleId =
//! yyyymm`) or `weekly` (`cycleId = yyyymmdd`).

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
    Weekday,
};
use chrono_tz::Tz;
use domain::CycleId;

/// How often this deployment runs a cycle, set once per deployment via Lambda
/// env vars (`CYCLE_FREQUENCY`/`CYCLE_WEEKLY_START`), never per group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleCadence {
    Monthly,
    /// The group-local instant each weekly cycle opens (`responseOpenAt`).
    Weekly {
        weekday: Weekday,
        at: NaiveTime,
    },
}

/// `CYCLE_FREQUENCY`/`CYCLE_WEEKLY_START` fail the cold start the same way a
/// missing `TABLE_NAME` does — see [`CycleCadence::from_env`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CadenceError {
    #[error("CYCLE_FREQUENCY must be `monthly` or `weekly` (case-insensitive); got `{0}`")]
    InvalidFrequency(String),
    #[error(
        "CYCLE_WEEKLY_START must be `<day> HH:MM` (e.g. `sun 21:00`), where <day> is a 3-letter \
         or full English weekday name; got `{0}`"
    )]
    InvalidWeeklyStart(String),
}

impl CycleCadence {
    /// Pure core of [`CycleCadence::from_env`] (mirrors `http::error_type_uri_from`'s
    /// pure-function-plus-thin-env-wrapper split): tests never set process env
    /// vars, since `cargo test` runs unit tests in parallel within one process.
    ///
    /// `weekly_start` is read only when `frequency` resolves to weekly; an
    /// unset/empty `frequency` always means monthly, regardless of
    /// `weekly_start`'s value.
    pub fn parse(
        frequency: Option<&str>,
        weekly_start: Option<&str>,
    ) -> Result<Self, CadenceError> {
        let frequency = frequency.unwrap_or("").trim();
        if frequency.is_empty() || frequency.eq_ignore_ascii_case("monthly") {
            return Ok(CycleCadence::Monthly);
        }
        if !frequency.eq_ignore_ascii_case("weekly") {
            return Err(CadenceError::InvalidFrequency(frequency.to_owned()));
        }

        let raw = weekly_start
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(crate::config::DEFAULT_WEEKLY_CYCLE_START);
        let (weekday, at) = parse_weekly_start(raw)
            .ok_or_else(|| CadenceError::InvalidWeeklyStart(raw.to_owned()))?;
        Ok(CycleCadence::Weekly { weekday, at })
    }

    /// Reads `CYCLE_FREQUENCY`/`CYCLE_WEEKLY_START` and calls [`Self::parse`].
    pub fn from_env() -> Result<Self, CadenceError> {
        Self::parse(
            std::env::var("CYCLE_FREQUENCY").ok().as_deref(),
            std::env::var("CYCLE_WEEKLY_START").ok().as_deref(),
        )
    }

    /// `MAX_RESPONSE_WINDOW_DAYS` (28) for monthly, `MAX_WEEKLY_RESPONSE_WINDOW_DAYS`
    /// (7) for weekly, so a `responseWindowDays` can never outlast its period.
    pub fn max_response_window_days(&self) -> u32 {
        match self {
            CycleCadence::Monthly => crate::config::MAX_RESPONSE_WINDOW_DAYS,
            CycleCadence::Weekly { .. } => crate::config::MAX_WEEKLY_RESPONSE_WINDOW_DAYS,
        }
    }

    /// `days`, capped at this cadence's [`Self::max_response_window_days`].
    pub fn clamp_window(&self, days: u32) -> u32 {
        days.min(self.max_response_window_days())
    }
}

/// `<day> <HH>:<MM>`: `<day>` is a 3-letter or full English weekday name
/// (case-insensitive), one space, then a strict two-digit `HH:MM` (`00`-`23`,
/// `00`-`59`) — no single-digit hours/minutes, no missing separator.
fn parse_weekly_start(raw: &str) -> Option<(Weekday, NaiveTime)> {
    let (day, time) = raw.split_once(' ')?;
    if time.contains(' ') {
        return None;
    }
    Some((parse_weekday(day)?, parse_hhmm(time)?))
}

fn parse_weekday(raw: &str) -> Option<Weekday> {
    match raw.to_ascii_lowercase().as_str() {
        "mon" | "monday" => Some(Weekday::Mon),
        "tue" | "tuesday" => Some(Weekday::Tue),
        "wed" | "wednesday" => Some(Weekday::Wed),
        "thu" | "thursday" => Some(Weekday::Thu),
        "fri" | "friday" => Some(Weekday::Fri),
        "sat" | "saturday" => Some(Weekday::Sat),
        "sun" | "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

fn parse_hhmm(raw: &str) -> Option<NaiveTime> {
    let (hh, mm) = raw.split_once(':')?;
    if hh.len() != 2 || mm.len() != 2 {
        return None;
    }
    let hour: u32 = hh.parse().ok()?;
    let minute: u32 = mm.parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

/// A newly computed cycle's identity and window, derived from `(after, tz,
/// response_window_days)` per §4.1/§4.2/§5.4/§13. `vote_window_open_at` is not
/// part of this schedule — it is "informational only" and set by the caller to
/// the instant the row is actually created (§4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleSchedule {
    pub cycle_id: CycleId,
    pub response_open_at: DateTime<Utc>,
    pub response_close_at: DateTime<Utc>,
}

/// `cycleId` of `response_open_at` in the group's local time zone (§4.1, §13):
/// `yyyymm` for monthly, `yyyymmdd` for weekly.
pub fn cycle_id_for(cadence: CycleCadence, response_open_at: DateTime<Utc>, tz: Tz) -> CycleId {
    let local = response_open_at.with_timezone(&tz);
    match cadence {
        CycleCadence::Monthly => CycleId::new(format!("{:04}{:02}", local.year(), local.month())),
        CycleCadence::Weekly { .. } => CycleId::new(format!(
            "{:04}{:02}{:02}",
            local.year(),
            local.month(),
            local.day()
        )),
    }
}

/// The local calendar date a `cycleId` encodes: a 6-digit id is `(year,
/// month, 1)`, an 8-digit id is `(year, month, day)`. `None` unless it is a
/// real calendar date — the digit-length check doubles as the format check.
fn parse_cycle_id(cycle_id: &CycleId) -> Option<NaiveDate> {
    let id = cycle_id.as_str();
    if !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: i32 = id.get(..4)?.parse().ok()?;
    let month: u32 = id.get(4..6)?.parse().ok()?;
    match id.len() {
        6 => NaiveDate::from_ymd_opt(year, month, 1),
        8 => {
            let day: u32 = id.get(6..8)?.parse().ok()?;
            NaiveDate::from_ymd_opt(year, month, day)
        }
        _ => None,
    }
}

/// The anchor §5.4/§13 schedules a cycle's successor from, in UTC. `None` if
/// `cycle_id` isn't a valid id under either format.
///
/// Monthly: local midnight on the 1st of that date's month — for a 6-digit id
/// this is exactly the id's own month. Weekly: that date at the cadence's
/// `at`, local.
///
/// Anchoring on the cycle's identity rather than one of its timestamps keeps
/// the next cycle exactly one period on: a monthly `responseCloseAt` can
/// itself land on the 1st of the next month (a 28-day window in a non-leap
/// February), and the dev-only deadline rewind (`docs/design/api.md` §11a.1)
/// moves `responseOpenAt` back by an arbitrary amount — the id is fixed at
/// creation, so it always yields the following period.
pub fn period_start(cadence: CycleCadence, cycle_id: &CycleId, tz: Tz) -> Option<DateTime<Utc>> {
    let date = parse_cycle_id(cycle_id)?;
    match cadence {
        CycleCadence::Monthly => {
            let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1)?;
            Some(local_to_utc(tz, first, NaiveTime::MIN))
        }
        CycleCadence::Weekly { at, .. } => Some(local_to_utc(tz, date, at)),
    }
}

/// The first period boundary strictly after `after`, in the group's local
/// time zone — used for a group's very first cycle. Monthly is the 1st of the
/// next month at local midnight. Weekly steps local calendar dates (never `+
/// Duration::days(7)` in UTC, so DST can't drift the wall-clock hour) to the
/// first `(weekday, at)` instant after `after`.
pub fn next_period_start(cadence: CycleCadence, after: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    match cadence {
        CycleCadence::Monthly => {
            let local_date = after.with_timezone(&tz).date_naive();
            local_to_utc(tz, next_month_first_day(local_date), NaiveTime::MIN)
        }
        CycleCadence::Weekly { weekday, at } => weekly_instant(after, tz, weekday, at, false),
    }
}

/// The full schedule for the period after `after`, per §4.2/§13's default
/// schedule formula. `response_window_days` is clamped to the cadence's cap.
pub fn next_cycle_schedule(
    cadence: CycleCadence,
    after: DateTime<Utc>,
    tz: Tz,
    response_window_days: u32,
) -> CycleSchedule {
    let response_open_at = next_period_start(cadence, after, tz);
    let response_close_at =
        response_open_at + Duration::days(cadence.clamp_window(response_window_days).into());
    CycleSchedule {
        cycle_id: cycle_id_for(cadence, response_open_at, tz),
        response_open_at,
        response_close_at,
    }
}

/// The schedule for the cycle that succeeds `current_id` (§5.4, §13) —
/// `persistence::newsletters::create_next_voting_cycle`'s caller computes
/// this both at promotion and at successor repair. `None` if `current_id`
/// isn't a valid id under either format.
///
/// Monthly: exactly [`period_start`]'s anchor, then the 1st of the next
/// month — `current_close` plays no part, for the reason [`period_start`]'s
/// doc explains.
///
/// Weekly: the earliest `(weekday, at)` group-local instant that is **on or
/// after** `max(local midnight at the start of (id's date + 7 days),
/// current_close)`. The `max()` guards two cases that could otherwise open
/// the successor before the current cycle closes: an operator changing
/// `CYCLE_WEEKLY_START` to an earlier time on a live deployment, and a legacy
/// `yyyymm` id inherited from a monthly deployment that just switched to
/// weekly. The dev-only deadline rewind (`docs/design/api.md` §11a.1) only
/// ever moves timestamps earlier, so the `max()` is safe against it too.
pub fn successor_schedule(
    cadence: CycleCadence,
    current_id: &CycleId,
    current_close: DateTime<Utc>,
    tz: Tz,
    response_window_days: u32,
) -> Option<CycleSchedule> {
    match cadence {
        CycleCadence::Monthly => {
            let anchor = period_start(cadence, current_id, tz)?;
            Some(next_cycle_schedule(
                cadence,
                anchor,
                tz,
                response_window_days,
            ))
        }
        CycleCadence::Weekly { weekday, at } => {
            let id_date = parse_cycle_id(current_id)?;
            let floor_from_id = local_to_utc(tz, id_date + Duration::days(7), NaiveTime::MIN);
            let floor = floor_from_id.max(current_close);
            let response_open_at = weekly_instant(floor, tz, weekday, at, true);
            let response_close_at = response_open_at
                + Duration::days(cadence.clamp_window(response_window_days).into());
            Some(CycleSchedule {
                cycle_id: cycle_id_for(cadence, response_open_at, tz),
                response_open_at,
                response_close_at,
            })
        }
    }
}

/// The first day on/after `date` whose weekday is `weekday`.
fn next_or_same_weekday(date: NaiveDate, weekday: Weekday) -> NaiveDate {
    date + Duration::days(weekday.days_since(date.weekday()).into())
}

/// The first `(weekday, at)` group-local instant satisfying `floor`: on-or-after
/// it when `inclusive`, strictly after otherwise. Steps whole local calendar
/// weeks from `floor`'s local date — never a fixed UTC duration — so a DST
/// transition inside the gap can't drift the wall-clock hour.
fn weekly_instant(
    floor: DateTime<Utc>,
    tz: Tz,
    weekday: Weekday,
    at: NaiveTime,
    inclusive: bool,
) -> DateTime<Utc> {
    let mut candidate_date = next_or_same_weekday(floor.with_timezone(&tz).date_naive(), weekday);
    loop {
        let candidate = local_to_utc(tz, candidate_date, at);
        let satisfied = if inclusive {
            candidate >= floor
        } else {
            candidate > floor
        };
        if satisfied {
            return candidate;
        }
        candidate_date += Duration::days(7);
    }
}

/// The first day of the month after `date`.
fn next_month_first_day(date: NaiveDate) -> NaiveDate {
    let (year, month) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    // Day 1 is valid in every month — this is an invariant, not a reachable
    // runtime failure.
    NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap_or_else(|| unreachable!("day 1 is invalid in {year:04}-{month:02}"))
}

/// Resolve `date` at `time`, local to `tz`, to a UTC instant, without ever
/// unwrapping a `LocalResult`.
///
/// A local `(date, time)` can be ambiguous (DST fall-back) or nonexistent
/// (DST spring-forward landing exactly on it). Neither case may panic: an
/// ambiguous instant resolves to its earliest instant; a nonexistent one
/// steps forward by whole hours to the next local time that exists. That is
/// normally later the same day, but a gap that runs past midnight (a late
/// `CYCLE_WEEKLY_START` in a zone that springs forward at 23:00) carries
/// into the next day rather than failing.
fn local_to_utc(tz: Tz, date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
    let start = NaiveDateTime::new(date, time);
    for hours in 0..24 {
        match tz.from_local_datetime(&(start + Duration::hours(hours))) {
            LocalResult::Single(dt) => return dt.with_timezone(&Utc),
            LocalResult::Ambiguous(earliest, _latest) => return earliest.with_timezone(&Utc),
            LocalResult::None => continue,
        }
    }
    // No real IANA zone skips a whole day's worth of hours — DST shifts are at
    // most a few hours. This is an invariant, not a reachable runtime failure.
    unreachable!("no valid local time within 24h of {start} in {tz}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{America, Pacific, Tz, UTC};

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn hms(h: u32, mi: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, mi, s).unwrap()
    }

    // -- CycleCadence::parse -------------------------------------------------

    #[test]
    fn parse_defaults_to_monthly_when_frequency_is_unset_or_empty() {
        assert_eq!(
            CycleCadence::parse(None, None).unwrap(),
            CycleCadence::Monthly
        );
        assert_eq!(
            CycleCadence::parse(Some(""), None).unwrap(),
            CycleCadence::Monthly
        );
    }

    #[test]
    fn parse_is_case_insensitive_for_monthly_and_weekly() {
        assert_eq!(
            CycleCadence::parse(Some("MONTHLY"), None).unwrap(),
            CycleCadence::Monthly
        );
        assert_eq!(
            CycleCadence::parse(Some("Weekly"), Some("sun 21:00")).unwrap(),
            CycleCadence::Weekly {
                weekday: Weekday::Sun,
                at: hms(21, 0, 0)
            }
        );
    }

    #[test]
    fn parse_accepts_a_three_letter_or_full_weekday_name() {
        assert_eq!(
            CycleCadence::parse(Some("weekly"), Some("sun 21:00")).unwrap(),
            CycleCadence::Weekly {
                weekday: Weekday::Sun,
                at: hms(21, 0, 0)
            }
        );
        assert_eq!(
            CycleCadence::parse(Some("weekly"), Some("SUNDAY 09:05")).unwrap(),
            CycleCadence::Weekly {
                weekday: Weekday::Sun,
                at: hms(9, 5, 0)
            }
        );
    }

    #[test]
    fn parse_defaults_weekly_start_when_unset_or_empty() {
        let expected = CycleCadence::Weekly {
            weekday: Weekday::Sun,
            at: hms(21, 0, 0),
        };
        assert_eq!(CycleCadence::parse(Some("weekly"), None).unwrap(), expected);
        assert_eq!(
            CycleCadence::parse(Some("weekly"), Some("")).unwrap(),
            expected
        );
    }

    #[test]
    fn parse_rejects_an_unknown_frequency() {
        let err = CycleCadence::parse(Some("weekly-ish"), None).unwrap_err();
        assert_eq!(err, CadenceError::InvalidFrequency("weekly-ish".to_owned()));
    }

    #[test]
    fn parse_rejects_malformed_weekly_start_grammar() {
        for bad in ["sun 24:00", "sun 9:00", "funday 10:00", "sun21:00"] {
            CycleCadence::parse(Some("weekly"), Some(bad))
                .expect_err(&format!("`{bad}` must be rejected"));
        }
    }

    #[test]
    fn parse_ignores_weekly_start_when_frequency_is_monthly() {
        // An invalid weekly_start never surfaces when it isn't read.
        assert_eq!(
            CycleCadence::parse(Some("monthly"), Some("nonsense")).unwrap(),
            CycleCadence::Monthly
        );
    }

    // -- max_response_window_days / clamp_window -----------------------------

    #[test]
    fn monthly_caps_the_response_window_at_28_days() {
        assert_eq!(CycleCadence::Monthly.max_response_window_days(), 28);
        assert_eq!(CycleCadence::Monthly.clamp_window(40), 28);
        assert_eq!(CycleCadence::Monthly.clamp_window(10), 10);
    }

    #[test]
    fn weekly_caps_the_response_window_at_7_days() {
        let weekly = CycleCadence::Weekly {
            weekday: Weekday::Sun,
            at: hms(21, 0, 0),
        };
        assert_eq!(weekly.max_response_window_days(), 7);
        assert_eq!(weekly.clamp_window(10), 7);
        assert_eq!(weekly.clamp_window(5), 5);
    }

    // -- monthly time math (unchanged behavior) ------------------------------

    #[test]
    fn rolls_over_from_december_to_january() {
        let after = utc(2026, 12, 15, 12, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, America::New_York);
        // EST (UTC-5) in January.
        assert_eq!(got, utc(2027, 1, 1, 5, 0, 0));
    }

    #[test]
    fn mid_month_rolls_to_next_month_same_year() {
        let after = utc(2026, 6, 10, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, America::New_York);
        // EDT (UTC-4) in July.
        assert_eq!(got, utc(2026, 7, 1, 4, 0, 0));
    }

    /// `docs/design/lifecycle.md` §11 #8 — a New York cycle whose
    /// `responseOpenAt` is 2026-03-01 must land at the correct UTC instant.
    /// 2026 US DST starts March 8, so March 1 is still EST (UTC-5).
    #[test]
    fn dst_spring_forward_boundary_is_respected() {
        let after = utc(2026, 2, 5, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, America::New_York);
        assert_eq!(got, utc(2026, 3, 1, 5, 0, 0));
    }

    /// Southern-hemisphere DST runs opposite the US calendar: Auckland is on
    /// daylight time (NZDT, UTC+13) in its January.
    #[test]
    fn southern_hemisphere_timezone_is_handled() {
        let after = utc(2026, 1, 5, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, Pacific::Auckland);
        assert_eq!(got, utc(2026, 1, 31, 11, 0, 0));
    }

    #[test]
    fn non_dst_timezone_is_handled() {
        let after = utc(2026, 3, 5, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, chrono_tz::Asia::Tokyo);
        // JST is a fixed UTC+9 offset year-round.
        assert_eq!(got, utc(2026, 3, 31, 15, 0, 0));
    }

    #[test]
    fn utc_timezone_round_trips_trivially() {
        let after = utc(2026, 4, 1, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, UTC);
        assert_eq!(got, utc(2026, 5, 1, 0, 0, 0));
    }

    /// Brazil historically shifted DST at local midnight, so a specific
    /// calendar date's midnight did not exist. Whatever chrono-tz's bundled
    /// tzdata says for this instant, the function must not panic and must
    /// return a time on-or-after the requested day.
    #[test]
    fn nonexistent_local_midnight_does_not_panic() {
        let tz: Tz = America::Sao_Paulo;
        let after = utc(2018, 10, 5, 0, 0, 0);
        let got = next_period_start(CycleCadence::Monthly, after, tz);
        assert!(got >= utc(2018, 11, 1, 0, 0, 0));
        assert!(got < utc(2018, 11, 2, 0, 0, 0));
    }

    #[test]
    fn cycle_id_uses_the_response_open_at_local_date() {
        let response_open_at = utc(2026, 6, 1, 4, 0, 0); // EDT midnight on June 1
        let id = cycle_id_for(CycleCadence::Monthly, response_open_at, America::New_York);
        assert_eq!(id, CycleId::new("202606"));
    }

    #[test]
    fn cycle_id_can_differ_from_the_utc_calendar_date() {
        // 2026-01-31T11:00:00Z is already 2026-02-01 midnight in NZDT (UTC+13).
        let response_open_at = utc(2026, 1, 31, 11, 0, 0);
        let id = cycle_id_for(CycleCadence::Monthly, response_open_at, Pacific::Auckland);
        assert_eq!(id, CycleId::new("202602"));
    }

    #[test]
    fn next_cycle_schedule_matches_the_default_formula() {
        let after = utc(2026, 5, 15, 0, 0, 0);
        let schedule = next_cycle_schedule(CycleCadence::Monthly, after, America::New_York, 4);
        assert_eq!(schedule.cycle_id, CycleId::new("202606"));
        assert_eq!(schedule.response_open_at, utc(2026, 6, 1, 4, 0, 0));
        assert_eq!(schedule.response_close_at, utc(2026, 6, 5, 4, 0, 0));
    }

    #[test]
    fn next_cycle_schedule_clamps_a_window_beyond_the_monthly_cap() {
        let after = utc(2026, 5, 15, 0, 0, 0);
        let schedule = next_cycle_schedule(CycleCadence::Monthly, after, America::New_York, 90);
        assert_eq!(
            schedule.response_close_at - schedule.response_open_at,
            Duration::days(28)
        );
    }

    #[test]
    fn period_start_is_local_midnight_on_the_first() {
        let got = period_start(
            CycleCadence::Monthly,
            &CycleId::new("202711"),
            America::New_York,
        );
        // EST (UTC-5): US DST ends 2027-11-07.
        assert_eq!(got, Some(utc(2027, 11, 1, 4, 0, 0)));
    }

    /// `lambda-groups` reschedules a `voting` cycle from its own id after a
    /// timezone change, so the recomputed open must map back to that id in
    /// any zone, never moving the row's key (`docs/design/lifecycle.md` §6).
    #[test]
    fn period_start_maps_back_to_its_own_cycle_id_monthly() {
        let zones: [Tz; 4] = [America::New_York, Pacific::Honolulu, Pacific::Auckland, UTC];
        for tz in zones {
            for id in ["202607", "202701", "202711"] {
                let start = period_start(CycleCadence::Monthly, &CycleId::new(id), tz).unwrap();
                assert_eq!(
                    cycle_id_for(CycleCadence::Monthly, start, tz),
                    CycleId::new(id),
                    "{id} in {tz}"
                );
            }
        }
    }

    /// A 28-day window in a non-leap February closes at exactly March 1
    /// 00:00, so scheduling from `responseCloseAt` would skip March. Anchored
    /// on the cycle's own month, the next cycle is March.
    #[test]
    fn a_full_length_february_window_does_not_skip_march() {
        let tz = America::New_York;
        let feb = next_cycle_schedule(CycleCadence::Monthly, utc(2027, 1, 15, 0, 0, 0), tz, 28);
        assert_eq!(feb.cycle_id, CycleId::new("202702"));
        assert_eq!(feb.response_close_at, utc(2027, 3, 1, 5, 0, 0));

        let anchor = period_start(CycleCadence::Monthly, &feb.cycle_id, tz).unwrap();
        let next = next_cycle_schedule(CycleCadence::Monthly, anchor, tz, 28);
        assert_eq!(next.cycle_id, CycleId::new("202703"));

        // The bug this guards against.
        let from_close = next_cycle_schedule(CycleCadence::Monthly, feb.response_close_at, tz, 28);
        assert_eq!(from_close.cycle_id, CycleId::new("202704"));
    }

    #[test]
    fn monthly_successor_schedule_matches_the_existing_rule() {
        let tz = America::New_York;
        let feb = next_cycle_schedule(CycleCadence::Monthly, utc(2027, 1, 15, 0, 0, 0), tz, 28);
        let successor = successor_schedule(
            CycleCadence::Monthly,
            &feb.cycle_id,
            feb.response_close_at,
            tz,
            28,
        )
        .unwrap();
        assert_eq!(successor.cycle_id, CycleId::new("202703"));
    }

    // -- malformed ids --------------------------------------------------------

    #[test]
    fn malformed_ids_return_none_for_period_start_and_successor_schedule() {
        let tz = UTC;
        for bad in [
            "",
            "2027",
            "202713",
            "202700",
            "2027-1",
            "20271a",
            "2027011",
            "20271301", // month 13, 8-digit
            "20270230", // Feb 30 doesn't exist
            "2027011a",
            "202701011", // 9 digits
        ] {
            let id = CycleId::new(bad);
            assert_eq!(
                period_start(CycleCadence::Monthly, &id, tz),
                None,
                "{bad:?}"
            );
            assert_eq!(period_start(weekly_sun_2100(), &id, tz), None, "{bad:?}");
            assert_eq!(
                successor_schedule(CycleCadence::Monthly, &id, utc(2026, 1, 1, 0, 0, 0), tz, 7),
                None,
                "{bad:?}"
            );
            assert_eq!(
                successor_schedule(weekly_sun_2100(), &id, utc(2026, 1, 1, 0, 0, 0), tz, 7),
                None,
                "{bad:?}"
            );
        }
    }

    // -- weekly cadence --------------------------------------------------------

    fn weekly_sun_2100() -> CycleCadence {
        CycleCadence::Weekly {
            weekday: Weekday::Sun,
            at: hms(21, 0, 0),
        }
    }

    #[test]
    fn weekly_cycle_id_is_yyyymmdd() {
        let response_open_at = utc(2026, 11, 9, 2, 0, 0); // Sun Nov 8 21:00 EST
        let id = cycle_id_for(weekly_sun_2100(), response_open_at, America::New_York);
        assert_eq!(id, CycleId::new("20261108"));
    }

    #[test]
    fn next_period_start_finds_the_next_weekly_occurrence() {
        let tz = America::New_York;
        // A Wednesday, well before the following Sunday.
        let after = utc(2026, 11, 4, 12, 0, 0);
        let got = next_period_start(weekly_sun_2100(), after, tz);
        assert_eq!(got, utc(2026, 11, 9, 2, 0, 0)); // Sun Nov 8 21:00 EST
    }

    /// "Strictly after": landing exactly on a weekly boundary must advance to
    /// the following week, not return the same instant.
    #[test]
    fn next_period_start_is_strictly_after_an_exact_boundary() {
        let tz = America::New_York;
        let boundary = utc(2026, 11, 9, 2, 0, 0); // Sun Nov 8 21:00 EST exactly
        let got = next_period_start(weekly_sun_2100(), boundary, tz);
        assert_eq!(got, utc(2026, 11, 16, 2, 0, 0)); // the following Sunday, Nov 15 21:00 EST
    }

    /// `docs/design/lifecycle.md` §13 worked example: Sun 2026-11-01 is the US
    /// DST-end day. By 21:00 local the fall-back (at 2am) has already
    /// happened, so 21:00 is already EST (UTC-5). The Sunday before, Oct 25,
    /// is still EDT (UTC-4).
    #[test]
    fn weekly_anchor_across_us_dst_end() {
        let tz = America::New_York;
        let nov1 = local_to_utc(
            tz,
            NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
            hms(21, 0, 0),
        );
        assert_eq!(nov1, utc(2026, 11, 2, 2, 0, 0));
        let oct25 = local_to_utc(
            tz,
            NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
            hms(21, 0, 0),
        );
        assert_eq!(oct25, utc(2026, 10, 26, 1, 0, 0));
    }

    /// A spring-forward week (2026 US DST starts March 8) must still step
    /// exactly 7 local calendar days, not 168 fixed UTC hours.
    #[test]
    fn weekly_stepping_across_a_spring_forward_week() {
        let tz = America::New_York;
        // Sun Mar 1 2026 21:00 EST.
        let after = local_to_utc(
            tz,
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            hms(21, 0, 0),
        );
        let got = next_period_start(weekly_sun_2100(), after, tz);
        // Sun Mar 8 2026 21:00 EDT — the clocks sprang forward at 2am that
        // same day, so the UTC offset is now -4, one hour less than before.
        assert_eq!(got, utc(2026, 3, 9, 1, 0, 0));
    }

    #[test]
    fn weekly_cadence_in_pacific_auckland() {
        let tz = Pacific::Auckland;
        // A Monday in NZ summer (NZDT, UTC+13).
        let after = utc(2026, 1, 5, 0, 0, 0);
        let got = next_period_start(weekly_sun_2100(), after, tz);
        assert_eq!(got, utc(2026, 1, 11, 8, 0, 0)); // Sun Jan 11 21:00 NZDT
    }

    /// Mirrors `period_start_maps_back_to_its_own_cycle_id_monthly` for the
    /// weekly `yyyymmdd` format.
    #[test]
    fn weekly_id_maps_back_to_itself_across_zones() {
        let zones: [Tz; 4] = [America::New_York, Pacific::Honolulu, Pacific::Auckland, UTC];
        for tz in zones {
            for id in ["20260607", "20270103", "20271128"] {
                let start = period_start(weekly_sun_2100(), &CycleId::new(id), tz).unwrap();
                assert_eq!(
                    cycle_id_for(weekly_sun_2100(), start, tz),
                    CycleId::new(id),
                    "{id} in {tz}"
                );
            }
        }
    }

    /// `docs/design/lifecycle.md` §13 worked example: dev's existing voting
    /// cycle `202611` (a legacy `yyyymm` id, opened Sun 2026-11-01 00:00
    /// America/New_York under a 7-day window, closing Nov 8 00:00) switches
    /// to weekly `sun 21:00`. Its successor must be `20261108`, opening Sun
    /// Nov 8 21:00 local — not a week after Nov 1 21:00, which would overlap
    /// the still-open legacy cycle.
    #[test]
    fn legacy_monthly_id_gets_the_correct_weekly_successor() {
        let tz = America::New_York;
        let current_id = CycleId::new("202611");
        let current_close = utc(2026, 11, 8, 5, 0, 0); // Nov 8 00:00 EST

        let successor =
            successor_schedule(weekly_sun_2100(), &current_id, current_close, tz, 7).unwrap();

        assert_eq!(successor.cycle_id, CycleId::new("20261108"));
        assert_eq!(successor.response_open_at, utc(2026, 11, 9, 2, 0, 0)); // Sun Nov 8 21:00 EST
        assert_eq!(
            successor.response_close_at,
            successor.response_open_at + Duration::days(7)
        );
    }

    /// Window == period (7 days, `docs/design/lifecycle.md` §13): the normal
    /// case opens the successor at exactly `id_date + 7 days` at `at`, which
    /// is also exactly when the current cycle closes.
    #[test]
    fn weekly_successor_follows_the_anchor_in_the_normal_case() {
        let tz = America::New_York;
        let current_id = CycleId::new("20261101"); // Sun Nov 1 2026
        let current_close = utc(2026, 11, 9, 2, 0, 0); // Nov 8 21:00 EST, a 7-day window

        let successor =
            successor_schedule(weekly_sun_2100(), &current_id, current_close, tz, 7).unwrap();

        assert_eq!(successor.cycle_id, CycleId::new("20261108"));
        assert_eq!(successor.response_open_at, current_close);
    }

    /// An anchor change (e.g. `CYCLE_WEEKLY_START` moved earlier in the week,
    /// or a shorter window) must never open the successor before the current
    /// cycle's own close.
    #[test]
    fn weekly_successor_never_opens_before_the_current_close() {
        let tz = America::New_York;
        let current_id = CycleId::new("20261101"); // Sun Nov 1 2026
                                                   // A close far later than a plain `id_date + 7 days` would suggest —
                                                   // as if the window had been extended before the cadence switched.
        let current_close = utc(2026, 11, 20, 0, 0, 0);

        let successor =
            successor_schedule(weekly_sun_2100(), &current_id, current_close, tz, 7).unwrap();

        assert!(successor.response_open_at >= current_close);
        assert_eq!(successor.cycle_id, CycleId::new("20261122")); // Sun Nov 22 21:00 EST
    }

    #[test]
    fn weekly_window_clamp_through_successor_schedule() {
        let tz = America::New_York;
        let current_id = CycleId::new("20261101");
        let current_close = utc(2026, 11, 9, 2, 0, 0);
        let successor =
            successor_schedule(weekly_sun_2100(), &current_id, current_close, tz, 10).unwrap();
        assert_eq!(
            successor.response_close_at - successor.response_open_at,
            Duration::days(7)
        );
    }
}
