//! Streak settlement, `publish(nl)`'s step between the auto-publish sweep
//! and the status flip (`docs/design/lifecycle.md` §5.3,
//! `docs/design/data-model.md` §2.17).
//!
//! Each test gets its own table in the shared DynamoDB Local.

mod common;

use common::Answer;
use cycle_tick::tick::{auto_publish_drafts, run_tick, settle_streaks};
use domain::UserId;
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;
use std::collections::HashSet;

const TEXT_Q: &str = "q-text";

#[tokio::test]
async fn an_answered_member_extends_the_streak_and_raises_the_longest() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-streak-up", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-streak-up").await;
    // Already on a 3-streak with the same high-water mark, settled last cycle.
    common::seed_streak(&repo, "g-streak-up", "u1", 3, 3, "202605").await;
    common::save(
        &repo,
        &nl,
        "u1",
        TEXT_Q,
        Answer::Text {
            body: "Lake 22.",
            images: &[],
        },
        false,
    )
    .await;
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    let stats = common::member_stats(&repo, "g-streak-up", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(stats.current_streak, 4);
    assert_eq!(
        stats.longest_streak, 4,
        "a new high extends the longest streak too"
    );
    assert_eq!(stats.streak_cycle, Some(nl.cycle_id.clone()));
}

#[tokio::test]
async fn a_member_who_did_not_answer_has_their_streak_reset_but_keeps_the_longest() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-streak-reset", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-streak-reset").await;
    common::seed_streak(&repo, "g-streak-reset", "u1", 5, 5, "202605").await;
    // u1 answers nothing this cycle.
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    let stats = common::member_stats(&repo, "g-streak-reset", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(stats.current_streak, 0);
    assert_eq!(stats.longest_streak, 5, "longest_streak is never lowered");
    assert_eq!(stats.streak_cycle, Some(nl.cycle_id.clone()));
}

#[tokio::test]
async fn rerunning_settlement_for_the_same_cycle_does_not_double_count() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-streak-rerun", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-streak-rerun").await;
    common::save(
        &repo,
        &nl,
        "u1",
        TEXT_Q,
        Answer::Text {
            body: "Lake 22.",
            images: &[],
        },
        true,
    )
    .await;
    let closed = common::close_cycle(&repo, &nl).await;

    let mut authors = HashSet::new();
    authors.insert(UserId::new("u1"));

    // First pass settles the streak...
    settle_streaks(&repo, &closed, &authors).await;
    let after_first = common::member_stats(&repo, "g-streak-rerun", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(after_first.current_streak, 1);

    // ...a crashed-then-rerun pass over the SAME cycle must skip silently
    // rather than incrementing again.
    settle_streaks(&repo, &closed, &authors).await;
    let after_second = common::member_stats(&repo, "g-streak-rerun", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(
        after_second.current_streak, 1,
        "a rerun must not double count"
    );
}

#[tokio::test]
async fn an_empty_edition_settles_no_streaks() {
    let (_c, repo) = common::make_test_repo().await;
    // No locked questions at all.
    let nl = common::seed_open_cycle(&repo, "g-streak-empty", &[]).await;
    common::put_membership(&repo, "u1", "g-streak-empty").await;
    common::seed_streak(&repo, "g-streak-empty", "u1", 2, 2, "202605").await;
    let closed = common::close_cycle(&repo, &nl).await;

    settle_streaks(&repo, &closed, &HashSet::new()).await;

    let stats = common::member_stats(&repo, "g-streak-empty", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(
        stats.current_streak, 2,
        "an empty edition must neither extend nor break a streak"
    );
    assert_eq!(stats.streak_cycle, Some(domain::CycleId::new("202605")));
}

#[tokio::test]
async fn a_departed_member_is_ignored_by_settlement() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-streak-left", &[TEXT_Q]).await;
    // u-gone answered this edition before leaving the group, so it's in
    // `authors` — but it's never a current member
    // (`groups::list_members_for_group` never returns it), which is all
    // `settle_streaks` consults to decide who to settle.
    common::seed_streak(&repo, "g-streak-left", "u-gone", 2, 2, "202605").await;
    let closed = common::close_cycle(&repo, &nl).await;

    let mut authors = HashSet::new();
    authors.insert(UserId::new("u-gone"));
    settle_streaks(&repo, &closed, &authors).await;

    let stats = common::member_stats(&repo, "g-streak-left", "u-gone")
        .await
        .expect("u-gone keeps its stats row");
    assert_eq!(
        stats.current_streak, 2,
        "a departed member's streak is untouched by settlement"
    );
    assert_eq!(
        stats.streak_cycle,
        Some(domain::CycleId::new("202605")),
        "settlement never stamped a departed member's row for this cycle"
    );
}

#[tokio::test]
async fn auto_publish_drafts_collects_authors_for_settlement() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-streak-authors", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-streak-authors").await;
    common::put_membership(&repo, "u2", "g-streak-authors").await;
    // u1 already published; u2 leaves a non-empty draft the sweep publishes.
    common::save(
        &repo,
        &nl,
        "u1",
        TEXT_Q,
        Answer::Text {
            body: "Already published.",
            images: &[],
        },
        true,
    )
    .await;
    common::save(
        &repo,
        &nl,
        "u2",
        TEXT_Q,
        Answer::Text {
            body: "Still a draft.",
            images: &[],
        },
        false,
    )
    .await;
    let closed = common::close_cycle(&repo, &nl).await;

    let report = auto_publish_drafts(&repo, &closed).await;

    assert_eq!(
        report.authors,
        [UserId::new("u1"), UserId::new("u2")].into_iter().collect()
    );
}

#[tokio::test]
async fn a_streak_carries_across_a_switch_from_monthly_to_weekly_cycles() {
    // Streaks count consecutive published editions, not calendar months, so
    // a weekly edition straight after a monthly one extends the streak.
    let cadence = CycleCadence::Weekly {
        weekday: chrono::Weekday::Sun,
        at: chrono::NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
    };
    let (_c, repo) = common::make_test_repo().await;
    let nl =
        common::seed_open_cycle_with_cadence(&repo, "g-streak-weekly", &[TEXT_Q], cadence).await;
    assert_eq!(nl.cycle_id.as_str().len(), 8, "weekly ids are yyyymmdd");
    common::put_membership(&repo, "u1", "g-streak-weekly").await;
    common::seed_streak(&repo, "g-streak-weekly", "u1", 2, 2, "202610").await;
    common::save(
        &repo,
        &nl,
        "u1",
        TEXT_Q,
        Answer::Text {
            body: "Lake 22.",
            images: &[],
        },
        true,
    )
    .await;
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, cadence).await;

    let stats = common::member_stats(&repo, "g-streak-weekly", "u1")
        .await
        .expect("u1 has a stats row");
    assert_eq!(stats.current_streak, 3);
    assert_eq!(stats.longest_streak, 3);
    assert_eq!(stats.streak_cycle, Some(nl.cycle_id.clone()));
}
