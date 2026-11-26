//! Promotion order: votes DESC, then the older suggestion, then question id
//! (`docs/design/lifecycle.md` §5.2).

mod common;

use cycle_tick::tick::run_tick;
use domain::{GroupId, Newsletter, NewsletterStatus, QuestionId, SignedDuration, Timestamp};
use persistence::{groups, newsletters, questions};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

#[tokio::test]
async fn ties_go_to_the_older_suggestion_and_zero_vote_candidates_fill_the_rest() {
    let (_c, repo) = common::make_test_repo().await;
    let mut group = common::group("g-ties", "u1");
    group.cycle_settings.questions_per_cycle = 3;
    groups::put_group(&repo, &group).await.unwrap();

    // A voting cycle that's due now. Its close stays in the future, so the same
    // tick doesn't go on to publish it.
    let now = Timestamp::now();
    let tz = shared::cycle_time::time_zone(&group.timezone).unwrap();
    let voting = Newsletter {
        group_id: GroupId::new("g-ties"),
        cycle_id: shared::cycle_time::cycle_id_for(CycleCadence::Monthly, now, &tz),
        status: NewsletterStatus::Voting,
        vote_window_open_at: now - SignedDuration::from_hours(30 * 24),
        vote_window_close_at: now - SignedDuration::from_mins(1),
        response_open_at: now - SignedDuration::from_mins(1),
        response_close_at: now + SignedDuration::from_hours(7 * 24),
        published_at: None,
        next_transition_at: Some(now - SignedDuration::from_mins(1)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    // Ids sort the opposite way to submission time, so only `submitted_at`
    // can explain the expected order. GSI1 would put "q-z" before "q-a".
    let day = |d: u32| {
        format!("2026-05-{d:02}T00:00:00Z")
            .parse::<Timestamp>()
            .unwrap()
    };
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

    run_tick(&repo, CycleCadence::Monthly).await;

    let opened = newsletters::get_newsletter(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    // The top 3 promoted, in order, plus the standing question appended last
    // with a freshly generated id (`docs/design/lifecycle.md` §5.2).
    assert_eq!(opened.locked_question_ids.len(), 4);
    assert_eq!(
        &opened.locked_question_ids[..3],
        &[
            QuestionId::new("q-z"),
            QuestionId::new("q-a"),
            QuestionId::new("q-m")
        ]
    );
    let locked = questions::list_locked_questions(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap();
    let standing = locked
        .iter()
        .find(|lq| lq.question_id == opened.locked_question_ids[3])
        .expect("standing question row exists");
    assert!(standing.is_standing());
    assert_eq!(standing.display_order, 3);
    assert_eq!(standing.prompt, shared::config::STANDING_QUESTION_PROMPT);
}

/// Even with no candidates suggested at all, the cycle still promotes to
/// `open` — with exactly the standing question (`docs/design/lifecycle.md`
/// §5.2, §9).
#[tokio::test]
async fn an_empty_candidate_pool_still_promotes_with_just_the_standing_question() {
    let (_c, repo) = common::make_test_repo().await;
    let group = common::group("g-empty-pool", "u1");
    groups::put_group(&repo, &group).await.unwrap();

    let now = Timestamp::now();
    let tz = shared::cycle_time::time_zone(&group.timezone).unwrap();
    let voting = Newsletter {
        group_id: GroupId::new("g-empty-pool"),
        cycle_id: shared::cycle_time::cycle_id_for(CycleCadence::Monthly, now, &tz),
        status: NewsletterStatus::Voting,
        vote_window_open_at: now - SignedDuration::from_hours(30 * 24),
        vote_window_close_at: now - SignedDuration::from_mins(1),
        response_open_at: now - SignedDuration::from_mins(1),
        response_close_at: now + SignedDuration::from_hours(7 * 24),
        published_at: None,
        next_transition_at: Some(now - SignedDuration::from_mins(1)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
    };
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    run_tick(&repo, CycleCadence::Monthly).await;

    let opened = newsletters::get_newsletter(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(opened.locked_question_ids.len(), 1);

    let locked = questions::list_locked_questions(&repo, &voting.group_id, &voting.cycle_id)
        .await
        .unwrap();
    assert_eq!(locked.len(), 1);
    assert!(locked[0].is_standing());
    assert_eq!(locked[0].display_order, 0);
    assert_eq!(locked[0].prompt, shared::config::STANDING_QUESTION_PROMPT);
}
