//! Business logic for the push routes (`plans/03-api-contract.md` §10).

use crate::state::AppState;
use crate::validation;
use crate::{delivery, metrics, payload};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::Utc;
use domain::api::{
    PushDeliveryOutcome, PushPreferenceListResponse, PushPreferenceResponse, PushSubscribeRequest,
    PushSubscribeResponse, PushSubscriptionListResponse, PushSubscriptionResponse,
    PushTestResponse, PushTestResult, PushUnsubscribeRequest, PutPushPreferenceRequest,
};
use domain::{ApiError, GroupId, NotificationPref, PushSubscription, SubscriptionId, UserId};
use persistence::{auth, groups, push as push_repo};
use sha2::{Digest, Sha256};
use shared::config::PUSH_TEST_SEND_CUTOFF_SECS;
use std::time::{Duration, Instant};

/// `POST /push/subscribe` (§10.1). Upserts by `(userId, endpointHash)`.
pub async fn subscribe(
    state: &AppState,
    caller: &UserId,
    request: PushSubscribeRequest,
) -> Result<PushSubscribeResponse, ApiError> {
    let endpoint = validation::endpoint(&request.endpoint)?;
    let p256dh = validation::p256dh(&request.keys.p256dh)?;
    let auth_secret = validation::auth(&request.keys.auth)?;
    let user_agent = validation::user_agent(request.user_agent.as_deref())?;

    let endpoint_hash = hash_endpoint(&endpoint);
    push_repo::upsert_subscription(
        &state.repo,
        caller,
        &endpoint_hash,
        &endpoint,
        &p256dh,
        &auth_secret,
        &user_agent,
        Utc::now(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, user_id = %caller, "push subscribe failed");
        ApiError::internal("failed to save push subscription")
    })?;

    Ok(PushSubscribeResponse {
        subscription_id: SubscriptionId::new(endpoint_hash),
    })
}

/// `POST /push/unsubscribe` (§10.2). Idempotent.
pub async fn unsubscribe(
    state: &AppState,
    caller: &UserId,
    request: PushUnsubscribeRequest,
) -> Result<(), ApiError> {
    let endpoint = validation::endpoint(&request.endpoint)?;
    let endpoint_hash = hash_endpoint(&endpoint);
    push_repo::delete_subscription(&state.repo, caller, &endpoint_hash)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "push unsubscribe failed");
            ApiError::internal("failed to remove push subscription")
        })
}

/// `GET /push/subscriptions` (§10.3). Newest `createdAt` first; never
/// returns the encryption keys.
pub async fn list_subscriptions(
    state: &AppState,
    caller: &UserId,
) -> Result<PushSubscriptionListResponse, ApiError> {
    let mut subs = push_repo::list_subs_for_user(&state.repo, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "push subscription listing failed");
            ApiError::internal("failed to list push subscriptions")
        })?;
    subs.sort_by_key(|s| std::cmp::Reverse(s.created_at));
    Ok(PushSubscriptionListResponse {
        items: subs.into_iter().map(to_subscription_response).collect(),
    })
}

/// `POST /push/test` (§10.4, `07-notifications.md` §9). A caller with no
/// subscriptions gets an empty list. Sends to every device concurrently so
/// the reply lands inside API Gateway's 30s limit even when several
/// endpoints hang; a device not attempted before the cutoff reports
/// `failed` with no status code.
pub async fn send_test(state: &AppState, caller: &UserId) -> Result<PushTestResponse, ApiError> {
    let cutoff = Instant::now() + Duration::from_secs(PUSH_TEST_SEND_CUTOFF_SECS);
    let subs = push_repo::list_subs_for_user(&state.repo, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "push subscription listing failed");
            ApiError::internal("failed to list push subscriptions")
        })?;
    if subs.is_empty() {
        return Ok(PushTestResponse {
            results: Vec::new(),
        });
    }

    let vapid = state.vapid_keys().await?;
    let payload = payload::test();
    let deliveries: Vec<_> = subs
        .iter()
        .map(|sub| delivery::Delivery {
            user_id: caller,
            sub,
            payload: &payload,
        })
        .collect();
    let sent = delivery::send_all(
        &state.repo,
        state.sender.as_ref(),
        vapid,
        &deliveries,
        cutoff,
    )
    .await;

    let mut fanout_metrics = metrics::FanoutMetrics::new("test");
    fanout_metrics.record_all(&sent);
    fanout_metrics.emit();

    let results = subs
        .into_iter()
        .zip(sent)
        .map(|(sub, result)| PushTestResult {
            subscription_id: SubscriptionId::new(sub.endpoint_hash),
            user_agent: sub.user_agent,
            outcome: result
                .as_ref()
                .map_or(PushDeliveryOutcome::Failed, |r| to_api_outcome(r.outcome)),
            status_code: result.and_then(|r| r.status_code),
        })
        .collect();
    Ok(PushTestResponse { results })
}

/// `GET /push/preferences` (§10, added M11). One entry per membership, in
/// `GET /config` membership order; a group with no stored
/// `NotificationPref` row reports the defaults (both true).
pub async fn list_preferences(
    state: &AppState,
    caller: &UserId,
) -> Result<PushPreferenceListResponse, ApiError> {
    let memberships = groups::list_memberships_for_user(&state.repo, caller)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %caller, "membership listing failed");
            ApiError::internal("failed to list memberships")
        })?;

    let mut items = Vec::with_capacity(memberships.len());
    for membership in memberships {
        let pref = push_repo::get_pref(&state.repo, caller, &membership.group_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, user_id = %caller, group_id = %membership.group_id, "notification pref lookup failed");
                ApiError::internal("failed to load notification preference")
            })?;
        items.push(match pref {
            Some(p) => PushPreferenceResponse {
                group_id: membership.group_id,
                cycle_open: p.cycle_open,
                deadline_reminders: p.deadline_reminders,
            },
            None => PushPreferenceResponse {
                group_id: membership.group_id,
                cycle_open: true,
                deadline_reminders: true,
            },
        });
    }
    Ok(PushPreferenceListResponse { items })
}

/// `PUT /push/preferences/{groupId}` (§10.5). Non-member -> 403.
pub async fn put_preference(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    request: PutPushPreferenceRequest,
) -> Result<PushPreferenceResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;

    let pref = NotificationPref {
        user_id: caller.clone(),
        group_id: group_id.clone(),
        cycle_open: request.cycle_open,
        deadline_reminders: request.deadline_reminders,
    };
    push_repo::put_pref(&state.repo, &pref).await.map_err(|e| {
        tracing::error!(error = ?e, user_id = %caller, group_id = %group_id, "notification pref save failed");
        ApiError::internal("failed to save notification preference")
    })?;

    Ok(PushPreferenceResponse {
        group_id: group_id.clone(),
        cycle_open: pref.cycle_open,
        deadline_reminders: pref.deadline_reminders,
    })
}

fn hash_endpoint(endpoint: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(endpoint.as_bytes()))
}

fn to_subscription_response(s: PushSubscription) -> PushSubscriptionResponse {
    PushSubscriptionResponse {
        subscription_id: SubscriptionId::new(s.endpoint_hash),
        endpoint: s.endpoint,
        user_agent: s.user_agent,
        created_at: s.created_at,
        last_success_at: s.last_success_at,
        failure_count: s.failure_count,
    }
}

fn to_api_outcome(outcome: delivery::DeliveryOutcome) -> PushDeliveryOutcome {
    match outcome {
        delivery::DeliveryOutcome::Delivered => PushDeliveryOutcome::Delivered,
        delivery::DeliveryOutcome::Expired => PushDeliveryOutcome::Expired,
        delivery::DeliveryOutcome::Failed => PushDeliveryOutcome::Failed,
    }
}
