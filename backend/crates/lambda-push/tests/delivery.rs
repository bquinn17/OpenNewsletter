//! `delivery::send_all` — concurrent sends and the send cutoff that keeps
//! a fan-out inside its Lambda / API Gateway time limit
//! (`plans/07-notifications.md` §6.2).

mod common;

use async_trait::async_trait;
use chrono::Utc;
use domain::{PushSubscription, UserId};
use pretty_assertions::assert_eq;
use push::delivery::{self, Delivery, DeliveryOutcome};
use push::payload;
use push::sender::{PushSender, SendResult};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A push service that answers 201 after a fixed delay, like a slow (but
/// alive) endpoint.
struct SlowSender {
    delay: Duration,
    calls: AtomicUsize,
}

#[async_trait]
impl PushSender for SlowSender {
    async fn send(
        &self,
        _endpoint: &str,
        _headers: &[(String, String)],
        _body: Vec<u8>,
    ) -> SendResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.delay).await;
        SendResult::Status(201)
    }
}

async fn seed_subs(repo: &persistence::Repo, user_id: &str, n: usize) -> Vec<PushSubscription> {
    let mut subs = Vec::with_capacity(n);
    for i in 0..n {
        let (p256dh, auth) = common::make_subscriber_keys();
        let hash = format!("h{i}");
        let endpoint = format!("https://example.com/{user_id}/{i}");
        common::put_subscription(
            repo,
            user_id,
            &hash,
            &endpoint,
            &p256dh,
            &auth,
            "UA",
            Utc::now(),
            None,
            0,
        )
        .await;
        subs.push(PushSubscription {
            user_id: UserId::new(user_id),
            endpoint_hash: hash,
            endpoint,
            p256dh,
            auth,
            created_at: Utc::now(),
            last_success_at: None,
            failure_count: 0,
            user_agent: "UA".to_owned(),
        });
    }
    subs
}

#[tokio::test]
async fn send_all_sends_concurrently_and_reports_in_input_order() {
    let (_c, repo) = common::make_repo().await;
    let user_id = UserId::new("udeliv1");
    let subs = seed_subs(&repo, "udeliv1", 20).await;
    let sender = SlowSender {
        delay: Duration::from_millis(300),
        calls: AtomicUsize::new(0),
    };
    let vapid = common::make_test_vapid_keys();
    let message = payload::test();
    let deliveries: Vec<_> = subs
        .iter()
        .map(|sub| Delivery {
            user_id: &user_id,
            sub,
            payload: &message,
        })
        .collect();

    let started = Instant::now();
    let results = delivery::send_all(
        &repo,
        &sender,
        &vapid,
        &deliveries,
        Instant::now() + Duration::from_secs(60),
    )
    .await;

    // Sequentially this is 20 × 300ms = 6s; concurrently it's two waves.
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "sends ran sequentially: {:?}",
        started.elapsed()
    );
    assert_eq!(sender.calls.load(Ordering::SeqCst), 20);
    assert_eq!(results.len(), 20);
    assert!(results
        .iter()
        .all(|r| r.as_ref().map(|r| r.outcome) == Some(DeliveryOutcome::Delivered)));
}

#[tokio::test]
async fn send_all_starts_nothing_after_the_cutoff() {
    let (_c, repo) = common::make_repo().await;
    let user_id = UserId::new("udeliv2");
    let subs = seed_subs(&repo, "udeliv2", 3).await;
    let sender = SlowSender {
        delay: Duration::ZERO,
        calls: AtomicUsize::new(0),
    };
    let vapid = common::make_test_vapid_keys();
    let message = payload::test();
    let deliveries: Vec<_> = subs
        .iter()
        .map(|sub| Delivery {
            user_id: &user_id,
            sub,
            payload: &message,
        })
        .collect();

    let results = delivery::send_all(&repo, &sender, &vapid, &deliveries, Instant::now()).await;

    assert_eq!(sender.calls.load(Ordering::SeqCst), 0);
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(Option::is_none));
}
