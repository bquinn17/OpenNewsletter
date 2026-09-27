//! Newsletter (cycle) access (AP7, AP8, AP9), status-transition writer, and the
//! read/write helpers from `plans/06-newsletter-lifecycle.md` §5, §10.

use crate::error::{self, RepoError};
use crate::keys::{
    attr, group_pk, index, key_timestamp, newsletter_gsi2pk, newsletter_gsi2sk,
    newsletter_gsi2sk_sentinel, newsletter_sk, TICK_CYCLE_SK, TICK_PK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use domain::{CycleId, GroupId, Newsletter, NewsletterStatus};
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;

/// AP7 — list newsletters in a group, newest first.
pub async fn list_newsletters_for_group(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Vec<Newsletter>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(group_pk(group_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("NL#".into()))
        .scan_index_forward(false)
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Newsletter>(i).map_err(RepoError::from))
        .collect()
}

/// AP8 — get newsletter.
pub async fn get_newsletter(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<Option<Newsletter>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(attr::SK, AttributeValue::S(newsletter_sk(cycle_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// AP9 — cycles by status whose next transition is due (`<= now`). `now` is
/// truncated to whole seconds (via [`key_timestamp`]) before comparison,
/// which is correct: a cycle due at exactly this second must still match.
pub async fn list_cycles_due(
    repo: &Repo,
    status: NewsletterStatus,
    now: DateTime<Utc>,
) -> Result<Vec<Newsletter>, RepoError> {
    let upper = format!("{}#~~~", key_timestamp(now));
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI2)
        .key_condition_expression("#pk = :pk AND #sk <= :upper")
        .expression_attribute_names("#pk", attr::GSI2PK)
        .expression_attribute_names("#sk", attr::GSI2SK)
        .expression_attribute_values(":pk", AttributeValue::S(newsletter_gsi2pk(status)))
        .expression_attribute_values(":upper", AttributeValue::S(upper))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Newsletter>(i).map_err(RepoError::from))
        .collect()
}

/// Write or rewrite a Newsletter with consistent GSI2 keys. The single chokepoint
/// for state transitions referenced in §9 of `02-data-model-dynamodb.md`.
pub async fn write_status_transition(repo: &Repo, nl: &Newsletter) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(nl)?;
    item.insert(attr::PK.into(), AttributeValue::S(group_pk(&nl.group_id)));
    item.insert(
        attr::SK.into(),
        AttributeValue::S(newsletter_sk(&nl.cycle_id)),
    );
    item.insert(
        attr::GSI2PK.into(),
        AttributeValue::S(newsletter_gsi2pk(nl.status)),
    );
    let gsi2sk = match nl.next_transition_at {
        Some(d) => newsletter_gsi2sk(d, &nl.group_id, &nl.cycle_id),
        None => newsletter_gsi2sk_sentinel(&nl.group_id, &nl.cycle_id),
    };
    item.insert(attr::GSI2SK.into(), AttributeValue::S(gsi2sk));
    item.insert(attr::ENTITY.into(), AttributeValue::S("Newsletter".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// §10 — the current `voting` cycle for a group, if any. Candidate-question
/// endpoints use this to resolve "the next cycle."
pub async fn find_voting_cycle(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Option<Newsletter>, RepoError> {
    let cycles = list_newsletters_for_group(repo, group_id).await?;
    Ok(cycles
        .into_iter()
        .find(|nl| nl.status == NewsletterStatus::Voting))
}

/// §10 — the current `open` cycle for a group, if any.
pub async fn find_open_cycle(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Option<Newsletter>, RepoError> {
    let cycles = list_newsletters_for_group(repo, group_id).await?;
    Ok(cycles
        .into_iter()
        .find(|nl| nl.status == NewsletterStatus::Open))
}

/// §10 — a page of a group's newsletters, newest first, for
/// `GET /groups/{g}/newsletters`. `cursor` is the opaque token this function
/// returned as `next_cursor` from a previous call.
///
/// The cursor encodes only the last-seen `cycleId`: for this base-table query
/// (no GSI), `{pk, sk}` is a complete and sufficient `ExclusiveStartKey`, so
/// there is no need to round-trip DynamoDB's raw `LastEvaluatedKey`.
pub async fn list_recent(
    repo: &Repo,
    group_id: &GroupId,
    limit: i32,
    cursor: Option<&str>,
) -> Result<(Vec<Newsletter>, Option<String>), RepoError> {
    let mut request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(group_pk(group_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("NL#".into()))
        .scan_index_forward(false)
        .limit(limit);

    if let Some(raw) = cursor {
        let cycle_id = decode_cursor(raw)?;
        let mut start_key = HashMap::new();
        start_key.insert(attr::PK.to_owned(), AttributeValue::S(group_pk(group_id)));
        start_key.insert(
            attr::SK.to_owned(),
            AttributeValue::S(newsletter_sk(&cycle_id)),
        );
        request = request.set_exclusive_start_key(Some(start_key));
    }

    let resp = request.send().await?;
    let items = resp.items.unwrap_or_default();
    let newsletters: Vec<Newsletter> = items
        .into_iter()
        .map(|i| from_item::<_, Newsletter>(i).map_err(RepoError::from))
        .collect::<Result<_, _>>()?;

    let next_cursor = if resp.last_evaluated_key.is_some() {
        newsletters.last().map(|nl| encode_cursor(&nl.cycle_id))
    } else {
        None
    };
    Ok((newsletters, next_cursor))
}

fn encode_cursor(cycle_id: &CycleId) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::json!({ "cycleId": cycle_id.as_str() }).to_string())
}

fn decode_cursor(raw: &str) -> Result<CycleId, RepoError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|e| RepoError::Serde(format!("invalid cursor: {e}")))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| RepoError::Serde(format!("invalid cursor: {e}")))?;
    let cycle_id = value
        .get("cycleId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RepoError::Serde("cursor missing cycleId".into()))?;
    Ok(CycleId::new(cycle_id))
}

/// §5.4 — put-if-not-exists the next `voting` cycle after `after`, per the
/// default-schedule formula in `shared::cycle_time`. Returns the newsletter
/// that now exists — either the one this call just created, or (when
/// `RepoError::ConditionalCheckFailed`, i.e. a concurrent tick already won the
/// race) the caller should treat that as a no-op, not a failure (§8).
pub async fn create_next_voting_cycle(
    repo: &Repo,
    group_id: &GroupId,
    tz: Tz,
    response_window_days: u32,
    after: DateTime<Utc>,
) -> Result<Newsletter, RepoError> {
    let schedule = shared::cycle_time::next_cycle_schedule(after, tz, response_window_days);
    let nl = Newsletter {
        group_id: group_id.clone(),
        cycle_id: schedule.cycle_id,
        status: NewsletterStatus::Voting,
        vote_window_open_at: Utc::now(),
        vote_window_close_at: schedule.response_open_at,
        response_open_at: schedule.response_open_at,
        response_close_at: schedule.response_close_at,
        published_at: None,
        next_transition_at: Some(schedule.response_open_at),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
    };

    let mut item: HashMap<String, AttributeValue> = to_item(&nl)?;
    item.insert(attr::PK.into(), AttributeValue::S(group_pk(group_id)));
    item.insert(
        attr::SK.into(),
        AttributeValue::S(newsletter_sk(&nl.cycle_id)),
    );
    item.insert(
        attr::GSI2PK.into(),
        AttributeValue::S(newsletter_gsi2pk(NewsletterStatus::Voting)),
    );
    item.insert(
        attr::GSI2SK.into(),
        AttributeValue::S(newsletter_gsi2sk(
            nl.response_open_at,
            group_id,
            &nl.cycle_id,
        )),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Newsletter".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .condition_expression("attribute_not_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK)
        .send()
        .await
        .map_err(error::from_put_item_error)?;
    Ok(nl)
}

/// §5.3 — `open -> published`: conditional on the cycle still being `open`,
/// sets `publishedAt`, moves the GSI2 sort key to the archive sentinel so the
/// tick's due-query stops matching it, and clears `nextTransitionAt`.
pub async fn publish_cycle(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    published_at: DateTime<Utc>,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(attr::SK, AttributeValue::S(newsletter_sk(cycle_id)))
        .update_expression(
            "SET #s = :published, published_at = :published_at, #g2pk = :g2pk, #g2sk = :g2sk \
             REMOVE next_transition_at",
        )
        .condition_expression("#s = :open")
        .expression_attribute_names("#s", "status")
        .expression_attribute_names("#g2pk", attr::GSI2PK)
        .expression_attribute_names("#g2sk", attr::GSI2SK)
        .expression_attribute_values(":open", AttributeValue::S("open".into()))
        .expression_attribute_values(":published", AttributeValue::S("published".into()))
        .expression_attribute_values(
            ":published_at",
            AttributeValue::S(published_at.to_rfc3339()),
        )
        .expression_attribute_values(
            ":g2pk",
            AttributeValue::S(newsletter_gsi2pk(NewsletterStatus::Published)),
        )
        .expression_attribute_values(
            ":g2sk",
            AttributeValue::S(newsletter_gsi2sk_sentinel(group_id, cycle_id)),
        )
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}

/// §5.1 step 4 — upsert the `pk=TICK sk=CYCLE` sentinel row so operators can
/// see when `lambda-cycle-tick` last ran.
pub async fn upsert_tick_sentinel(repo: &Repo, ran_at: DateTime<Utc>) -> Result<(), RepoError> {
    repo.client
        .put_item()
        .table_name(&repo.table)
        .item(attr::PK, AttributeValue::S(TICK_PK.into()))
        .item(attr::SK, AttributeValue::S(TICK_CYCLE_SK.into()))
        .item(attr::ENTITY, AttributeValue::S("TickSentinel".into()))
        .item("last_ran_at", AttributeValue::S(ran_at.to_rfc3339()))
        .send()
        .await?;
    Ok(())
}

/// Dev-only helper backing `POST /admin/dev/tick/cycle`'s `advanceCycleClosesBy`
/// (`plans/03-api-contract.md` §11a.1). Rewinds whichever of the group's
/// non-terminal cycles is due to transition next — the `open` cycle if one
/// exists (its `responseCloseAt`), else the `voting` cycle (its
/// `responseOpenAt`/`voteWindowCloseAt`) — so the next tick treats the
/// transition as due immediately. Returns `None` if the group has no
/// non-terminal cycle at all.
pub async fn rewind_active_cycle_deadline(
    repo: &Repo,
    group_id: &GroupId,
    by: Duration,
) -> Result<Option<Newsletter>, RepoError> {
    let active = match find_open_cycle(repo, group_id).await? {
        Some(nl) => Some(nl),
        None => find_voting_cycle(repo, group_id).await?,
    };
    let Some(mut nl) = active else {
        return Ok(None);
    };

    match nl.status {
        NewsletterStatus::Open => {
            let new_close = nl.response_close_at - by;
            nl.response_close_at = new_close;
            nl.next_transition_at = Some(new_close);
        }
        NewsletterStatus::Voting => {
            let new_open = nl.response_open_at - by;
            nl.response_open_at = new_open;
            nl.vote_window_close_at = new_open;
            nl.next_transition_at = Some(new_open);
        }
        NewsletterStatus::Published | NewsletterStatus::Archived => {
            unreachable!("find_open_cycle/find_voting_cycle only return Open/Voting newsletters")
        }
    }

    write_status_transition(repo, &nl).await?;
    Ok(Some(nl))
}
