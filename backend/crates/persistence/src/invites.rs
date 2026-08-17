//! Invite CRUD and the join-via-invite transaction (§4 #5).

use crate::error::RepoError;
use crate::keys::{
    attr, cognito_sub_pk, group_pk, index, invite_gsi1pk, invite_gsi1sk, invite_pk,
    membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk, COGNITO_SUB_SK, GROUP_META_SK,
    INVITE_SK, USER_PROFILE_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem, Update};
use domain::{CognitoSubLookup, GroupId, GroupMembership, Invite, InviteCode, InviteStatus, User};
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

pub async fn put_invite(
    repo: &Repo,
    invite: &Invite,
    ttl_epoch_seconds: i64,
) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(invite)?;
    item.insert(attr::PK.into(), AttributeValue::S(invite_pk(&invite.code)));
    item.insert(attr::SK.into(), AttributeValue::S(INVITE_SK.into()));
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(invite_gsi1pk(&invite.group_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(invite_gsi1sk(&invite.code)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Invite".into()));
    item.insert(
        attr::TTL.into(),
        AttributeValue::N(ttl_epoch_seconds.to_string()),
    );

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// Flip a pending invite to `revoked`. Fails with [`RepoError::ConditionalCheckFailed`]
/// if the invite was consumed or revoked in the meantime.
pub async fn revoke(repo: &Repo, code: &InviteCode) -> Result<(), RepoError> {
    let status =
        serde_json::to_string(&InviteStatus::Revoked).unwrap_or_else(|_| "\"revoked\"".to_string());
    let trimmed = status.trim_matches('"').to_string();
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(invite_pk(code)))
        .key(attr::SK, AttributeValue::S(INVITE_SK.into()))
        .update_expression("SET #s = :s")
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":s", AttributeValue::S(trimmed))
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .send()
        .await
        .map_err(classify_conditional_failure)?;
    Ok(())
}

fn classify_conditional_failure<E, R>(err: aws_sdk_dynamodb::error::SdkError<E, R>) -> RepoError
where
    E: std::fmt::Debug,
    R: std::fmt::Debug,
{
    let rendered = format!("{err:?}");
    if rendered.contains("ConditionalCheckFailed") {
        RepoError::ConditionalCheckFailed
    } else {
        RepoError::Dynamo(rendered)
    }
}

/// Transaction §4 #5 — join group via invite.
///
/// Atomically:
/// - create the `CognitoSubLookup` + `User` rows when `new_user` is set (first-ever
///   redemption for this Cognito account — `plans/05-auth-flow.md` §4.2)
/// - flip Invite `pending → consumed`
/// - Put GroupMembership
/// - Update Group.memberCount += 1 with member-cap check
///
/// Expiry is enforced by the caller rather than by a condition expression: `expires_at`
/// is stored as RFC-3339 with a variable-width fractional part, so a lexicographic
/// comparison against "now" is not reliably ordered at sub-second resolution.
pub async fn join_via_invite_tx(
    repo: &Repo,
    invite: &Invite,
    membership: &GroupMembership,
    new_user: Option<&User>,
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
    mem_item.insert(
        attr::PK.into(),
        AttributeValue::S(user_pk(&membership.user_id)),
    );
    mem_item.insert(
        attr::SK.into(),
        AttributeValue::S(membership_sk(&membership.group_id)),
    );
    mem_item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(membership_gsi1pk(&membership.group_id)),
    );
    mem_item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(membership_gsi1sk(&membership.user_id)),
    );
    mem_item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("GroupMembership".into()),
    );

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

    let mut tx = repo.client.transact_write_items();

    if let Some(user) = new_user {
        let mut lookup_item: std::collections::HashMap<String, AttributeValue> =
            to_item(CognitoSubLookup {
                cognito_sub: user.cognito_sub.clone(),
                user_id: user.user_id.clone(),
            })?;
        lookup_item.insert(
            attr::PK.into(),
            AttributeValue::S(cognito_sub_pk(&user.cognito_sub)),
        );
        lookup_item.insert(attr::SK.into(), AttributeValue::S(COGNITO_SUB_SK.into()));
        lookup_item.insert(
            attr::ENTITY.into(),
            AttributeValue::S("CognitoSubLookup".into()),
        );

        let put_lookup = Put::builder()
            .table_name(&repo.table)
            .set_item(Some(lookup_item))
            .condition_expression("attribute_not_exists(#pk)")
            .expression_attribute_names("#pk", attr::PK)
            .build()
            .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

        let mut user_item: std::collections::HashMap<String, AttributeValue> = to_item(user)?;
        user_item.insert(attr::PK.into(), AttributeValue::S(user_pk(&user.user_id)));
        user_item.insert(attr::SK.into(), AttributeValue::S(USER_PROFILE_SK.into()));
        user_item.insert(attr::ENTITY.into(), AttributeValue::S("User".into()));

        let put_user = Put::builder()
            .table_name(&repo.table)
            .set_item(Some(user_item))
            .build()
            .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

        tx = tx
            .transact_items(TransactWriteItem::builder().put(put_lookup).build())
            .transact_items(TransactWriteItem::builder().put(put_user).build());
    }

    tx.transact_items(TransactWriteItem::builder().update(consume).build())
        .transact_items(TransactWriteItem::builder().put(put_member).build())
        .transact_items(TransactWriteItem::builder().update(bump_count).build())
        .send()
        .await
        .map_err(classify_conditional_failure)?;
    Ok(())
}
