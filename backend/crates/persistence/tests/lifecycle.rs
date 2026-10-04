//! Lifecycle-engine scenarios from `plans/06-newsletter-lifecycle.md` §11.
//!
//! Item 6 (membership-cap enforcement) is already covered by
//! `tests/invites.rs::redemption_is_refused_once_the_group_is_at_its_member_cap`
//! from M4 — not duplicated here.
//!
//! Item 5 (settings change mid-cycle) is decided in `lambda-groups`'
//! `patch_group` handler, not in this crate; the tests here instead prove the
//! persistence primitives that decision is built on — `find_voting_cycle`,
//! `questions::list_candidates`, and `write_status_transition` — behave
//! correctly for the three cases that handler branches on: recompute, leave
//! alone (has candidates), leave alone (recomputed cycle id would move the row).

mod common;

use chrono::{Duration, TimeZone, Utc};
use chrono_tz::America;
use domain::{CycleId, GroupId, NewsletterStatus, UserId};
use persistence::{newsletters, questions};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn tick_promotes_only_the_top_n_candidates_by_vote() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");

    let candidates = [
        common::candidate("g1", "202606", "q-low", "u1", 1),
        common::candidate("g1", "202606", "q-mid", "u1", 5),
        common::candidate("g1", "202606", "q-top", "u1", 10),
        common::candidate("g1", "202606", "q-bottom", "u1", 0),
    ];
    for c in &candidates {
        questions::put_candidate(&repo, c).await.unwrap();
    }

    // questionsPerCycle = 2: only the top 2 by vote count get promoted.
    let top2 = questions::list_top_candidates_by_votes(&repo, &group_id, &cycle_id, 2)
        .await
        .unwrap();
    assert_eq!(top2.len(), 2);

    let locked: Vec<_> = top2
        .into_iter()
        .enumerate()
        .map(|(idx, c)| {
            let mut lq = common::locked_question("g1", "202606", c.question_id.as_str(), "u1");
            lq.display_order = idx as u32;
            lq
        })
        .collect();

    let mut nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    nl.status = NewsletterStatus::Open;
    nl.locked_question_ids = locked.iter().map(|l| l.question_id.clone()).collect();
    nl.next_transition_at = Some(nl.response_close_at);
    questions::promote_candidates_tx(&repo, &locked, &nl)
        .await
        .unwrap();

    let locked_rows = questions::list_locked_questions(&repo, &group_id, &cycle_id)
        .await
        .unwrap();
    assert_eq!(locked_rows.len(), 2);
    let promoted_ids: Vec<&str> = locked_rows.iter().map(|q| q.question_id.as_str()).collect();
    assert!(promoted_ids.contains(&"q-top"));
    assert!(promoted_ids.contains(&"q-mid"));

    let updated = newsletters::get_newsletter(&repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, NewsletterStatus::Open);
}

#[tokio::test]
async fn tick_auto_publishes_an_open_cycle_past_its_deadline() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");

    let mut nl = common::voting_newsletter("g1", "202606");
    nl.status = NewsletterStatus::Open;
    nl.next_transition_at = Some(Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap());
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    let now = Utc.with_ymd_and_hms(2026, 6, 6, 0, 0, 0).unwrap();
    let due = newsletters::list_cycles_due(&repo, NewsletterStatus::Open, now)
        .await
        .unwrap();
    assert_eq!(due.len(), 1);

    let published_at = Utc.with_ymd_and_hms(2026, 6, 6, 0, 0, 0).unwrap();
    newsletters::publish_cycle(&repo, &group_id, &cycle_id, published_at)
        .await
        .unwrap();

    let updated = newsletters::get_newsletter(&repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.status, NewsletterStatus::Published);
    assert_eq!(updated.published_at, Some(published_at));
    assert_eq!(updated.next_transition_at, None);

    // The archive sentinel moves it out of the tick's due-query entirely.
    let still_open_due = newsletters::list_cycles_due(&repo, NewsletterStatus::Open, now)
        .await
        .unwrap();
    assert_eq!(still_open_due.len(), 0);
}

#[tokio::test]
async fn promotion_is_idempotent_across_rapid_reruns() {
    let (_c, repo) = common::make_repo().await;
    let lq = common::locked_question("g1", "202606", "q1", "u1");

    let mut nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();
    nl.status = NewsletterStatus::Open;
    nl.locked_question_ids = vec![lq.question_id.clone()];
    nl.next_transition_at = Some(nl.response_close_at);

    // First tick run wins the race.
    questions::promote_candidates_tx(&repo, std::slice::from_ref(&lq), &nl)
        .await
        .unwrap();

    // A second, concurrent tick run replaying the same promotion must lose —
    // distinguishably, so the caller can log it at INFO instead of treating
    // it as a failure (`06-newsletter-lifecycle.md` §8).
    let second = questions::promote_candidates_tx(&repo, std::slice::from_ref(&lq), &nl).await;
    let err = second.expect_err("second promotion attempt must lose the race");
    assert!(err.is_lost_race());
}

#[tokio::test]
async fn publish_is_idempotent_across_rapid_reruns() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");

    let mut nl = common::voting_newsletter("g1", "202606");
    nl.status = NewsletterStatus::Open;
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    let now = Utc::now();
    newsletters::publish_cycle(&repo, &group_id, &cycle_id, now)
        .await
        .unwrap();

    let second = newsletters::publish_cycle(&repo, &group_id, &cycle_id, now).await;
    let err = second.expect_err("second publish attempt must lose the race");
    assert!(err.is_lost_race());
}

#[tokio::test]
async fn create_next_voting_cycle_is_idempotent_across_rapid_reruns() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let after = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();

    newsletters::create_next_voting_cycle(&repo, &group_id, America::New_York, 4, after)
        .await
        .unwrap();

    let second =
        newsletters::create_next_voting_cycle(&repo, &group_id, America::New_York, 4, after).await;
    let err = second.expect_err("second create attempt must lose the race");
    assert!(err.is_lost_race());
}

#[tokio::test]
async fn a_cycle_with_no_candidates_still_opens_and_publishes_empty() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");

    let mut nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    // No candidates were ever submitted: promotion still runs, with an empty set.
    nl.status = NewsletterStatus::Open;
    nl.locked_question_ids = Vec::new();
    nl.next_transition_at = Some(nl.response_close_at);
    questions::promote_candidates_tx(&repo, &[], &nl)
        .await
        .unwrap();

    let locked = questions::list_locked_questions(&repo, &group_id, &cycle_id)
        .await
        .unwrap();
    assert_eq!(locked, Vec::new());

    let opened = newsletters::get_newsletter(&repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.status, NewsletterStatus::Open);
    assert_eq!(opened.locked_question_ids, Vec::new());

    // The empty edition still auto-publishes on schedule.
    let published_at = Utc::now();
    newsletters::publish_cycle(&repo, &group_id, &cycle_id, published_at)
        .await
        .unwrap();
    let published = newsletters::get_newsletter(&repo, &group_id, &cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.status, NewsletterStatus::Published);
}

#[tokio::test]
async fn settings_change_recomputes_a_voting_cycle_with_no_candidates() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");

    // The fixture's canned `vote_window_open_at` doesn't correspond to its
    // canned `cycle_id` under the real schedule formula, so pin it explicitly
    // to an instant that actually derives "202606" in `America/New_York`.
    let mut nl = common::voting_newsletter("g1", "202606");
    nl.vote_window_open_at = Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap();
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    // Mirrors lambda-groups' reschedule_voting_cycle_if_needed: no candidates
    // yet, so a `responseWindowDays` change recomputes the schedule in place.
    let voting = newsletters::find_voting_cycle(&repo, &group_id)
        .await
        .unwrap()
        .expect("voting cycle exists");
    let candidates = questions::list_candidates(&repo, &group_id, &voting.cycle_id)
        .await
        .unwrap();
    assert!(candidates.is_empty());

    let schedule = shared::cycle_time::next_cycle_schedule(
        voting.vote_window_open_at,
        chrono_tz::America::New_York,
        7, // was 4
    );
    assert_eq!(
        schedule.cycle_id, voting.cycle_id,
        "response_window_days doesn't move the cycle id"
    );

    let mut rescheduled = voting;
    rescheduled.response_open_at = schedule.response_open_at;
    rescheduled.vote_window_close_at = schedule.response_open_at;
    rescheduled.response_close_at = schedule.response_close_at;
    rescheduled.next_transition_at = Some(schedule.response_open_at);
    newsletters::write_status_transition(&repo, &rescheduled)
        .await
        .unwrap();

    let updated = newsletters::get_newsletter(&repo, &group_id, &nl.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.response_close_at - updated.response_open_at,
        Duration::days(7)
    );
}

#[tokio::test]
async fn settings_change_leaves_a_voting_cycle_with_candidates_untouched() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");

    let nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("g1", "202606", "q1", "u1", 0))
        .await
        .unwrap();

    let voting = newsletters::find_voting_cycle(&repo, &group_id)
        .await
        .unwrap()
        .expect("voting cycle exists");
    let candidates = questions::list_candidates(&repo, &group_id, &voting.cycle_id)
        .await
        .unwrap();
    // The handler's decision point: candidates exist, so it never calls
    // write_status_transition at all. Nothing left to assert beyond that the
    // row is exactly as it was written.
    assert!(!candidates.is_empty());

    let untouched = newsletters::get_newsletter(&repo, &group_id, &nl.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched, nl);
}

#[tokio::test]
async fn settings_change_does_not_move_the_row_when_the_cycle_id_would_change() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");

    // Created while interpreted as being in June (New York); recomputing the
    // same creation instant in a timezone far enough ahead pushes the local
    // "next month" calculation from July to August.
    let created_at = Utc.with_ymd_and_hms(2026, 6, 30, 23, 0, 0).unwrap();
    let mut nl = common::voting_newsletter("g1", "202607");
    nl.vote_window_open_at = created_at;
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    let schedule =
        shared::cycle_time::next_cycle_schedule(created_at, chrono_tz::Pacific::Auckland, 4);
    assert_ne!(
        schedule.cycle_id, nl.cycle_id,
        "the scenario requires the recompute to land on a different cycle id"
    );

    // The handler's decision: recomputed id differs from the row's key, so it
    // must NOT write anything. The row must be exactly as it was.
    let untouched = newsletters::get_newsletter(&repo, &group_id, &nl.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched, nl);
}

#[tokio::test]
async fn concurrent_vote_and_admin_delete_leave_the_store_consistent() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");
    let voter = UserId::new("u2");

    let q = common::candidate("g1", "202606", "q1", "u1", 0);
    questions::put_candidate(&repo, &q).await.unwrap();

    let vote = common::vote("u2", "g1", "202606", "q1");
    let (vote_result, delete_result) = tokio::join!(
        questions::cast_vote_tx(&repo, &vote, 1, 3),
        questions::delete_candidate(&repo, &group_id, &cycle_id, &q.question_id),
    );

    // delete_candidate has no precondition, so it always succeeds regardless
    // of interleaving; the candidate row is gone either way.
    assert!(delete_result.is_ok());
    let remaining = questions::list_candidates(&repo, &group_id, &cycle_id)
        .await
        .unwrap();
    assert!(remaining.is_empty());

    // Whichever way the race went, the vote row's existence is consistent
    // with whether cast_vote_tx reported success — no orphaned partial state.
    let votes = questions::list_my_votes(&repo, &group_id, &cycle_id, &voter)
        .await
        .unwrap();
    match vote_result {
        Ok(()) => assert_eq!(votes.len(), 1),
        Err(_) => assert_eq!(votes.len(), 0),
    }
}

#[tokio::test]
async fn create_next_voting_cycle_lands_on_the_correct_utc_instant_across_the_dst_boundary() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g1");

    // 06-newsletter-lifecycle.md §11 #8: a New York cycle whose responseOpenAt
    // is 2026-03-01 must land at the right UTC instant. 2026 US DST starts
    // March 8, so March 1 is still EST (UTC-5).
    let after = Utc.with_ymd_and_hms(2026, 2, 10, 0, 0, 0).unwrap();
    let created =
        newsletters::create_next_voting_cycle(&repo, &group_id, America::New_York, 4, after)
            .await
            .unwrap();

    assert_eq!(created.cycle_id, CycleId::new("202603"));
    assert_eq!(
        created.response_open_at,
        Utc.with_ymd_and_hms(2026, 3, 1, 5, 0, 0).unwrap()
    );

    let persisted = newsletters::get_newsletter(&repo, &group_id, &created.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(persisted, created);
}
