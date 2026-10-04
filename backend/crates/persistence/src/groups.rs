//! Group, GroupMembership access (AP2, AP3, AP4) and leave transaction (§4 #6).

use crate::batch::batch_get_items;
use crate::error::{self, RepoError};
use crate::expr::set_fields;
use crate::keys::{
    attr, group_pk, index, membership_gsi1pk, membership_sk, user_pk, GROUP_META_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, ConditionCheck, Delete, TransactWriteItem, Update};
use domain::{CycleSettings, Group, GroupId, GroupMembership, NotificationSettings, Role, UserId};
use serde_dynamo::{from_item, to_attribute_value, to_item};
use std::collections::HashMap;

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

/// AP4, batched — fetch many groups' meta rows in one round trip, keyed by
/// `GroupId`. Groups that don't exist are absent from the map.
pub async fn get_groups_batch(
    repo: &Repo,
    group_ids: &[GroupId],
) -> Result<HashMap<GroupId, Group>, RepoError> {
    let keys = group_ids
        .iter()
        .map(|group_id| {
            HashMap::from([
                (attr::PK.to_owned(), AttributeValue::S(group_pk(group_id))),
                (
                    attr::SK.to_owned(),
                    AttributeValue::S(GROUP_META_SK.to_owned()),
                ),
            ])
        })
        .collect();

    let mut groups = HashMap::with_capacity(group_ids.len());
    for item in batch_get_items(repo, keys).await? {
        let group: Group = from_item(item)?;
        groups.insert(group.group_id.clone(), group);
    }
    Ok(groups)
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
///
/// `admin_witness`, when `Some`, names a *different* member who must
/// currently hold the admin role. The write then runs as a
/// `TransactWriteItems` with an added `ConditionCheck` on the witness's own
/// membership row (`role = admin`). DynamoDB transactions are serializable,
/// so as long as every removal/demotion of an admin names a still-admin
/// witness, no interleaving of concurrent requests can ever commit a state
/// with zero admins (`plans/03-api-contract.md` §3.6's `LAST_ADMIN` guard;
/// see also [`leave_group_tx`]). Pass `None` when `user_id` does not
/// currently hold the admin role — no witness is needed to protect an
/// invariant that isn't at stake.
pub async fn update_membership_role(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
    role: Role,
    admin_witness: Option<&UserId>,
) -> Result<(), RepoError> {
    let Some(witness) = admin_witness else {
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
            .await
            .map_err(error::from_update_item_error)?;
        return Ok(());
    };

    let update = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .update_expression("SET #role = :role")
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#role", "role")
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":role", to_attribute_value(role)?)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let witness_check = admin_witness_condition(repo, witness, group_id)?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().update(update).build())
        .transact_items(
            TransactWriteItem::builder()
                .condition_check(witness_check)
                .build(),
        )
        .send()
        .await
        .map_err(error::from_transact_write_error)?;
    Ok(())
}

/// Build the `ConditionCheck` transact-item asserting that `witness` currently
/// holds the admin role in `group_id`. Shared by [`update_membership_role`]
/// and [`leave_group_tx`].
fn admin_witness_condition(
    repo: &Repo,
    witness: &UserId,
    group_id: &GroupId,
) -> Result<ConditionCheck, RepoError> {
    ConditionCheck::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(witness)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .condition_expression("attribute_exists(#pk) AND #role = :admin")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#role", "role")
        .expression_attribute_values(":admin", to_attribute_value(Role::Admin)?)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// Transaction §4 #6 — leave group: delete membership + decrement memberCount.
///
/// `admin_witness`, when `Some`, names a *different* member who must
/// currently hold the admin role, and adds a `ConditionCheck` transact-item
/// enforcing that — see [`update_membership_role`]'s rustdoc for why this
/// closes the last-admin TOCTOU race. Pass `None` when the member being
/// removed does not currently hold the admin role.
pub async fn leave_group_tx(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
    admin_witness: Option<&UserId>,
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

    let mut request = repo
        .client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(update).build());

    if let Some(witness) = admin_witness {
        let witness_check = admin_witness_condition(repo, witness, group_id)?;
        request = request.transact_items(
            TransactWriteItem::builder()
                .condition_check(witness_check)
                .build(),
        );
    }

    request
        .send()
        .await
        .map_err(error::from_transact_write_error)?;
    Ok(())
}
