//! Push payload JSON and per-kind copy (`plans/07-notifications.md` §5,
//! M11 decision D6). Pure formatting — no I/O, no persistence lookups.

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use domain::{CycleId, GroupId};
use serde::Serialize;
use shared::config::MAX_PUSH_PAYLOAD_BYTES;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushPayload {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub url: String,
    pub tag: String,
    pub group_name: String,
}

/// `cycle_open` (`07-notifications.md` §6, D6): title "{groupName}: time to
/// write", body "{total} questions are waiting. You have {window} days to
/// respond." `window` is computed by the caller as
/// `round((closeAt - openAt) / 24h)`, minimum 1.
pub fn cycle_open(
    group_name: &str,
    group_id: &GroupId,
    cycle_id: &CycleId,
    total_questions: u32,
    window_days: u32,
) -> PushPayload {
    let title = format!("{group_name}: time to write");
    let questions = if total_questions == 1 {
        "1 question is waiting.".to_owned()
    } else {
        format!("{total_questions} questions are waiting.")
    };
    let days = if window_days == 1 {
        "You have 1 day to respond.".to_owned()
    } else {
        format!("You have {window_days} days to respond.")
    };
    build(
        "cycle_open",
        title,
        format!("{questions} {days}"),
        group_name,
        group_id,
        cycle_id,
    )
}

/// `deadline_reminder` (`07-notifications.md` §7.2, D5/D6). `is_done` means
/// this recipient has published every locked question in the cycle; D5
/// decides whether a done member is sent at all before this is called.
#[allow(clippy::too_many_arguments)]
pub fn deadline_reminder(
    group_name: &str,
    group_id: &GroupId,
    cycle_id: &CycleId,
    offset_hours: u32,
    response_close_at: DateTime<Utc>,
    tz: Tz,
    n_published: u32,
    total_questions: u32,
    is_done: bool,
) -> PushPayload {
    let title = deadline_title(group_name, offset_hours);
    let body = if is_done {
        format!("{n_published}/{total_questions} answers in. You're done — but you can still edit.")
    } else {
        deadline_body(
            offset_hours,
            response_close_at,
            tz,
            n_published,
            total_questions,
        )
    };
    build(
        "deadline_reminder",
        title,
        body,
        group_name,
        group_id,
        cycle_id,
    )
}

fn deadline_title(group_name: &str, offset_hours: u32) -> String {
    if offset_hours >= 48 {
        let days = (offset_hours as f64 / 24.0).round() as u64;
        format!("{group_name}: {days} days left")
    } else if offset_hours >= 24 {
        format!("{group_name}: due tomorrow")
    } else if offset_hours == 1 {
        format!("{group_name}: 1 hour left")
    } else {
        format!("{group_name}: {offset_hours} hours left")
    }
}

fn deadline_body(
    offset_hours: u32,
    response_close_at: DateTime<Utc>,
    tz: Tz,
    n_published: u32,
    total_questions: u32,
) -> String {
    if offset_hours >= 72 {
        format!(
            "Don't forget — write your answers before {}.",
            format_due_date(response_close_at, tz)
        )
    } else if offset_hours >= 36 {
        let remaining = total_questions.saturating_sub(n_published);
        format!("{n_published} of {total_questions} answers written. {remaining} to go.")
    } else {
        format!(
            "Last call — answers lock at {}.",
            format_due_time(response_close_at, tz)
        )
    }
}

/// `%A, %b %-d` in the group's timezone, e.g. "Friday, Oct 9".
pub fn format_due_date(at: DateTime<Utc>, tz: Tz) -> String {
    at.with_timezone(&tz).format("%A, %b %-d").to_string()
}

/// `%-I:%M %p %Z` in the group's timezone, e.g. "9:00 PM EDT".
pub fn format_due_time(at: DateTime<Utc>, tz: Tz) -> String {
    at.with_timezone(&tz).format("%-I:%M %p %Z").to_string()
}

/// `publication` (`07-notifications.md` §8, D6). `answers` is published
/// responses to text questions across all members; `polls` is the number of
/// poll questions locked in the cycle. Always-on; no preference, no group
/// switch.
pub fn publication(
    group_name: &str,
    group_id: &GroupId,
    cycle_id: &CycleId,
    answers: u32,
    polls: u32,
) -> PushPayload {
    let title = format!("{group_name}: this month's edition is out");
    let answers_part = if answers == 1 {
        "1 answer".to_owned()
    } else {
        format!("{answers} answers")
    };
    let body = if polls == 0 {
        format!("{answers_part} — go read it.")
    } else {
        let polls_part = if polls == 1 {
            "1 poll".to_owned()
        } else {
            format!("{polls} polls")
        };
        format!("{answers_part}, {polls_part} — go read it.")
    };
    build("publication", title, body, group_name, group_id, cycle_id)
}

/// `test` (`03-api-contract.md` §10.4, D6). `groupName` is empty; `url` and
/// `tag` are the fixed strings `/settings` and `test`, not the
/// group/cycle-scoped forms the other kinds use.
pub fn test() -> PushPayload {
    PushPayload {
        kind: "test".to_owned(),
        title: "Test push from OpenNewsletter".to_owned(),
        body: "If you got this, your device is wired up.".to_owned(),
        url: "/settings".to_owned(),
        tag: "test".to_owned(),
        group_name: String::new(),
    }
}

fn build(
    kind: &str,
    title: String,
    body: String,
    group_name: &str,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> PushPayload {
    let mut payload = PushPayload {
        kind: kind.to_owned(),
        title,
        body,
        url: format!("/g/{group_id}/n/{cycle_id}"),
        tag: format!("{group_id}:{cycle_id}:{kind}"),
        group_name: group_name.to_owned(),
    };
    enforce_payload_budget(&mut payload);
    payload
}

/// Truncates `body` (never `title`/`url`/`tag`, which are short and fixed in
/// shape) until the serialized payload fits [`MAX_PUSH_PAYLOAD_BYTES`]. A
/// defensive backstop — no copy this module generates should ever actually
/// hit it (`07-notifications.md` §5, D6).
fn enforce_payload_budget(payload: &mut PushPayload) {
    loop {
        let size = serde_json::to_vec(payload).map(|v| v.len()).unwrap_or(0);
        if size <= MAX_PUSH_PAYLOAD_BYTES || payload.body.is_empty() {
            return;
        }
        payload.body.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use chrono_tz::America::New_York;

    fn gid() -> GroupId {
        GroupId::new("01HG2")
    }
    fn cid() -> CycleId {
        CycleId::new("202610")
    }

    #[test]
    fn cycle_open_pluralizes_questions_and_days() {
        let p = cycle_open("Trail Crew", &gid(), &cid(), 5, 4);
        assert_eq!(p.title, "Trail Crew: time to write");
        assert_eq!(
            p.body,
            "5 questions are waiting. You have 4 days to respond."
        );
        assert_eq!(p.kind, "cycle_open");
        assert_eq!(p.url, "/g/01HG2/n/202610");
        assert_eq!(p.tag, "01HG2:202610:cycle_open");
        assert_eq!(p.group_name, "Trail Crew");
    }

    #[test]
    fn cycle_open_uses_singular_forms_for_one() {
        let p = cycle_open("Trail Crew", &gid(), &cid(), 1, 1);
        assert_eq!(p.body, "1 question is waiting. You have 1 day to respond.");
    }

    #[test]
    fn deadline_title_by_offset_band() {
        assert_eq!(deadline_title("G", 96), "G: 4 days left");
        assert_eq!(deadline_title("G", 48), "G: 2 days left");
        assert_eq!(deadline_title("G", 36), "G: due tomorrow");
        assert_eq!(deadline_title("G", 24), "G: due tomorrow");
        assert_eq!(deadline_title("G", 12), "G: 12 hours left");
        assert_eq!(deadline_title("G", 1), "G: 1 hour left");
    }

    #[test]
    fn deadline_body_by_offset_band() {
        let close = New_York
            .with_ymd_and_hms(2026, 10, 9, 21, 0, 0)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            deadline_body(96, close, New_York, 0, 5),
            "Don't forget — write your answers before Friday, Oct 9."
        );
        assert_eq!(
            deadline_body(48, close, New_York, 2, 5),
            "2 of 5 answers written. 3 to go."
        );
        assert_eq!(
            deadline_body(12, close, New_York, 2, 5),
            "Last call — answers lock at 9:00 PM EDT."
        );
    }

    #[test]
    fn deadline_reminder_done_member_gets_the_soft_message() {
        let close = Utc::now();
        let p = deadline_reminder("G", &gid(), &cid(), 48, close, New_York, 5, 5, true);
        assert_eq!(
            p.body,
            "5/5 answers in. You're done — but you can still edit."
        );
        // Title is unaffected by `is_done`.
        assert_eq!(p.title, "G: 2 days left");
    }

    #[test]
    fn publication_omits_the_polls_clause_when_there_are_none() {
        let p = publication("G", &gid(), &cid(), 7, 0);
        assert_eq!(p.body, "7 answers — go read it.");
    }

    #[test]
    fn publication_pluralizes_answers_and_polls() {
        let p = publication("G", &gid(), &cid(), 1, 1);
        assert_eq!(p.body, "1 answer, 1 poll — go read it.");
        let p = publication("G", &gid(), &cid(), 7, 2);
        assert_eq!(p.body, "7 answers, 2 polls — go read it.");
    }

    #[test]
    fn test_payload_has_fixed_shape() {
        let p = test();
        assert_eq!(p.kind, "test");
        assert_eq!(p.url, "/settings");
        assert_eq!(p.tag, "test");
        assert_eq!(p.group_name, "");
    }

    #[test]
    fn oversized_body_is_truncated_to_the_budget() {
        let mut payload = PushPayload {
            kind: "cycle_open".to_owned(),
            title: "G: time to write".to_owned(),
            body: "x".repeat(MAX_PUSH_PAYLOAD_BYTES * 2),
            url: "/g/01HG2/n/202610".to_owned(),
            tag: "01HG2:202610:cycle_open".to_owned(),
            group_name: "G".to_owned(),
        };
        enforce_payload_budget(&mut payload);
        assert!(serde_json::to_vec(&payload).unwrap().len() <= MAX_PUSH_PAYLOAD_BYTES);
    }
}
