//! `POST /push/subscribe`, `POST /push/unsubscribe`, `GET /push/subscriptions`,
//! `POST /push/test`, `GET /push/preferences`, `PUT /push/preferences/{groupId}`
//! (`plans/03-api-contract.md` §10).
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`.

mod common;

use chrono::{TimeZone, Utc};
use domain::api::{
    PushSubscribeRequest, PushSubscriptionKeys, PushUnsubscribeRequest, PutPushPreferenceRequest,
};
use domain::{ApiErrorCode, GroupId, Role, UserId};
use pretty_assertions::assert_eq;
use push::handlers;
use push::sender::SendResult;
use std::sync::Arc;

fn subscribe_request(
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: Option<&str>,
) -> PushSubscribeRequest {
    PushSubscribeRequest {
        endpoint: endpoint.to_owned(),
        expiration_time: None,
        keys: PushSubscriptionKeys {
            p256dh: p256dh.to_owned(),
            auth: auth.to_owned(),
        },
        user_agent: user_agent.map(str::to_owned),
    }
}

#[tokio::test]
async fn subscribe_rejects_a_non_https_endpoint() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let (p256dh, auth) = common::make_subscriber_keys();

    let err = handlers::subscribe(
        &state,
        &UserId::new("u1"),
        subscribe_request("http://example.com/push", &p256dh, &auth, None),
    )
    .await
    .expect_err("non-https endpoint");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "endpoint");
}

#[tokio::test]
async fn subscribe_rejects_a_malformed_p256dh() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let (_p256dh, auth) = common::make_subscriber_keys();

    let err = handlers::subscribe(
        &state,
        &UserId::new("u1"),
        subscribe_request("https://example.com/push", "not-base64url!!", &auth, None),
    )
    .await
    .expect_err("bad p256dh");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "keys.p256dh");
}

#[tokio::test]
async fn subscribe_defaults_a_missing_user_agent_to_empty_string() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let (p256dh, auth) = common::make_subscriber_keys();

    handlers::subscribe(
        &state,
        &UserId::new("u1"),
        subscribe_request("https://example.com/push/1", &p256dh, &auth, None),
    )
    .await
    .expect("subscribe succeeds");

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .expect("list succeeds");
    assert_eq!(listed.items[0].user_agent, "");
}

#[tokio::test]
async fn resubscribing_resets_failures_but_keeps_created_at_and_last_success_at() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let user_id = UserId::new("u1");
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";

    handlers::subscribe(
        &state,
        &user_id,
        subscribe_request(endpoint, &p256dh, &auth, None),
    )
    .await
    .expect("first subscribe");
    let first = &handlers::list_subscriptions(&state, &user_id)
        .await
        .unwrap()
        .items[0];
    let created_at = first.created_at;
    let endpoint_hash = first.subscription_id.0.clone();

    // Simulate a successful delivery, then a couple of failures.
    persistence::push::mark_delivered(&state.repo, &user_id, &endpoint_hash, Utc::now())
        .await
        .unwrap();
    persistence::push::record_failure(&state.repo, &user_id, &endpoint_hash)
        .await
        .unwrap();
    persistence::push::record_failure(&state.repo, &user_id, &endpoint_hash)
        .await
        .unwrap();
    let before_resubscribe = &handlers::list_subscriptions(&state, &user_id)
        .await
        .unwrap()
        .items[0];
    assert_eq!(before_resubscribe.failure_count, 2);
    let last_success_at = before_resubscribe
        .last_success_at
        .expect("marked delivered");

    // Re-subscribe with the same endpoint.
    handlers::subscribe(
        &state,
        &user_id,
        subscribe_request(endpoint, &p256dh, &auth, None),
    )
    .await
    .expect("resubscribe");

    let after = &handlers::list_subscriptions(&state, &user_id)
        .await
        .unwrap()
        .items[0];
    assert_eq!(after.failure_count, 0, "resubscribe resets failureCount");
    assert_eq!(
        after.created_at, created_at,
        "createdAt is sticky via if_not_exists"
    );
    assert_eq!(
        after.last_success_at,
        Some(last_success_at),
        "lastSuccessAt is left untouched by a resubscribe"
    );
}

#[tokio::test]
async fn subscribing_an_eleventh_endpoint_evicts_the_least_recently_successful() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let user_id = UserId::new("u1");
    let base = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();

    // 10 existing subscriptions. #0 has never succeeded (null lastSuccessAt
    // — the eviction target); the rest have increasing lastSuccessAt.
    for i in 0..10u32 {
        let (p256dh, auth) = common::make_subscriber_keys();
        let last_success_at = if i == 0 {
            None
        } else {
            Some(base + chrono::Duration::hours(i as i64))
        };
        common::put_subscription(
            &state.repo,
            "u1",
            &format!("hash-{i}"),
            &format!("https://example.com/push/{i}"),
            &p256dh,
            &auth,
            "",
            base,
            last_success_at,
            0,
        )
        .await;
    }

    let (p256dh, auth) = common::make_subscriber_keys();
    handlers::subscribe(
        &state,
        &user_id,
        subscribe_request("https://example.com/push/new", &p256dh, &auth, None),
    )
    .await
    .expect("11th subscribe succeeds");

    let listed = handlers::list_subscriptions(&state, &user_id)
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 10, "still at the cap");
    assert!(
        listed
            .items
            .iter()
            .all(|s| s.endpoint != "https://example.com/push/0"),
        "the never-succeeded subscription was evicted"
    );
    assert!(
        listed
            .items
            .iter()
            .any(|s| s.endpoint == "https://example.com/push/new"),
        "the new subscription was kept"
    );
}

#[tokio::test]
async fn unsubscribe_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let user_id = UserId::new("u1");
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";

    handlers::subscribe(
        &state,
        &user_id,
        subscribe_request(endpoint, &p256dh, &auth, None),
    )
    .await
    .unwrap();
    assert_eq!(
        handlers::list_subscriptions(&state, &user_id)
            .await
            .unwrap()
            .items
            .len(),
        1
    );

    handlers::unsubscribe(
        &state,
        &user_id,
        PushUnsubscribeRequest {
            endpoint: endpoint.to_owned(),
        },
    )
    .await
    .expect("first unsubscribe");
    assert_eq!(
        handlers::list_subscriptions(&state, &user_id)
            .await
            .unwrap()
            .items
            .len(),
        0
    );

    handlers::unsubscribe(
        &state,
        &user_id,
        PushUnsubscribeRequest {
            endpoint: endpoint.to_owned(),
        },
    )
    .await
    .expect("second unsubscribe on an already-absent row is still Ok");
}

#[tokio::test]
async fn list_subscriptions_never_returns_encryption_keys_and_orders_newest_first() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let (p256dh, auth) = common::make_subscriber_keys();
    let older = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let newer = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        "https://example.com/1",
        &p256dh,
        &auth,
        "UA1",
        older,
        None,
        0,
    )
    .await;
    common::put_subscription(
        &state.repo,
        "u1",
        "h2",
        "https://example.com/2",
        &p256dh,
        &auth,
        "UA2",
        newer,
        None,
        0,
    )
    .await;

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 2);
    assert_eq!(
        listed.items[0].endpoint, "https://example.com/2",
        "newest first"
    );
    assert_eq!(listed.items[1].endpoint, "https://example.com/1");
    // The response type structurally has no key fields — this is enforced
    // at compile time by `PushSubscriptionResponse` not declaring them; this
    // assertion documents that the fields we *do* return are the right ones.
    assert_eq!(listed.items[0].user_agent, "UA2");
}

#[tokio::test]
async fn test_push_delivered_resets_failures_and_sets_last_success_at() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        endpoint,
        &p256dh,
        &auth,
        "UA",
        Utc::now(),
        None,
        3,
    )
    .await;
    sender.script(endpoint, SendResult::Status(201));

    let response = handlers::send_test(&state, &UserId::new("u1"))
        .await
        .expect("test push");
    assert_eq!(response.results.len(), 1);
    assert_eq!(
        response.results[0].outcome,
        domain::api::PushDeliveryOutcome::Delivered
    );
    assert_eq!(response.results[0].status_code, Some(201));

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(listed.items[0].failure_count, 0);
    assert!(listed.items[0].last_success_at.is_some());
}

#[tokio::test]
async fn test_push_expired_deletes_the_subscription() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        endpoint,
        &p256dh,
        &auth,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;
    sender.script(endpoint, SendResult::Status(410));

    let response = handlers::send_test(&state, &UserId::new("u1"))
        .await
        .expect("test push");
    assert_eq!(
        response.results[0].outcome,
        domain::api::PushDeliveryOutcome::Expired
    );
    assert_eq!(response.results[0].status_code, Some(410));

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert!(listed.items.is_empty(), "expired subscription is deleted");
}

#[tokio::test]
async fn test_push_failure_increments_without_deleting_before_the_cap() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        endpoint,
        &p256dh,
        &auth,
        "UA",
        Utc::now(),
        None,
        0,
    )
    .await;
    sender.script(endpoint, SendResult::Status(500));

    let response = handlers::send_test(&state, &UserId::new("u1"))
        .await
        .expect("test push");
    assert_eq!(
        response.results[0].outcome,
        domain::api::PushDeliveryOutcome::Failed
    );
    assert_eq!(response.results[0].status_code, Some(500));

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1, "survives below the failure cap");
    assert_eq!(listed.items[0].failure_count, 1);
}

#[tokio::test]
async fn test_push_failure_deletes_the_subscription_at_the_cap() {
    let (_c, repo) = common::make_repo().await;
    let sender = Arc::new(common::FakePushSender::new());
    let state = common::test_state(repo, sender.clone());
    let (p256dh, auth) = common::make_subscriber_keys();
    let endpoint = "https://example.com/push/1";
    common::put_subscription(
        &state.repo,
        "u1",
        "h1",
        endpoint,
        &p256dh,
        &auth,
        "UA",
        Utc::now(),
        None,
        4,
    )
    .await;
    sender.script(endpoint, SendResult::Status(500));

    let response = handlers::send_test(&state, &UserId::new("u1"))
        .await
        .expect("test push");
    assert_eq!(
        response.results[0].outcome,
        domain::api::PushDeliveryOutcome::Failed
    );

    let listed = handlers::list_subscriptions(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert!(
        listed.items.is_empty(),
        "5th consecutive failure deletes the row"
    );
}

#[tokio::test]
async fn test_push_with_no_subscriptions_returns_an_empty_list() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let response = handlers::send_test(&state, &UserId::new("u1"))
        .await
        .expect("test push");
    assert!(response.results.is_empty());
}

#[tokio::test]
async fn preferences_default_to_true_for_groups_with_no_stored_pref() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let group = common::group("gpref1", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;

    let prefs = handlers::list_preferences(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(prefs.items.len(), 1);
    assert_eq!(prefs.items[0].group_id, GroupId::new("gpref1"));
    assert!(prefs.items[0].cycle_open);
    assert!(prefs.items[0].deadline_reminders);
}

#[tokio::test]
async fn preferences_reflect_a_stored_value() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let group = common::group("gpref2", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;

    handlers::put_preference(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpref2"),
        PutPushPreferenceRequest {
            cycle_open: false,
            deadline_reminders: true,
        },
    )
    .await
    .expect("put preference");

    let prefs = handlers::list_preferences(&state, &UserId::new("u1"))
        .await
        .unwrap();
    assert!(!prefs.items[0].cycle_open);
    assert!(prefs.items[0].deadline_reminders);
}

#[tokio::test]
async fn put_preference_is_forbidden_for_a_non_member() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo, Arc::new(common::FakePushSender::new()));
    let group = common::group("gpref3", "u1");
    common::seed_group_and_membership(&state.repo, &group, "u1", Role::Member).await;

    let err = handlers::put_preference(
        &state,
        &UserId::new("u2"),
        &GroupId::new("gpref3"),
        PutPushPreferenceRequest {
            cycle_open: false,
            deadline_reminders: false,
        },
    )
    .await
    .expect_err("u2 is not a member of gpref3");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[test]
fn put_preference_request_rejects_an_unknown_publication_field() {
    let raw = r#"{"cycleOpen":true,"deadlineReminders":true,"publication":false}"#;
    let result: Result<PutPushPreferenceRequest, _> = serde_json::from_str(raw);
    assert!(
        result.is_err(),
        "publication is not a real field and must be rejected"
    );
}
