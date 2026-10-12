//! Promotion order: votes DESC, then the older suggestion, then question id
//! (`plans/06-newsletter-lifecycle.md` §5.2).

mod common;

use chrono::{Duration, TimeZone, Utc};
use cycle_tick::tick::run_tick;
use domain::{GroupId, Newsletter, NewsletterStatus, QuestionId};
use persistence::{groups, newsletters, questions};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn ties_go_to_the_older_suggestion_and_zero_vote_candidates_fill_the_rest() {
    let (_c, repo) = common::make_test_repo().await;
    let mut group = common::group("g-ties", "u1");
    group.cycle_settings.questions_per_cycle = 3;
    groups::put_group(&repo, &group).await.unwrap();

    // A voting cycle that's due now. Its close stays in the future, so the same
    // tick doesn't go on to publish it.
    let now = Utc::now();
    let tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let voting = Newsletter {
        group_id: GroupId::new("g-ties"),
        cycle_id: shared::cycle_time::cycle_id_for(now, tz),
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
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    // Ids sort the opposite way to submission time, so only `submitted_at`
    // can explain the expected order. GSI1 would put "q-z" before "q-a".
    let day = |d| Utc.with_ymd_and_hms(2026, 5, d, 0, 0, 0).unwrap();
    let cycle = voting.cycle_id.as_str();
    for (id, votes, submitted) in [
        ("q-a", 2, day(20)), // tied, newer
        ("q-z", 2, day(10)), // tied, older: wins the tie
        ("q-m", 0, day(5)),  // oldest zero-vote candidate: fills the last slot
        ("q-b", 0, day(15)), // newer zero-vote candidate: left out
    ] {
        let mut c = common::candidate("g-ties", cycle, id, "u1", votes);
        c.submitted_at = submitted;
        questions::put_candidate(&repo, &c).await.unwrap();
    }

    run_tick(&repo).await;

    let opened = newsletters::get_newsletter(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(
        opened.locked_question_ids,
        vec![
            QuestionId::new("q-z"),
            QuestionId::new("q-a"),
            QuestionId::new("q-m")
        ]
    );
}
