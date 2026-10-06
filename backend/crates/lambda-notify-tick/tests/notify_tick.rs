//! `run_notify_tick` due/moot/smallest-only/marker logic (M11 decision D5,
//! `plans/07-notifications.md` §7), the "done" member rules, and the dev
//! `POST /admin/dev/tick/notify` route (`plans/03-api-contract.md` §11a.2).

mod common;

use chrono::{Duration, Utc};
use domain::{CycleId, GroupId, QuestionId, Role};
use notify_tick::{dispatch, tick};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn only_the_smallest_due_offset_is_sent_but_every_due_offset_is_marked() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone(), "dev");
    let group_id = GroupId::new("gnt1");
    let cycle_id = CycleId::new("202610");

    let group = common::group("gnt1", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;
    common::put_user(&state.repo, "u1").await;

    // Close in 20h, opened long ago: offsets 96/48/24 are all simultaneously
    // "due" (mimics a late tick / rewound deadline). Only 24 (the smallest)
    // should be sent.
    let now = Utc::now();
    let open_at = now - Duration::days(10);
    let close_at = now + Duration::hours(20);
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gnt1",
            "202610",
            open_at,
            close_at,
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
    )
    .await;

    let summary = tick::run_notify_tick(&state).await;
    let fanout = summary
        .fanouts
        .iter()
        .find(|f| f.group_id == group_id.to_string() && f.cycle_id == cycle_id.to_string())
        .expect("one fanout for this cycle");
    assert_eq!(
        fanout.offset_hours, 24,
        "only the smallest due offset is sent"
    );
    assert_eq!(sender.call_count(), 1);

    // The 96h and 48h markers were also written this tick, even though they
    // weren't sent — confirmed by claiming them again and getting `false`.
    assert!(
        !persistence::push::claim_close_notified_marker(
            &state.repo,
            &group_id,
            &cycle_id,
            96,
            Utc::now()
        )
        .await
        .unwrap(),
        "96h marker was already claimed this tick"
    );
    assert!(
        !persistence::push::claim_close_notified_marker(
            &state.repo,
            &group_id,
            &cycle_id,
            48,
            Utc::now()
        )
        .await
        .unwrap(),
        "48h marker was already claimed this tick"
    );

    // A second tick sends nothing further for this cycle.
    let second = tick::run_notify_tick(&state).await;
    assert!(
        !second
            .fanouts
            .iter()
            .any(|f| f.group_id == group_id.to_string() && f.cycle_id == cycle_id.to_string()),
        "a second tick reports nothing more for an already-handled cycle"
    );
    assert_eq!(sender.call_count(), 1, "and sends nothing more");
}

#[tokio::test]
async fn moot_offsets_are_marked_but_never_sent() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone(), "dev");
    let group_id = GroupId::new("gnt2");
    let cycle_id = CycleId::new("202610");

    let group = common::group("gnt2", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;
    common::put_user(&state.repo, "u1").await;

    // A 10h response window: every configured offset (96/48/24) would land
    // at or before the cycle even opened, so all three are moot.
    let now = Utc::now();
    let open_at = now;
    let close_at = now + Duration::hours(10);
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gnt2",
            "202610",
            open_at,
            close_at,
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
    )
    .await;

    let summary = tick::run_notify_tick(&state).await;
    assert!(
        !summary
            .fanouts
            .iter()
            .any(|f| f.group_id == group_id.to_string() && f.cycle_id == cycle_id.to_string()),
        "nothing is sent when every offset is moot"
    );
    assert_eq!(sender.call_count(), 0);

    assert!(
        !persistence::push::claim_close_notified_marker(
            &state.repo,
            &group_id,
            &cycle_id,
            24,
            Utc::now()
        )
        .await
        .unwrap(),
        "the moot 24h offset was still marked"
    );
}

#[tokio::test]
async fn a_done_member_is_skipped_when_the_sent_offset_is_the_smallest() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone(), "dev");

    let mut group = common::group("gnt3", "u1");
    group.notification_settings.offsets_hours_before_close = vec![24];
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&state.repo, &group, "u2", Role::Member).await;
    common::put_user(&state.repo, "u1").await;
    common::put_user(&state.repo, "u2").await;

    let now = Utc::now();
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gnt3",
            "202610",
            now - Duration::days(1),
            now + Duration::hours(10),
            vec![QuestionId::new("q1")],
        ),
    )
    .await;

    // u1 has published the cycle's only question — done. u2 has not.
    common::put_published_text_answer(&state.repo, "gnt3", "202610", "q1", "u1", "r1").await;

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
    )
    .await;

    tick::run_notify_tick(&state).await;

    let calls = sender.calls();
    assert!(
        !calls.contains(&"https://example.com/u1".to_owned()),
        "done member is skipped at the smallest offset"
    );
    assert!(
        calls.contains(&"https://example.com/u2".to_owned()),
        "not-done member still gets the reminder"
    );
}

#[tokio::test]
async fn a_done_member_still_gets_the_reminder_when_the_offset_is_not_the_smallest() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone(), "dev");

    let mut group = common::group("gnt4", "u1");
    group.notification_settings.offsets_hours_before_close = vec![48, 24];
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;
    common::put_user(&state.repo, "u1").await;

    let now = Utc::now();
    // Close in 40h: 48h is due (target = close-48h is in the past), 24h is
    // not yet due (target = close-24h is still in the future). So the sent
    // offset (48) is NOT the smallest configured offset (24).
    common::put_newsletter(
        &state.repo,
        &common::open_newsletter(
            "gnt4",
            "202610",
            now - Duration::days(10),
            now + Duration::hours(40),
            vec![QuestionId::new("q1")],
        ),
    )
    .await;
    common::put_published_text_answer(&state.repo, "gnt4", "202610", "q1", "u1", "r1").await;
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
    )
    .await;

    let summary = tick::run_notify_tick(&state).await;
    let fanout = summary
        .fanouts
        .iter()
        .find(|f| f.cycle_id == "202610")
        .expect("fanout");
    assert_eq!(
        fanout.offset_hours, 48,
        "the sent offset is not the group's smallest"
    );
    assert!(
        sender
            .calls()
            .contains(&"https://example.com/u1".to_owned()),
        "a done member still gets the reminder when it isn't the final one"
    );
}

#[tokio::test]
async fn dev_route_404s_outside_dev() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()), "prod");
    let event = dev_tick_event("sub-whoever");
    let response = dispatch::handle_http(&state, &event).await;
    assert_eq!(response["statusCode"], 404);
}

#[tokio::test]
async fn dev_route_403s_for_a_non_admin() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()), "dev");
    common::put_user(&state.repo, "u1").await;
    common::put_cognito_sub_lookup(&state.repo, "sub-u1", "u1").await;
    let group = common::group("gnt5", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;

    let response = dispatch::handle_http(&state, &dev_tick_event("sub-u1")).await;
    assert_eq!(response["statusCode"], 403);
}

#[tokio::test]
async fn dev_route_runs_the_tick_for_an_admin() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()), "dev");
    common::put_user(&state.repo, "u1").await;
    common::put_cognito_sub_lookup(&state.repo, "sub-u1", "u1").await;
    let group = common::group("gnt6", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Admin).await;

    let response = dispatch::handle_http(&state, &dev_tick_event("sub-u1")).await;
    assert_eq!(response["statusCode"], 200);
    let body: serde_json::Value = serde_json::from_str(response["body"].as_str().unwrap()).unwrap();
    assert!(body.get("fanouts").is_some());
}

fn dev_tick_event(sub: &str) -> serde_json::Value {
    json!({
        "version": "2.0",
        "rawPath": "/admin/dev/tick/notify",
        "requestContext": {
            "http": { "method": "POST", "path": "/admin/dev/tick/notify" },
            "authorizer": { "jwt": { "claims": { "sub": sub } } },
        },
    })
}
