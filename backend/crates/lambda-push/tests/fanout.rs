//! `cycle_open_fanout` / `publication_fanout` — the internal fan-outs
//! `lambda-cycle-tick` triggers (`plans/07-notifications.md` §6, §8).

mod common;

use chrono::{Duration, Utc};
use domain::{
    CycleId, GroupId, LockedQuestion, NotificationPref, QuestionId, QuestionKind, Role, UserId,
};
use pretty_assertions::assert_eq;
use push::fanout;
use std::sync::Arc;

fn locked_text(group_id: &str, cycle_id: &str, question_id: &str) -> LockedQuestion {
    common::locked_question(group_id, cycle_id, question_id, "u1")
}

fn locked_poll(group_id: &str, cycle_id: &str, question_id: &str) -> LockedQuestion {
    let mut lq = common::locked_question(group_id, cycle_id, question_id, "u1");
    lq.kind = QuestionKind::Poll;
    lq
}

#[tokio::test]
async fn cycle_open_fanout_respects_prefs_zero_subs_and_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let group_id = GroupId::new("gfan1");
    let cycle_id = CycleId::new("202610");

    let group = common::group("gfan1", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&state.repo, &group, "u2", Role::Member).await;
    common::seed_group_and_membership(&state.repo, &group, "u3", Role::Member).await;
    common::put_user(&state.repo, "u1").await;
    common::put_user(&state.repo, "u2").await;
    common::put_user(&state.repo, "u3").await;

    let open_at = Utc::now();
    let close_at = open_at + Duration::days(4);
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gfan1",
            "202610",
            open_at,
            close_at,
            vec![QuestionId::new("q1"), QuestionId::new("q2")],
        ),
    )
    .await;

    // u1: default pref (true) + one subscription -> sent.
    let (p1, a1) = common::make_subscriber_keys();
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        "https://example.com/u1",
        &p1,
        &a1,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;

    // u2: explicit cycleOpen=false -> skipped even though subscribed.
    let (p2, a2) = common::make_subscriber_keys();
    common::put_subscription(
        &state.repo,
        "u2",
        "h2",
        "https://example.com/u2",
        &p2,
        &a2,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;
    persistence::push::put_pref(
        &state.repo,
        &NotificationPref {
            user_id: UserId::new("u2"),
            group_id: group_id.clone(),
            cycle_open: false,
            deadline_reminders: true,
        },
    )
    .await
    .unwrap();

    // u3: zero subscriptions -> skipped.

    fanout::cycle_open_fanout(&state, &group_id, &cycle_id).await;
    assert_eq!(sender.call_count(), 1, "only u1's device gets the push");
    assert_eq!(sender.calls(), vec!["https://example.com/u1".to_owned()]);

    // Second invoke: the `NOTIFIED#OPEN` marker is already claimed, so
    // nothing is sent again.
    fanout::cycle_open_fanout(&state, &group_id, &cycle_id).await;
    assert_eq!(
        sender.call_count(),
        1,
        "idempotent: a second invoke sends nothing more"
    );
}

#[tokio::test]
async fn cycle_open_fanout_is_skipped_entirely_when_the_group_switch_is_off() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let group_id = GroupId::new("gfan2");
    let cycle_id = CycleId::new("202610");

    let mut group = common::group("gfan2", "u1");
    group.notification_settings.on_cycle_open = false;
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Admin).await;
    common::put_user(&state.repo, "u1").await;

    let open_at = Utc::now();
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gfan2",
            "202610",
            open_at,
            open_at + Duration::days(4),
            vec![QuestionId::new("q1")],
        ),
    )
    .await;

    let (p1, a1) = common::make_subscriber_keys();
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        "https://example.com/u1",
        &p1,
        &a1,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;

    fanout::cycle_open_fanout(&state, &group_id, &cycle_id).await;
    assert_eq!(
        sender.call_count(),
        0,
        "group has cycle-open pushes disabled"
    );
}

#[tokio::test]
async fn publication_fanout_sends_to_every_subscribed_member_regardless_of_pref() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let group_id = GroupId::new("gfan3");
    let cycle_id = CycleId::new("202610");

    let group = common::group("gfan3", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&state.repo, &group, "u2", Role::Member).await;
    common::put_user(&state.repo, "u1").await;
    common::put_user(&state.repo, "u2").await;

    common::put_newsletter(
        &state.repo,
        &common::published_newsletter("gfan3", "202610"),
    )
    .await;
    common::put_locked_question(&state.repo, &locked_text("gfan3", "202610", "q1")).await;
    common::put_locked_question(&state.repo, &locked_poll("gfan3", "202610", "q2")).await;
    common::put_published_text_answer(
        &state.repo,
        "gfan3",
        "202610",
        "q1",
        "u1",
        "r1",
        "Answer one",
    )
    .await;
    common::put_published_text_answer(
        &state.repo,
        "gfan3",
        "202610",
        "q1",
        "u2",
        "r2",
        "Answer two",
    )
    .await;

    // u2 has explicitly turned off cycleOpen/deadlineReminders — publication
    // has no toggle at all, so this must not matter.
    persistence::push::put_pref(
        &state.repo,
        &NotificationPref {
            user_id: UserId::new("u2"),
            group_id: group_id.clone(),
            cycle_open: false,
            deadline_reminders: false,
        },
    )
    .await
    .unwrap();

    let (p1, a1) = common::make_subscriber_keys();
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        "https://example.com/u1",
        &p1,
        &a1,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;
    let (p2, a2) = common::make_subscriber_keys();
    common::put_subscription(
        &state.repo,
        "u2",
        "h2",
        "https://example.com/u2",
        &p2,
        &a2,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;

    fanout::publication_fanout(&state, &group_id, &cycle_id).await;

    let mut calls = sender.calls();
    calls.sort();
    assert_eq!(
        calls,
        vec![
            "https://example.com/u1".to_owned(),
            "https://example.com/u2".to_owned()
        ]
    );

    // Idempotent via `NOTIFIED#PUBLISH`.
    fanout::publication_fanout(&state, &group_id, &cycle_id).await;
    assert_eq!(sender.call_count(), 2, "a second invoke sends nothing more");
}
