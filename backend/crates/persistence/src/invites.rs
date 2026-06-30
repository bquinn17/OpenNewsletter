//! Invite CRUD and the join-via-invite transaction (§4 #5).

use crate::error::RepoError;
use crate::keys::{
    attr, group_pk, index, invite_gsi1pk, invite_gsi1sk, invite_pk, membership_gsi1pk,
    membership_gsi1sk, membership_sk, user_pk, GROUP_META_SK, INVITE_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem, Update};
use domain::{GroupId, GroupMembership, Invite, InviteCode, InviteStatus};
use serde_dynamo::{from_item, to_item};

/// AP5 — get invite by code.
pub async fn get_invite(repo: &Repo, code: &InviteCode) -> Result<Option<Invite>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(invite_pk(code)))
        .key(attr::SK, AttributeValue::S(INVITE_SK.into()))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// AP6 — list invites for a group.
pub async fn list_invites_for_group(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Vec<Invite>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_names("#sk", attr::GSI1SK)
        .expression_attribute_values(":pk", AttributeValue::S(invite_gsi1pk(group_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("INVITE#".into()))
        .send()
        .await?;

    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Invite>(i).map_err(RepoError::from))
        .collect()
}

pub async fn put_invite(repo: &Repo, invite: &Invite, ttl_epoch_seconds: i64) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(invite)?;
    item.insert(attr::PK.into(), AttributeValue::S(invite_pk(&invite.code)));
    item.insert(attr::SK.into(), AttributeValue::S(INVITE_SK.into()));
    item.insert(attr::GSI1PK.into(), AttributeValue::S(invite_gsi1pk(&invite.group_id)));
    item.insert(attr::GSI1SK.into(), AttributeValue::S(invite_gsi1sk(&invite.code)));
    item.insert(attr::ENTITY.into(), AttributeValue::S("Invite".into()));
    item.insert(attr::TTL.into(), AttributeValue::N(ttl_epoch_seconds.to_string()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

pub async fn revoke(repo: &Repo, code: &InviteCode) -> Result<(), RepoError> {
    let status = serde_json::to_string(&InviteStatus::Revoked).unwrap_or_else(|_| "\"revoked\"".to_string());
    let trimmed = status.trim_matches('"').to_string();
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(invite_pk(code)))
        .key(attr::SK, AttributeValue::S(INVITE_SK.into()))
        .update_expression("SET #s = :s")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":s", AttributeValue::S(trimmed))
        .send()
        .await?;
    Ok(())
}

/// Transaction §4 #5 — join group via invite.
///
/// Atomically:
/// - flip Invite `pending → consumed` (precondition: status was pending and not expired)
/// - Put GroupMembership
/// - Update Group.memberCount += 1 with member-cap check
pub async fn join_via_invite_tx(
    repo: &Repo,
    invite: &Invite,
    membership: &GroupMembership,
    consumed_at_iso: &str,
    member_cap: u32,
) -> Result<(), RepoError> {
    let consume = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(invite_pk(&invite.code)))
        .key(attr::SK, AttributeValue::S(INVITE_SK.into()))
        .update_expression("SET #s = :consumed, consumed_by = :u, consumed_at = :t")
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":consumed", AttributeValue::S("consumed".into()))
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .expression_attribute_values(":u", AttributeValue::S(membership.user_id.to_string()))
        .expression_attribute_values(":t", AttributeValue::S(consumed_at_iso.into()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let mut mem_item: std::collections::HashMap<String, AttributeValue> = to_item(membership)?;
    mem_item.insert(attr::PK.into(), AttributeValue::S(user_pk(&membership.user_id)));
    mem_item.insert(attr::SK.into(), AttributeValue::S(membership_sk(&membership.group_id)));
    mem_item.insert(attr::GSI1PK.into(), AttributeValue::S(membership_gsi1pk(&membership.group_id)));
    mem_item.insert(attr::GSI1SK.into(), AttributeValue::S(membership_gsi1sk(&membership.user_id)));
    mem_item.insert(attr::ENTITY.into(), AttributeValue::S("GroupMembership".into()));

    let put_member = Put::builder()
        .table_name(&repo.table)
        .set_item(Some(mem_item))
        .condition_expression("attribute_not_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let bump_count = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(&membership.group_id)))
        .key(attr::SK, AttributeValue::S(GROUP_META_SK.into()))
        .update_expression("SET member_count = member_count + :one")
        .condition_expression("member_count < :cap")
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .expression_attribute_values(":cap", AttributeValue::N(member_cap.to_string()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().update(consume).build())
        .transact_items(TransactWriteItem::builder().put(put_member).build())
        .transact_items(TransactWriteItem::builder().update(bump_count).build())
        .send()
        .await?;
    Ok(())
}

