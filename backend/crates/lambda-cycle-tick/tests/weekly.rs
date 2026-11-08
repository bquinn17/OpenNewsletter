//! A weekly-cadence deployment runs the same tick as monthly, but promotes
//! on a `yyyymmdd` id and seeds an 8-digit successor at the weekly anchor
//! (`docs/design/lifecycle.md` §13).

mod common;

use chrono::{Duration, NaiveTime, Utc, Weekday};
use cycle_tick::tick::run_tick;
use domain::{GroupId, Newsletter, NewsletterStatus, QuestionId};
use persistence::{groups, newsletters, questions};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

fn weekly_sun_2100() -> CycleCadence {
    CycleCadence::Weekly {
        weekday: Weekday::Sun,
        at: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
    }
}

#[tokio::test]
async fn a_weekly_tick_promotes_a_cycle_and_creates_a_yyyymmdd_successor_at_the_anchor() {
    let (_c, repo) = common::make_test_repo().await;
    let mut group = common::group("g-weekly", "u1");
    group.cycle_settings.questions_per_cycle = 2;
    group.cycle_settings.response_window_days = 7;
    groups::put_group(&repo, &group).await.unwrap();

    let cadence = weekly_sun_2100();
    let tz: chrono_tz::Tz = group.timezone.parse().unwrap();

    // A voting cycle that's due now. Its close stays in the future, so the
    // same tick doesn't go on to publish it.
    let now = Utc::now();
    let voting = Newsletter {
        group_id: GroupId::new("g-weekly"),
        cycle_id: shared::cycle_time::cycle_id_for(cadence, now, tz),
        status: NewsletterStatus::Voting,
        vote_window_open_at: now - Duration::days(7),
        vote_window_close_at: now - Duration::minutes(1),
        response_open_at: now - Duration::minutes(1),
        response_close_at: now + Duration::days(7),
        published_at: None,
        next_transition_at: Some(now - Duration::minutes(1)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    assert_eq!(voting.cycle_id.as_str().len(), 8, "weekly ids are yyyymmdd");

    let candidate = common::candidate("g-weekly", voting.cycle_id.as_str(), "q1", "u1", 1);
    questions::put_candidate(&repo, &candidate).await.unwrap();

    run_tick(&repo, cadence).await;

    let opened = newsletters::get_newsletter(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(opened.locked_question_ids, vec![QuestionId::new("q1")]);

    let next_voting = newsletters::find_voting_cycle(&repo, &voting.group_id)
        .await
        .unwrap()
        .expect("the tick seeds the group's next voting cycle");
    assert_eq!(
        next_voting.cycle_id.as_str().len(),
        8,
        "the successor is also a weekly yyyymmdd id"
    );

    let expected = shared::cycle_time::successor_schedule(
        cadence,
        &voting.cycle_id,
        opened.response_close_at,
        tz,
        group.cycle_settings.response_window_days,
    )
    .expect("a valid weekly successor");
    assert_eq!(next_voting.cycle_id, expected.cycle_id);
    assert_eq!(next_voting.response_open_at, expected.response_open_at);
    assert_eq!(next_voting.response_close_at, expected.response_close_at);
}
