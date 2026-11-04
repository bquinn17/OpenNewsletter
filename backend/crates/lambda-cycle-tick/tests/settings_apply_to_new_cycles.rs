//! A `timezone`/`responseWindowDays` change made after a cycle already
//! exists never reschedules that cycle; only a cycle the tick creates after
//! the change reads the new settings (`docs/design/lifecycle.md` §6).

mod common;

use chrono::{Duration, Utc};
use cycle_tick::tick::run_tick;
use domain::{Newsletter, NewsletterStatus};
use persistence::{groups, newsletters};
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
    };
    newsletters::write_status_transition(&repo, &open)
        .await
        .unwrap();

    // The settings change happens after the open cycle already exists.
    group.timezone = "Pacific/Honolulu".into();
    group.cycle_settings.response_window_days = 10;
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
}
