//! A cycle-settings change made after a cycle already exists never changes
//! that cycle; only a cycle the tick creates after the change reads the new
//! settings (`docs/design/lifecycle.md` §6).

mod common;

use chrono::{Duration, Utc};
use cycle_tick::tick::run_tick;
use domain::{GroupId, Newsletter, NewsletterStatus, QuestionId};
use persistence::{groups, newsletters, questions};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

#[tokio::test]
async fn a_settings_change_after_an_open_cycle_exists_only_applies_to_its_successor() {
    let (_c, repo) = common::make_test_repo().await;
    let mut group = common::group("g-settings-apply", "u1");
    group.cycle_settings.response_window_days = 7;
    groups::put_group(&repo, &group).await.unwrap();

    let now = Utc::now();
    let old_tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let cycle_id = shared::cycle_time::cycle_id_for(CycleCadence::Monthly, now, old_tz);
    let open = Newsletter {
        group_id: group.group_id.clone(),
        cycle_id: cycle_id.clone(),
        status: NewsletterStatus::Open,
        vote_window_open_at: now - Duration::days(30),
        vote_window_close_at: now - Duration::days(1),
        response_open_at: now - Duration::days(1),
        response_close_at: now + Duration::days(5),
        published_at: None,
        next_transition_at: Some(now + Duration::days(5)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
    };
    newsletters::write_status_transition(&repo, &open)
        .await
        .unwrap();

    // The settings change happens after the open cycle already exists.
    group.timezone = "Pacific/Honolulu".into();
    group.cycle_settings.response_window_days = 10;
    group.cycle_settings.questions_per_cycle = 7;
    group.cycle_settings.votes_per_user_per_cycle = 4;
    groups::put_group(&repo, &group).await.unwrap();

    run_tick(&repo, CycleCadence::Monthly).await;

    // The open cycle itself is untouched — no reschedule.
    let reloaded_open = newsletters::get_newsletter(&repo, &open.group_id, &open.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded_open, open);

    // Its successor, created by the repair pass, is scheduled with the new
    // timezone and window.
    let new_tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let expected = shared::cycle_time::successor_schedule(
        CycleCadence::Monthly,
        &open.cycle_id,
        open.response_close_at,
        new_tz,
        group.cycle_settings.response_window_days,
    )
    .unwrap();
    let successor = newsletters::find_voting_cycle(&repo, &open.group_id)
        .await
        .unwrap()
        .expect("the tick creates the successor");
    assert_eq!(successor.cycle_id, expected.cycle_id);
    assert_eq!(successor.response_open_at, expected.response_open_at);
    assert_eq!(successor.response_close_at, expected.response_close_at);
    assert_eq!(successor.questions_per_cycle, Some(7));
    assert_eq!(successor.votes_per_user_per_cycle, Some(4));
}

#[tokio::test]
async fn promotion_locks_the_cycles_own_question_count_not_the_groups_current_one() {
    let (_c, repo) = common::make_test_repo().await;
    let mut group = common::group("g-settings-promote", "u1");
    // Raised after the voting cycle below was created with 2.
    group.cycle_settings.questions_per_cycle = 5;
    groups::put_group(&repo, &group).await.unwrap();

    let now = Utc::now();
    let tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let voting = Newsletter {
        group_id: GroupId::new("g-settings-promote"),
        cycle_id: shared::cycle_time::cycle_id_for(CycleCadence::Monthly, now, tz),
        status: NewsletterStatus::Voting,
        vote_window_open_at: now - Duration::days(30),
        vote_window_close_at: now - Duration::minutes(1),
        response_open_at: now - Duration::minutes(1),
        response_close_at: now + Duration::days(7),
        published_at: None,
        next_transition_at: Some(now - Duration::minutes(1)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: Some(2),
        votes_per_user_per_cycle: Some(3),
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    let cycle = voting.cycle_id.as_str();
    for (id, votes) in [("q-a", 4), ("q-b", 3), ("q-c", 2), ("q-d", 1)] {
        let c = common::candidate("g-settings-promote", cycle, id, "u1", votes);
        questions::put_candidate(&repo, &c).await.unwrap();
    }

    run_tick(&repo, CycleCadence::Monthly).await;

    let opened = newsletters::get_newsletter(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(
        opened.locked_question_ids,
        vec![QuestionId::new("q-a"), QuestionId::new("q-b")]
    );
    // Its successor takes the group's current value.
    let successor = newsletters::find_voting_cycle(&repo, &voting.group_id)
        .await
        .unwrap()
        .expect("promotion creates the successor");
    assert_eq!(successor.questions_per_cycle, Some(5));
}
