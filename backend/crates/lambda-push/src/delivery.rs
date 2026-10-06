//! Send one push message end-to-end — encrypt, sign, POST — and apply the
//! delivery-outcome bookkeeping from `plans/07-notifications.md` §3.2 (M11
//! decision D7).

use crate::payload::PushPayload;
use crate::sender::{PushSender, SendResult};
use crate::webpush::{self, VapidKeys};
use chrono::Utc;
use domain::{PushSubscription, UserId};
use persistence::{push as push_repo, Repo};
use shared::config::PUSH_TTL_SECONDS;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Delivered,
    Expired,
    Failed,
}

#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub outcome: DeliveryOutcome,
    pub status_code: Option<u16>,
    pub latency_ms: u64,
}

/// Encrypts `payload` for `sub`, signs it with VAPID, POSTs it via `sender`,
/// and applies the §3.2 bookkeeping to the subscription row. Never returns
/// an `Err` — a local failure (bad key material, serialization) is reported
/// as outcome `Failed` with no status code, same as a network error.
pub async fn send_one(
    repo: &Repo,
    sender: &dyn PushSender,
    vapid: &VapidKeys,
    user_id: &UserId,
    sub: &PushSubscription,
    payload: &PushPayload,
) -> DeliveryResult {
    let started = Instant::now();

    let body_bytes = match serde_json::to_vec(payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = ?e, user_id = %user_id, "failed to serialize push payload");
            record_failure(repo, user_id, &sub.endpoint_hash).await;
            return failed_locally(started);
        }
    };

    let encrypted = match webpush::encrypt(&body_bytes, &sub.p256dh, &sub.auth) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = ?e, user_id = %user_id, "push encryption failed");
            record_failure(repo, user_id, &sub.endpoint_hash).await;
            return failed_locally(started);
        }
    };

    let auth_header = match webpush::vapid_authorization_header(vapid, &sub.endpoint, Utc::now()) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = ?e, user_id = %user_id, "vapid header build failed");
            record_failure(repo, user_id, &sub.endpoint_hash).await;
            return failed_locally(started);
        }
    };

    let headers = [
        ("Authorization".to_owned(), auth_header),
        ("TTL".to_owned(), PUSH_TTL_SECONDS.to_string()),
        ("Urgency".to_owned(), "normal".to_owned()),
        (
            "Content-Type".to_owned(),
            "application/octet-stream".to_owned(),
        ),
        ("Content-Encoding".to_owned(), "aes128gcm".to_owned()),
    ];

    let result = sender.send(&sub.endpoint, &headers, encrypted).await;
    let latency_ms = started.elapsed().as_millis() as u64;

    let (outcome, status_code) = match result {
        SendResult::Status(code) if (200..300).contains(&code) => {
            if let Err(e) =
                push_repo::mark_delivered(repo, user_id, &sub.endpoint_hash, Utc::now()).await
            {
                tracing::error!(error = ?e, user_id = %user_id, "failed to record push delivery");
            }
            (DeliveryOutcome::Delivered, Some(code))
        }
        SendResult::Status(code @ (404 | 410)) => {
            if let Err(e) = push_repo::mark_expired(repo, user_id, &sub.endpoint_hash).await {
                tracing::error!(error = ?e, user_id = %user_id, "failed to delete expired subscription");
            }
            (DeliveryOutcome::Expired, Some(code))
        }
        SendResult::Status(code) => {
            record_failure(repo, user_id, &sub.endpoint_hash).await;
            (DeliveryOutcome::Failed, Some(code))
        }
        SendResult::NetworkError => {
            record_failure(repo, user_id, &sub.endpoint_hash).await;
            (DeliveryOutcome::Failed, None)
        }
    };

    DeliveryResult {
        outcome,
        status_code,
        latency_ms,
    }
}

async fn record_failure(repo: &Repo, user_id: &UserId, endpoint_hash: &str) {
    if let Err(e) = push_repo::record_failure(repo, user_id, endpoint_hash).await {
        tracing::error!(error = ?e, user_id = %user_id, "failed to record push failure");
    }
}

fn failed_locally(started: Instant) -> DeliveryResult {
    DeliveryResult {
        outcome: DeliveryOutcome::Failed,
        status_code: None,
        latency_ms: started.elapsed().as_millis() as u64,
    }
}
