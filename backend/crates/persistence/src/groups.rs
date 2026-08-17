//! Group, GroupMembership access (AP2, AP3, AP4) and leave transaction (§4 #6).

use crate::error::RepoError;
use crate::expr::set_fields;
use crate::keys::{
    attr, group_pk, index, membership_gsi1pk, membership_sk, user_pk, GROUP_META_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, Delete, TransactWriteItem, Update};
use domain::{CycleSettings, Group, GroupId, GroupMembership, NotificationSettings, Role, UserId};
use serde_dynamo::{from_item, to_attribute_value, to_item};

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

/// A validated `PATCH /groups/{groupId}` patch. Every field is optional; absent
/// fields are left untouched.
#[derive(Debug, Default)]
pub struct GroupPatch {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub gradient: Option<String>,
    pub cycle_settings: Option<CycleSettings>,
    pub notification_settings: Option<NotificationSettings>,
    pub member_soft_cap: Option<u32>,
}

impl GroupPatch {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.timezone.is_none()
            && self.gradient.is_none()
            && self.cycle_settings.is_none()
            && self.notification_settings.is_none()
            && self.member_soft_cap.is_none()
    }
}

/// Apply a group patch as a targeted `UpdateItem`, so a concurrent join or leave
/// can't have its `member_count` change clobbered by a read-modify-write.
pub async fn update_group(
    repo: &Repo,
    group_id: &GroupId,
    patch: &GroupPatch,
) -> Result<(), RepoError> {
    if patch.is_empty() {
        return Ok(());
    }

    let mut fields: Vec<(&str, AttributeValue)> = Vec::new();
    if let Some(name) = &patch.name {
        fields.push(("name", AttributeValue::S(name.clone())));
    }
    if let Some(timezone) = &patch.timezone {
        fields.push(("timezone", AttributeValue::S(timezone.clone())));
    }
    if let Some(gradient) = &patch.gradient {
        fields.push(("gradient", AttributeValue::S(gradient.clone())));
    }
    if let Some(settings) = &patch.cycle_settings {
        fields.push(("cycle_settings", to_attribute_value(settings)?));
    }
    if let Some(settings) = &patch.notification_settings {
        fields.push(("notification_settings", to_attribute_value(settings)?));
    }
    if let Some(cap) = patch.member_soft_cap {
        fields.push(("member_soft_cap", AttributeValue::N(cap.to_string())));
    }

    let update = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(attr::SK, AttributeValue::S(GROUP_META_SK.into()))
        .condition_expression("attribute_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK);

    let (update, assignments) = set_fields(update, fields);
    update
        .update_expression(format!("SET {}", assignments.join(", ")))
        .send()
        .await?;
    Ok(())
}

/// Change a member's role. Callers enforce the last-admin guard first.
pub async fn update_membership_role(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
    role: Role,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .update_expression("SET #role = :role")
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#role", "role")
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":role", to_attribute_value(role)?)
        .send()
        .await?;
    Ok(())
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
