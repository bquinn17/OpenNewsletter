//! Cycle scheduling time math (`plans/06-newsletter-lifecycle.md` §4, §5.4, §5.5).
//!
//! All wall-clock math for "when does the next cycle start" lives here so
//! `persistence` and `lambda-cycle-tick` share one implementation.

use chrono::{DateTime, Datelike, Duration, LocalResult, TimeZone, Utc};
use chrono_tz::Tz;
use domain::CycleId;

/// A newly computed cycle's identity and window, derived from `(after, tz,
/// response_window_days)` per §4.1/§4.2/§5.4. `vote_window_open_at` is not part
/// of this schedule — it is "informational only" and set by the caller to the
/// instant the row is actually created (§4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleSchedule {
    pub cycle_id: CycleId,
    pub response_open_at: DateTime<Utc>,
    pub response_close_at: DateTime<Utc>,
}

/// The instant "first day of the month after `after_utc`, at 00:00 `tz`-local"
/// converts to, in UTC.
///
/// A local midnight can be ambiguous (DST fall-back) or nonexistent (DST
/// spring-forward landing exactly on midnight, as some zones have historically
/// done). Neither case may panic: an ambiguous midnight resolves to its
/// earliest instant; a nonexistent one resolves to the first local hour that
/// *does* exist on that calendar day.
pub fn first_day_of_next_month_local(after_utc: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    let local = after_utc.with_timezone(&tz);
    let (year, month) = if local.month() == 12 {
        (local.year() + 1, 1)
    } else {
        (local.year(), local.month() + 1)
    };
    midnight_local_to_utc(tz, year, month, 1)
}

/// `cycleId = yyyymm` of `response_open_at` in the group's local time zone (§4.1).
pub fn cycle_id_for(response_open_at: DateTime<Utc>, tz: Tz) -> CycleId {
    let local = response_open_at.with_timezone(&tz);
    CycleId::new(format!("{:04}{:02}", local.year(), local.month()))
}

/// The full schedule for the cycle covering the month after `after`, per
/// §4.2's default-schedule formula.
pub fn next_cycle_schedule(
    after: DateTime<Utc>,
    tz: Tz,
    response_window_days: u32,
) -> CycleSchedule {
    let response_open_at = first_day_of_next_month_local(after, tz);
    let response_close_at = response_open_at + Duration::days(response_window_days.into());
    CycleSchedule {
        cycle_id: cycle_id_for(response_open_at, tz),
        response_open_at,
        response_close_at,
    }
}

/// Resolve local midnight on `(year, month, day)` in `tz` to a UTC instant,
/// without ever unwrapping a `LocalResult`.
fn midnight_local_to_utc(tz: Tz, year: i32, month: u32, day: u32) -> DateTime<Utc> {
    for hour in 0..24 {
        match tz.with_ymd_and_hms(year, month, day, hour, 0, 0) {
            LocalResult::Single(dt) => return dt.with_timezone(&Utc),
            // Fall-back DST transition through midnight: take the earlier
            // (pre-transition) instant, matching "the moment local clocks
            // first read this wall time."
            LocalResult::Ambiguous(earliest, _latest) => return earliest.with_timezone(&Utc),
            // Spring-forward transition lands exactly on this wall-clock hour
            // (some zones have historically done this at midnight); try the
            // next hour on the same calendar day.
            LocalResult::None => continue,
        }
    }
    // No real IANA zone lacks every hour of a calendar day — DST shifts are at
    // most a few hours. This is an invariant, not a reachable runtime failure.
    unreachable!("no valid local hour found on {year:04}-{month:02}-{day:02} in {tz}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::{America, Pacific, Tz, UTC};

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn rolls_over_from_december_to_january() {
        let after = utc(2026, 12, 15, 12, 0, 0);
        let got = first_day_of_next_month_local(after, America::New_York);
        // EST (UTC-5) in January.
        assert_eq!(got, utc(2027, 1, 1, 5, 0, 0));
    }

    #[test]
    fn mid_month_rolls_to_next_month_same_year() {
        let after = utc(2026, 6, 10, 0, 0, 0);
        let got = first_day_of_next_month_local(after, America::New_York);
        // EDT (UTC-4) in July.
        assert_eq!(got, utc(2026, 7, 1, 4, 0, 0));
    }

    /// `06-newsletter-lifecycle.md` §11 #8 — a New York cycle whose
    /// `responseOpenAt` is 2026-03-01 must land at the correct UTC instant.
    /// 2026 US DST starts March 8, so March 1 is still EST (UTC-5).
    #[test]
    fn dst_spring_forward_boundary_is_respected() {
        let after = utc(2026, 2, 5, 0, 0, 0);
        let got = first_day_of_next_month_local(after, America::New_York);
        assert_eq!(got, utc(2026, 3, 1, 5, 0, 0));
    }

    /// Southern-hemisphere DST runs opposite the US calendar: Auckland is on
    /// daylight time (NZDT, UTC+13) in its January.
    #[test]
    fn southern_hemisphere_timezone_is_handled() {
        let after = utc(2026, 1, 5, 0, 0, 0);
        let got = first_day_of_next_month_local(after, Pacific::Auckland);
        assert_eq!(got, utc(2026, 1, 31, 11, 0, 0));
    }

    #[test]
    fn non_dst_timezone_is_handled() {
        let after = utc(2026, 3, 5, 0, 0, 0);
        let got = first_day_of_next_month_local(after, chrono_tz::Asia::Tokyo);
        // JST is a fixed UTC+9 offset year-round.
        assert_eq!(got, utc(2026, 3, 31, 15, 0, 0));
    }

    #[test]
    fn utc_timezone_round_trips_trivially() {
        let after = utc(2026, 4, 1, 0, 0, 0);
        let got = first_day_of_next_month_local(after, UTC);
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
        let got = first_day_of_next_month_local(after, tz);
        assert!(got >= utc(2018, 11, 1, 0, 0, 0));
        assert!(got < utc(2018, 11, 2, 0, 0, 0));
    }

    #[test]
    fn cycle_id_uses_the_response_open_at_local_date() {
        let response_open_at = utc(2026, 6, 1, 4, 0, 0); // EDT midnight on June 1
        let id = cycle_id_for(response_open_at, America::New_York);
        assert_eq!(id, CycleId::new("202606"));
    }

    #[test]
    fn cycle_id_can_differ_from_the_utc_calendar_date() {
        // 2026-01-31T11:00:00Z is already 2026-02-01 midnight in NZDT (UTC+13).
        let response_open_at = utc(2026, 1, 31, 11, 0, 0);
        let id = cycle_id_for(response_open_at, Pacific::Auckland);
        assert_eq!(id, CycleId::new("202602"));
    }

    #[test]
    fn next_cycle_schedule_matches_the_default_formula() {
        let after = utc(2026, 5, 15, 0, 0, 0);
        let schedule = next_cycle_schedule(after, America::New_York, 4);
        assert_eq!(schedule.cycle_id, CycleId::new("202606"));
        assert_eq!(schedule.response_open_at, utc(2026, 6, 1, 4, 0, 0));
        assert_eq!(schedule.response_close_at, utc(2026, 6, 5, 4, 0, 0));
    }
}
