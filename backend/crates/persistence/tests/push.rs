mod common;

use chrono::{Duration, TimeZone, Utc};
use domain::{CycleId, PushSubscription};
use domain::{GroupId, UserId};
use persistence::push;
use persistence::Repo;
use pretty_assertions::assert_eq;
use shared::config::{MAX_PUSH_FAILURES, MAX_PUSH_SUBSCRIPTIONS_PER_USER};

/// Subscribes `s` as of its own `created_at`.
async fn upsert(repo: &Repo, s: &PushSubscription) {
    push::upsert_subscription(
        repo,
        &s.user_id,
        &s.endpoint_hash,
        &s.endpoint,
        &s.p256dh,
        &s.auth,
        &s.user_agent,
        s.created_at,
    )
    .await
    .unwrap();
}

async fn subs_of(repo: &Repo, user_id: &str) -> Vec<PushSubscription> {
    let mut subs = push::list_subs_for_user(repo, &UserId::new(user_id))
        .await
        .unwrap();
    subs.sort_by(|a, b| a.endpoint_hash.cmp(&b.endpoint_hash));
    subs
}

#[tokio::test]
async fn it_subscribes_and_lists_push_subs() {
    let (_c, repo) = common::make_repo().await;
    let s1 = common::push_subscription("u1", "hash-aaa");
    let s2 = common::push_subscription("u1", "hash-bbb");
    let other = common::push_subscription("u2", "hash-ccc");
    upsert(&repo, &s1).await;
    upsert(&repo, &s2).await;
    upsert(&repo, &other).await;

    let mut subs = push::list_subs_for_user(&repo, &UserId::new("u1"))
        .await
        .unwrap();
    subs.sort_by(|a, b| a.endpoint_hash.cmp(&b.endpoint_hash));
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0], s1);
    assert_eq!(subs[1], s2);
}

#[tokio::test]
async fn it_deletes_a_push_subscription() {
    let (_c, repo) = common::make_repo().await;
    let s = common::push_subscription("u1", "hash-aaa");
    upsert(&repo, &s).await;

    push::delete_subscription(&repo, &UserId::new("u1"), "hash-aaa")
        .await
        .unwrap();

    let subs = push::list_subs_for_user(&repo, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(subs.len(), 0);
}

#[tokio::test]
async fn it_puts_and_gets_notification_pref() {
    let (_c, repo) = common::make_repo().await;
    let p = common::notification_pref("u1", "g1");
    push::put_pref(&repo, &p).await.unwrap();
    let got = push::get_pref(&repo, &UserId::new("u1"), &GroupId::new("g1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, p);
}

#[tokio::test]
async fn it_returns_none_for_missing_notification_pref() {
    let (_c, repo) = common::make_repo().await;
    let result = push::get_pref(&repo, &UserId::new("u1"), &GroupId::new("g1"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_resubscribe_keeps_created_at_and_last_success_but_resets_failures() {
    let (_c, repo) = common::make_repo().await;
    let s = common::push_subscription("u1", "hash-aaa");
    upsert(&repo, &s).await;
    let delivered_at = s.created_at + Duration::hours(1);
    push::mark_delivered(&repo, &s.user_id, "hash-aaa", delivered_at)
        .await
        .unwrap();
    push::record_failure(&repo, &s.user_id, "hash-aaa")
        .await
        .unwrap();

    let mut again = s.clone();
    again.created_at = s.created_at + Duration::days(3);
    again.p256dh = "rotated".into();
    upsert(&repo, &again).await;

    let subs = subs_of(&repo, "u1").await;
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].created_at, s.created_at);
    assert_eq!(subs[0].last_success_at, Some(delivered_at));
    assert_eq!(subs[0].failure_count, 0);
    assert_eq!(subs[0].p256dh, "rotated");
}

#[tokio::test]
async fn it_evicts_the_least_recently_successful_sub_at_the_cap() {
    let (_c, repo) = common::make_repo().await;
    let base = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    for i in 0..MAX_PUSH_SUBSCRIPTIONS_PER_USER {
        let mut s = common::push_subscription("u1", &format!("hash-{i:02}"));
        s.created_at = base + Duration::minutes(i as i64);
        upsert(&repo, &s).await;
        // Every sub but hash-05 has delivered at least once, so hash-05 is
        // the eviction victim despite not being the oldest.
        if i != 5 {
            push::mark_delivered(
                &repo,
                &s.user_id,
                &s.endpoint_hash,
                base + Duration::days(1),
            )
            .await
            .unwrap();
        }
    }

    // Re-subscribing an existing endpoint at the cap evicts nothing.
    upsert(&repo, &common::push_subscription("u1", "hash-00")).await;
    assert_eq!(
        subs_of(&repo, "u1").await.len(),
        MAX_PUSH_SUBSCRIPTIONS_PER_USER
    );

    upsert(&repo, &common::push_subscription("u1", "hash-new")).await;
    let hashes: Vec<String> = subs_of(&repo, "u1")
        .await
        .into_iter()
        .map(|s| s.endpoint_hash)
        .collect();
    assert_eq!(hashes.len(), MAX_PUSH_SUBSCRIPTIONS_PER_USER);
    assert!(!hashes.contains(&"hash-05".to_string()));
    assert!(hashes.contains(&"hash-new".to_string()));
}

#[tokio::test]
async fn it_deletes_a_sub_once_failures_reach_the_limit() {
    let (_c, repo) = common::make_repo().await;
    let s = common::push_subscription("u1", "hash-aaa");
    upsert(&repo, &s).await;
    for _ in 1..MAX_PUSH_FAILURES {
        let deleted = push::record_failure(&repo, &s.user_id, "hash-aaa")
            .await
            .unwrap();
        assert!(!deleted);
    }
    assert_eq!(
        subs_of(&repo, "u1").await[0].failure_count,
        MAX_PUSH_FAILURES - 1
    );
    let deleted = push::record_failure(&repo, &s.user_id, "hash-aaa")
        .await
        .unwrap();
    assert!(deleted);
    assert_eq!(subs_of(&repo, "u1").await.len(), 0);
}

#[tokio::test]
async fn it_never_resurrects_a_deleted_sub_from_bookkeeping() {
    let (_c, repo) = common::make_repo().await;
    let s = common::push_subscription("u1", "hash-aaa");
    upsert(&repo, &s).await;
    push::mark_expired(&repo, &s.user_id, "hash-aaa")
        .await
        .unwrap();

    push::mark_delivered(&repo, &s.user_id, "hash-aaa", s.created_at)
        .await
        .unwrap();
    let deleted = push::record_failure(&repo, &s.user_id, "hash-aaa")
        .await
        .unwrap();
    assert!(!deleted);
    assert_eq!(subs_of(&repo, "u1").await.len(), 0);
}

#[tokio::test]
async fn it_claims_each_notified_marker_at_most_once() {
    let (_c, repo) = common::make_repo().await;
    let g = GroupId::new("g-markers");
    let c = CycleId::new("202606");
    let now = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();

    assert!(push::claim_open_notified_marker(&repo, &g, &c, now)
        .await
        .unwrap());
    assert!(!push::claim_open_notified_marker(&repo, &g, &c, now)
        .await
        .unwrap());
    assert!(push::claim_publish_notified_marker(&repo, &g, &c, now)
        .await
        .unwrap());
    assert!(!push::claim_publish_notified_marker(&repo, &g, &c, now)
        .await
        .unwrap());
    assert!(push::claim_close_notified_marker(&repo, &g, &c, 48, now)
        .await
        .unwrap());
    assert!(push::claim_close_notified_marker(&repo, &g, &c, 24, now)
        .await
        .unwrap());
    assert!(!push::claim_close_notified_marker(&repo, &g, &c, 48, now)
        .await
        .unwrap());
}
