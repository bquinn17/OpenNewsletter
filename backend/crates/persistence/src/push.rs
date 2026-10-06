//! PushSubscription, NotificationPref, and notification idempotency markers
//! (AP20, AP21; `plans/02-data-model-dynamodb.md` §2.13-§2.15).

use crate::error::{self, RepoError};
use crate::keys::{
    attr, notified_close_sk, notified_open_pk, npref_sk, push_sk, user_pk, NOTIFIED_OPEN_SK,
    NOTIFIED_PUBLISH_SK, TICK_NOTIFY_SK, TICK_PK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, ReturnValue};
use chrono::{DateTime, Utc};
use domain::{CycleId, GroupId, NotificationPref, PushSubscription, UserId};
use serde_dynamo::{from_item, to_item};
use shared::config::{MAX_PUSH_FAILURES, MAX_PUSH_SUBSCRIPTIONS_PER_USER};

/// AP20 — list all push subs for a user.
pub async fn list_subs_for_user(
    repo: &Repo,
    user_id: &UserId,
) -> Result<Vec<PushSubscription>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(user_pk(user_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("PUSH#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, PushSubscription>(i).map_err(RepoError::from))
        .collect()
}

/// `POST /push/subscribe` (`03-api-contract.md` §10.1, M11 decision D8).
/// Upserts by `(userId, endpointHash)`: `failureCount` always resets to 0,
/// `createdAt` is set once via `if_not_exists`, and `lastSuccessAt` is left
/// untouched (absent on a brand-new row, preserved on a re-subscribe).
///
/// Enforces [`MAX_PUSH_SUBSCRIPTIONS_PER_USER`]: when `endpoint_hash` is new
/// for this user and they are already at the cap, the least-recently-
/// successful existing subscription is evicted first (a `null`
/// `lastSuccessAt` counts as oldest; ties broken by oldest `createdAt`).
// Args mirror the subscribe route's own request fields plus `now`; bundling
// them into a struct wouldn't reduce real duplication (same reasoning as
// `persistence::responses::ResponseSave`).
#[allow(clippy::too_many_arguments)]
pub async fn upsert_subscription(
    repo: &Repo,
    user_id: &UserId,
    endpoint_hash: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: &str,
    now: DateTime<Utc>,
) -> Result<(), RepoError> {
    let existing = list_subs_for_user(repo, user_id).await?;
    let is_new_endpoint = !existing.iter().any(|s| s.endpoint_hash == endpoint_hash);
    if is_new_endpoint && existing.len() >= MAX_PUSH_SUBSCRIPTIONS_PER_USER {
        if let Some(victim) = existing.iter().min_by_key(|s| eviction_rank(s)) {
            delete_subscription(repo, user_id, &victim.endpoint_hash).await?;
        }
    }

    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(push_sk(endpoint_hash)))
        .update_expression(
            "SET #endpoint = :endpoint, #p256dh = :p256dh, #auth = :auth, #ua = :ua, \
             #user_id = :user_id, #hash = :hash, #entity = :entity, #fc = :zero, \
             #ca = if_not_exists(#ca, :now)",
        )
        .expression_attribute_names("#endpoint", "endpoint")
        .expression_attribute_names("#p256dh", "p256dh")
        .expression_attribute_names("#auth", "auth")
        .expression_attribute_names("#ua", "user_agent")
        .expression_attribute_names("#user_id", "user_id")
        .expression_attribute_names("#hash", "endpoint_hash")
        .expression_attribute_names("#entity", attr::ENTITY)
        .expression_attribute_names("#fc", "failure_count")
        .expression_attribute_names("#ca", "created_at")
        .expression_attribute_values(":endpoint", AttributeValue::S(endpoint.to_owned()))
        .expression_attribute_values(":p256dh", AttributeValue::S(p256dh.to_owned()))
        .expression_attribute_values(":auth", AttributeValue::S(auth.to_owned()))
        .expression_attribute_values(":ua", AttributeValue::S(user_agent.to_owned()))
        .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":hash", AttributeValue::S(endpoint_hash.to_owned()))
        .expression_attribute_values(":entity", AttributeValue::S("PushSubscription".into()))
        .expression_attribute_values(":zero", AttributeValue::N("0".into()))
        .expression_attribute_values(":now", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await?;
    Ok(())
}

/// Sort key for eviction: `(lastSuccessAt millis, createdAt millis)`, `None`
/// mapped to `i64::MIN` so a never-succeeded subscription always sorts as
/// the oldest.
fn eviction_rank(sub: &PushSubscription) -> (i64, i64) {
    let success_rank = sub
        .last_success_at
        .map(|t| t.timestamp_millis())
        .unwrap_or(i64::MIN);
    (success_rank, sub.created_at.timestamp_millis())
}

pub async fn delete_subscription(
    repo: &Repo,
    user_id: &UserId,
    endpoint_hash: &str,
) -> Result<(), RepoError> {
    repo.client
        .delete_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(push_sk(endpoint_hash)))
        .send()
        .await?;
    Ok(())
}

/// Delivery bookkeeping (`07-notifications.md` §3.2, M11 decision D7):
/// 200/201/202 resets `failureCount` to 0 and bumps `lastSuccessAt`,
/// conditioned on the row still existing. A lost condition — the row was
/// deleted concurrently, e.g. by its own failure path crossing the cap — is
/// not an error: never resurrect a row someone else deleted.
pub async fn mark_delivered(
    repo: &Repo,
    user_id: &UserId,
    endpoint_hash: &str,
    now: DateTime<Utc>,
) -> Result<(), RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(push_sk(endpoint_hash)))
        .update_expression("SET #fc = :zero, #lsa = :now")
        .condition_expression("attribute_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#fc", "failure_count")
        .expression_attribute_names("#lsa", "last_success_at")
        .expression_attribute_values(":zero", AttributeValue::N("0".into()))
        .expression_attribute_values(":now", AttributeValue::S(now.to_rfc3339()))
        .send()
        .await;
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let err = error::from_update_item_error(e);
            if err.is_lost_race() {
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

/// 404/410 (the push service says the endpoint is permanently gone) — delete
/// immediately. Same operation as [`delete_subscription`]; named separately
/// so call sites read as the delivery-outcome decision they are.
pub async fn mark_expired(
    repo: &Repo,
    user_id: &UserId,
    endpoint_hash: &str,
) -> Result<(), RepoError> {
    delete_subscription(repo, user_id, endpoint_hash).await
}

/// Everything else (413/429/other 4xx/5xx/network error) — increments
/// `failureCount`; once it reaches [`MAX_PUSH_FAILURES`] the row is deleted.
/// Returns `true` if this call deleted the row (the test-push / fan-out
/// caller reports that as outcome `failed`), `false` if it merely
/// incremented and the subscription survives. A lost condition (the row was
/// deleted concurrently) is treated as "already gone" and returns `false` —
/// the caller that raced us already reported its own outcome.
pub async fn record_failure(
    repo: &Repo,
    user_id: &UserId,
    endpoint_hash: &str,
) -> Result<bool, RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(push_sk(endpoint_hash)))
        .update_expression("ADD #fc :one")
        .condition_expression("attribute_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#fc", "failure_count")
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .return_values(ReturnValue::UpdatedNew)
        .send()
        .await;

    let output = match result {
        Ok(o) => o,
        Err(e) => {
            let err = error::from_update_item_error(e);
            return if err.is_lost_race() {
                Ok(false)
            } else {
                Err(err)
            };
        }
    };

    let new_count: u32 = output
        .attributes
        .as_ref()
        .and_then(|a| a.get("failure_count"))
        .and_then(|v| v.as_n().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if new_count >= MAX_PUSH_FAILURES {
        delete_subscription(repo, user_id, endpoint_hash).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// AP21 — notification pref for a (user, group).
pub async fn get_pref(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
) -> Result<Option<NotificationPref>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(npref_sk(group_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

pub async fn put_pref(repo: &Repo, p: &NotificationPref) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(p)?;
    item.insert(attr::PK.into(), AttributeValue::S(user_pk(&p.user_id)));
    item.insert(attr::SK.into(), AttributeValue::S(npref_sk(&p.group_id)));
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("NotificationPref".into()),
    );

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

// ---------- Idempotency markers (`02-data-model-dynamodb.md` §2.15, M11 decision D3) ----------

/// Claim the `NOTIFIED#OPEN` marker for one cycle, at-most-once. `Ok(true)`
/// means this call won the race and the caller should fan out; `Ok(false)`
/// means some other invocation already claimed it (skip).
pub async fn claim_open_notified_marker(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    now: DateTime<Utc>,
) -> Result<bool, RepoError> {
    claim_marker(
        repo,
        &notified_open_pk(group_id, cycle_id),
        NOTIFIED_OPEN_SK,
        now,
    )
    .await
}

/// Claim the `NOTIFIED#PUBLISH` marker for one cycle, at-most-once.
pub async fn claim_publish_notified_marker(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    now: DateTime<Utc>,
) -> Result<bool, RepoError> {
    claim_marker(
        repo,
        &notified_open_pk(group_id, cycle_id),
        NOTIFIED_PUBLISH_SK,
        now,
    )
    .await
}

/// Claim the `NOTIFIED#CLOSE#{offsetHours}` marker for one cycle/offset,
/// at-most-once.
pub async fn claim_close_notified_marker(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    offset_hours: u32,
    now: DateTime<Utc>,
) -> Result<bool, RepoError> {
    claim_marker(
        repo,
        &notified_open_pk(group_id, cycle_id),
        &notified_close_sk(offset_hours),
        now,
    )
    .await
}

async fn claim_marker(
    repo: &Repo,
    pk: &str,
    sk: &str,
    now: DateTime<Utc>,
) -> Result<bool, RepoError> {
    let result = repo
        .client
        .put_item()
        .table_name(&repo.table)
        .item(attr::PK, AttributeValue::S(pk.to_owned()))
        .item(attr::SK, AttributeValue::S(sk.to_owned()))
        .item(attr::ENTITY, AttributeValue::S("NotifiedMarker".into()))
        .item("created_at", AttributeValue::S(now.to_rfc3339()))
        .condition_expression("attribute_not_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK)
        .send()
        .await;
    match result {
        Ok(_) => Ok(true),
        Err(e) => {
            let err = error::from_put_item_error(e);
            if err.is_lost_race() {
                Ok(false)
            } else {
                Err(err)
            }
        }
    }
}

/// Upserts `pk=TICK sk=NOTIFY` with `last_ran_at`, mirroring
/// `persistence::newsletters::upsert_tick_sentinel`.
pub async fn upsert_notify_tick_sentinel(
    repo: &Repo,
    ran_at: DateTime<Utc>,
) -> Result<(), RepoError> {
    repo.client
        .put_item()
        .table_name(&repo.table)
        .item(attr::PK, AttributeValue::S(TICK_PK.into()))
        .item(attr::SK, AttributeValue::S(TICK_NOTIFY_SK.into()))
        .item(attr::ENTITY, AttributeValue::S("TickSentinel".into()))
        .item("last_ran_at", AttributeValue::S(ran_at.to_rfc3339()))
        .send()
        .await?;
    Ok(())
}
