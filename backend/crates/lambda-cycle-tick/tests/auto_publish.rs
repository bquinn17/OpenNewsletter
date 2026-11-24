//! `publish(nl)` auto-publishes non-empty drafts before flipping the cycle
//! (`docs/design/lifecycle.md` §5.3, `docs/design/data-model.md` §4 #4a).
//!
//! Each test gets its own table in the shared DynamoDB Local, and the tick
//! acts on every due cycle in its table, so tests can't see each other's cycles.

mod common;

use common::Answer;
use cycle_tick::tick::{auto_publish_drafts, run_tick};
use domain::{NewsletterStatus, ResponseStatus, UserId};
use persistence::responses::{self, AutoPublishOutcome};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

const TEXT_Q: &str = "q-text";
const POLL_Q: &str = "q-poll";

#[tokio::test]
async fn the_tick_publishes_non_empty_drafts_at_close_and_leaves_empty_ones() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-auto", &[TEXT_Q, POLL_Q]).await;
    for user in ["u1", "u2", "u3", "u4", "u5"] {
        common::put_membership(&repo, user, "g-auto").await;
    }
    let text = |body| Answer::Text { body, images: &[] };
    // u1: a text draft and a poll draft.
    common::save(&repo, &nl, "u1", TEXT_Q, text("Lake 22."), false).await;
    common::save(&repo, &nl, "u1", POLL_Q, Answer::Poll("opt-a"), false).await;
    // u2: already published one answer, left the other as a draft.
    common::save(&repo, &nl, "u2", TEXT_Q, text("Mailbox Peak."), true).await;
    common::save(&repo, &nl, "u2", POLL_Q, Answer::Poll("opt-b"), false).await;
    // u3: only whitespace — an empty draft.
    common::save(&repo, &nl, "u3", TEXT_Q, text("  \n "), false).await;
    // u4: images but no words still counts as an answer.
    common::put_ready_image(&repo, &nl, "u4", TEXT_Q, "img-1").await;
    let images = Answer::Text {
        body: "",
        images: &["img-1"],
    };
    common::save(&repo, &nl, "u4", TEXT_Q, images, false).await;
    // u5: no words, and its only image was deleted after the last save, so
    // there's nothing left to publish.
    let gone = Answer::Text {
        body: "",
        images: &["img-gone"],
    };
    common::save(&repo, &nl, "u5", TEXT_Q, gone, false).await;
    let closed = common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    assert_eq!(
        common::status(&repo, &nl.group_id, &nl.cycle_id).await,
        NewsletterStatus::Published
    );
    for (user, question) in [
        ("u1", TEXT_Q),
        ("u1", POLL_Q),
        ("u2", POLL_Q),
        ("u4", TEXT_Q),
    ] {
        let answer = common::response(&repo, &nl, user, question).await;
        assert_eq!(
            answer.status,
            ResponseStatus::Published,
            "{user}/{question}"
        );
        assert_eq!(
            answer.published_at.map(common::seconds),
            Some(common::seconds(closed.response_close_at)),
            "{user}/{question} is published as of the close"
        );
    }
    for user in ["u3", "u5"] {
        let empty = common::response(&repo, &nl, user, TEXT_Q).await;
        assert_eq!(empty.status, ResponseStatus::Draft, "{user}");
        assert_eq!(empty.published_at, None, "{user}");
    }

    // u1 has two non-empty answers in the cycle (one published by this sweep,
    // one that was already a draft needing it too); still counted once.
    assert_eq!(common::editions_answered(&repo, "u1", "g-auto").await, 1);
    // u2 mixes an already-published answer with a draft the sweep publishes.
    assert_eq!(common::editions_answered(&repo, "u2", "g-auto").await, 1);
    assert_eq!(common::editions_answered(&repo, "u4", "g-auto").await, 1);
    // u3's only answer is whitespace; u5's only image was deleted — neither
    // ever becomes non-empty, so neither is counted.
    assert_eq!(common::editions_answered(&repo, "u3", "g-auto").await, 0);
    assert_eq!(common::editions_answered(&repo, "u5", "g-auto").await, 0);
}

/// A member who published an answer and then emptied it (a later autosave
/// with `publish=false`, which leaves a published row's content overwritten
/// but still published — `docs/design/lifecycle.md` §5.3) isn't counted:
/// `is_non_empty` is checked on the row as it stands at close, not at the
/// moment it was published.
#[tokio::test]
async fn a_published_answer_emptied_before_close_is_not_counted() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-emptied", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-emptied").await;
    let text = |body| Answer::Text { body, images: &[] };
    common::save(&repo, &nl, "u1", TEXT_Q, text("my answer"), true).await;
    common::save(&repo, &nl, "u1", TEXT_Q, text(""), false).await;
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    let answer = common::response(&repo, &nl, "u1", TEXT_Q).await;
    assert_eq!(answer.status, ResponseStatus::Published);
    assert_eq!(common::editions_answered(&repo, "u1", "g-emptied").await, 0);
}

#[tokio::test]
async fn auto_publish_counts_each_member_s_edition_exactly_once() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-count", &[TEXT_Q, POLL_Q]).await;
    for user in ["u1", "u2", "u3"] {
        common::put_membership(&repo, user, "g-count").await;
    }
    let text = |body| Answer::Text { body, images: &[] };
    common::save(&repo, &nl, "u1", TEXT_Q, text("Lake 22."), false).await;
    common::save(&repo, &nl, "u1", POLL_Q, Answer::Poll("opt-a"), false).await;
    common::save(&repo, &nl, "u2", TEXT_Q, text("Mailbox Peak."), true).await;
    common::save(&repo, &nl, "u2", POLL_Q, Answer::Poll("opt-b"), false).await;
    common::save(&repo, &nl, "u3", TEXT_Q, text(" "), false).await;
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    assert_eq!(common::editions_answered(&repo, "u1", "g-count").await, 1);
    assert_eq!(common::editions_answered(&repo, "u2", "g-count").await, 1);
    assert_eq!(common::editions_answered(&repo, "u3", "g-count").await, 0);
}

#[tokio::test]
async fn a_tick_that_crashed_mid_sweep_is_finished_by_the_next_without_double_counting() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-crash", &[TEXT_Q, POLL_Q]).await;
    for user in ["u1", "u2"] {
        common::put_membership(&repo, user, "g-crash").await;
    }
    let text = |body| Answer::Text { body, images: &[] };
    common::save(&repo, &nl, "u1", TEXT_Q, text("Lake 22."), false).await;
    common::save(&repo, &nl, "u1", POLL_Q, Answer::Poll("opt-a"), false).await;
    common::save(&repo, &nl, "u2", TEXT_Q, text("Mailbox Peak."), false).await;
    let closed = common::close_cycle(&repo, &nl).await;

    // First tick: got through one draft, then died before the rest and the flip.
    let first = common::response(&repo, &nl, "u1", TEXT_Q).await;
    let outcome = responses::auto_publish_draft(&repo, &first, closed.response_close_at)
        .await
        .unwrap();
    assert_eq!(outcome, AutoPublishOutcome::Published);
    // Second tick: a full sweep, then died before the flip.
    assert_eq!(auto_publish_drafts(&repo, &closed).await.published, 2);
    assert_eq!(
        common::status(&repo, &nl.group_id, &nl.cycle_id).await,
        NewsletterStatus::Open
    );

    // Third tick: finds the cycle still due and finishes it.
    run_tick(&repo, CycleCadence::Monthly).await;

    assert_eq!(
        common::status(&repo, &nl.group_id, &nl.cycle_id).await,
        NewsletterStatus::Published
    );
    for (user, question) in [("u1", TEXT_Q), ("u1", POLL_Q), ("u2", TEXT_Q)] {
        let answer = common::response(&repo, &nl, user, question).await;
        assert_eq!(
            answer.status,
            ResponseStatus::Published,
            "{user}/{question}"
        );
    }
    assert_eq!(common::editions_answered(&repo, "u1", "g-crash").await, 1);
    assert_eq!(common::editions_answered(&repo, "u2", "g-crash").await, 1);
}

#[tokio::test]
async fn a_departed_member_s_draft_publishes_and_counts_without_recreating_their_membership() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-left", &[TEXT_Q]).await;
    let text = Answer::Text {
        body: "Wrote this, then left.",
        images: &[],
    };
    common::save(&repo, &nl, "u-gone", TEXT_Q, text, false).await;
    common::close_cycle(&repo, &nl).await;

    run_tick(&repo, CycleCadence::Monthly).await;

    let answer = common::response(&repo, &nl, "u-gone", TEXT_Q).await;
    assert_eq!(answer.status, ResponseStatus::Published);
    // The count lands on `MemberStats`, which survives a leave, like every
    // other stat on a departed member's published answer.
    assert_eq!(
        common::editions_answered(&repo, "u-gone", "g-left").await,
        1
    );
    let membership =
        persistence::groups::get_membership(&repo, &UserId::new("u-gone"), &nl.group_id)
            .await
            .unwrap();
    assert_eq!(membership, None);
}

#[tokio::test]
async fn a_broken_draft_holds_the_cycle_for_a_few_ticks_then_is_skipped() {
    let (_c, repo) = common::make_test_repo().await;
    let nl = common::seed_open_cycle(&repo, "g-broken", &[TEXT_Q]).await;
    common::put_membership(&repo, "u1", "g-broken").await;
    let text = Answer::Text {
        body: "Lake 22.",
        images: &[],
    };
    common::save(&repo, &nl, "u1", TEXT_Q, text, false).await;
    // A row under the question that won't deserialize as a `Response`.
    common::put_unreadable_answer(&repo, &nl, TEXT_Q, "u-broken").await;
    common::close_cycle(&repo, &nl).await;

    let max = shared::config::AUTO_PUBLISH_MAX_FAILED_SWEEPS;
    for tick in 1..max {
        run_tick(&repo, CycleCadence::Monthly).await;
        assert_eq!(
            common::status(&repo, &nl.group_id, &nl.cycle_id).await,
            NewsletterStatus::Open,
            "tick {tick} retries rather than publishing"
        );
        // The good draft doesn't wait for the broken one.
        let good = common::response(&repo, &nl, "u1", TEXT_Q).await;
        assert_eq!(good.status, ResponseStatus::Published, "tick {tick}");
    }

    run_tick(&repo, CycleCadence::Monthly).await;

    assert_eq!(
        common::status(&repo, &nl.group_id, &nl.cycle_id).await,
        NewsletterStatus::Published
    );
    assert_eq!(common::editions_answered(&repo, "u1", "g-broken").await, 1);
}
