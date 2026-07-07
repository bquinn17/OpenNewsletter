//! PushSubscription and NotificationPref (AP20, AP21).

use crate::error::RepoError;
use crate::keys::{attr, npref_sk, push_sk, user_pk};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{GroupId, NotificationPref, PushSubscription, UserId};
use serde_dynamo::{from_item, to_item};

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

pub async fn put_subscription(repo: &Repo, s: &PushSubscription) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(s)?;
    item.insert(attr::PK.into(), AttributeValue::S(user_pk(&s.user_id)));
    item.insert(
        attr::SK.into(),
        AttributeValue::S(push_sk(&s.endpoint_hash)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("PushSubscription".into()),
    );

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
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
