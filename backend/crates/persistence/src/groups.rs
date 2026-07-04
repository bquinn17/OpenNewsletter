//! Group, GroupMembership access (AP2, AP3, AP4) and leave transaction (§4 #6).

use crate::error::RepoError;
use crate::keys::{
    attr, group_pk, index, membership_gsi1pk, membership_sk, user_pk, GROUP_META_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, Delete, TransactWriteItem, Update};
use domain::{Group, GroupId, GroupMembership, UserId};
use serde_dynamo::{from_item, to_item};

/// AP4 — Get group meta.
pub async fn get_group(repo: &Repo, group_id: &GroupId) -> Result<Option<Group>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(attr::SK, AttributeValue::S(GROUP_META_SK.into()))
        .send()
        .await?;

    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

pub async fn put_group(repo: &Repo, group: &Group) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(group)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(group_pk(&group.group_id)),
    );
    item.insert(attr::SK.into(), AttributeValue::S(GROUP_META_SK.into()));
    item.insert(attr::ENTITY.into(), AttributeValue::S("Group".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// AP2 — list groups a user is in.
pub async fn list_memberships_for_user(
    repo: &Repo,
    user_id: &UserId,
) -> Result<Vec<GroupMembership>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(user_pk(user_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("GROUP#".into()))
        .send()
        .await?;

    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, GroupMembership>(i).map_err(RepoError::from))
        .collect()
}

/// AP3 — list members of a group via GSI1.
pub async fn list_members_for_group(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Vec<GroupMembership>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_names("#sk", attr::GSI1SK)
        .expression_attribute_values(":pk", AttributeValue::S(membership_gsi1pk(group_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("MEMBER#".into()))
        .send()
        .await?;

    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, GroupMembership>(i).map_err(RepoError::from))
        .collect()
}

pub async fn get_membership(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
) -> Result<Option<GroupMembership>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// Transaction §4 #6 — leave group: delete membership + decrement memberCount.
pub async fn leave_group_tx(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
) -> Result<(), RepoError> {
    let delete = Delete::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let update = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(attr::SK, AttributeValue::S(GROUP_META_SK.into()))
        .update_expression("SET member_count = member_count - :one")
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(update).build())
        .send()
        .await?;
    Ok(())
}
