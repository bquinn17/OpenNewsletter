//! Newsletter (cycle) access (AP7, AP8, AP9) and status-transition writer.

use crate::error::RepoError;
use crate::keys::{
    attr, group_pk, index, newsletter_gsi2pk, newsletter_gsi2sk, newsletter_sk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{CycleId, GroupId, Newsletter, NewsletterStatus};
use serde_dynamo::{from_item, to_item};

const SENTINEL_FAR_FUTURE: &str = "9999-12-31T00:00:00Z";

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

/// AP9 — cycles by status whose next transition is due (`<= now_iso`).
pub async fn list_cycles_due(
    repo: &Repo,
    status: NewsletterStatus,
    now_iso: &str,
) -> Result<Vec<Newsletter>, RepoError> {
    let upper = format!("{now_iso}#~~~");
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
    item.insert(attr::SK.into(), AttributeValue::S(newsletter_sk(&nl.cycle_id)));
    item.insert(attr::GSI2PK.into(), AttributeValue::S(newsletter_gsi2pk(nl.status)));
    let when = nl
        .next_transition_at
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| SENTINEL_FAR_FUTURE.to_owned());
    item.insert(
        attr::GSI2SK.into(),
        AttributeValue::S(newsletter_gsi2sk(&when, &nl.group_id, &nl.cycle_id)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Newsletter".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}
