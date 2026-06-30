//! Test factories. Available to downstream crates via the `test-utils` feature.

use chrono::{TimeZone, Utc};
use domain::*;

pub fn user(user_id: &str) -> User {
    User {
        user_id: UserId::new(user_id),
        cognito_sub: CognitoSub::new(format!("sub-{user_id}")),
        email: format!("{user_id}@example.com"),
        display_name: format!("User {user_id}"),
        avatar_color: "grape".into(),
        avatar_media_id: None,
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        last_login_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    }
}

pub fn group(group_id: &str, created_by: &str) -> Group {
    Group {
        group_id: GroupId::new(group_id),
        name: format!("Group {group_id}"),
        timezone: shared::config::DEFAULT_TIMEZONE.into(),
        cycle_settings: CycleSettings {
            questions_per_cycle: shared::config::DEFAULT_QUESTIONS_PER_CYCLE,
            votes_per_user_per_cycle: shared::config::DEFAULT_VOTES_PER_USER_PER_CYCLE,
            response_window_days: shared::config::DEFAULT_RESPONSE_WINDOW_DAYS,
            auto_publish: true,
        },
        notification_settings: NotificationSettings {
            offsets_hours_before_close: shared::config::DEFAULT_NOTIFY_OFFSETS_HOURS.to_vec(),
            on_cycle_open: true,
        },
        member_count: 1,
        member_soft_cap: shared::config::DEFAULT_MEMBER_SOFT_CAP,
        gradient: "grape-sky".into(),
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        created_by: UserId::new(created_by),
    }
}

pub fn membership(user_id: &str, group_id: &str, role: Role) -> GroupMembership {
    GroupMembership {
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        role,
        joined_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        editions_answered: 0,
    }
}

pub fn voting_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let open = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    let close = Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap();
    Newsletter {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        status: NewsletterStatus::Voting,
        vote_window_open_at: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        vote_window_close_at: open,
        response_open_at: open,
        response_close_at: close,
        published_at: None,
        next_transition_at: Some(open),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
    }
}

pub fn candidate(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    submitter: &str,
    votes: u32,
) -> CandidateQuestion {
    CandidateQuestion {
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        next_cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        prompt: "What's your favorite hike?".into(),
        poll_options: None,
        vote_count: votes,
        submitted_by: UserId::new(submitter),
        is_anonymous: false,
        submitted_at: Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap(),
    }
}
