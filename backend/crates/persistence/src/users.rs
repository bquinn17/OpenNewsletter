//! User and Cognito-sub-lookup access (AP1, plus auth resolution).

use crate::error::RepoError;
use crate::expr::set_fields;
use crate::keys::{attr, cognito_sub_pk, user_pk, COGNITO_SUB_SK, USER_PROFILE_SK};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, KeysAndAttributes};
use domain::{AvatarId, CognitoSub, CognitoSubLookup, User, UserId};
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;

/// AP1 — Get user profile.
pub async fn get_user(repo: &Repo, user_id: &UserId) -> Result<Option<User>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(USER_PROFILE_SK.to_owned()))
        .send()
        .await?;

    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// Resolve a Cognito sub claim to the internal `UserId` (used on every authed request).
pub async fn resolve_cognito_sub(
    repo: &Repo,
    sub: &CognitoSub,
) -> Result<Option<UserId>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(cognito_sub_pk(sub)))
        .key(attr::SK, AttributeValue::S(COGNITO_SUB_SK.to_owned()))
        .send()
        .await?;

    match resp.item {
        Some(item) => {
            let lookup: CognitoSubLookup = from_item(item)?;
            Ok(Some(lookup.user_id))
        }
        None => Ok(None),
    }
}

/// Upsert a user profile (used during invite redemption — transactional variant lives in `invites`).
pub async fn put_user(repo: &Repo, user: &User) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(user)?;
    item.insert(attr::PK.into(), AttributeValue::S(user_pk(&user.user_id)));
    item.insert(attr::SK.into(), AttributeValue::S(USER_PROFILE_SK.into()));
    item.insert(attr::ENTITY.into(), AttributeValue::S("User".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// Fetch many user profiles in one round trip, keyed by `UserId`.
///
/// Callers pass a group's member list, which the member soft cap keeps well inside
/// `BatchGetItem`'s 100-key limit; anything larger would need paging.
pub async fn get_users_batch(
    repo: &Repo,
    user_ids: &[UserId],
) -> Result<HashMap<UserId, User>, RepoError> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut keys = Vec::with_capacity(user_ids.len());
    for user_id in user_ids {
        keys.push(HashMap::from([
            (attr::PK.to_owned(), AttributeValue::S(user_pk(user_id))),
            (
                attr::SK.to_owned(),
                AttributeValue::S(USER_PROFILE_SK.to_owned()),
            ),
        ]));
    }

    let request = KeysAndAttributes::builder()
        .set_keys(Some(keys))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let resp = repo
        .client
        .batch_get_item()
        .request_items(&repo.table, request)
        .send()
        .await?;

    let items = resp
        .responses
        .and_then(|mut tables| tables.remove(&repo.table))
        .unwrap_or_default();

    let mut users = HashMap::with_capacity(items.len());
    for item in items {
        let user: User = from_item(item)?;
        users.insert(user.user_id.clone(), user);
    }
    Ok(users)
}

/// Apply a `PATCH /me` patch. Absent fields are left untouched; `avatar_media_id`
/// distinguishes "not supplied" (outer `None`) from "clear the photo" (inner `None`).
pub async fn update_profile(
    repo: &Repo,
    user_id: &UserId,
    display_name: Option<&str>,
    avatar_color: Option<&str>,
    avatar_media_id: Option<Option<&AvatarId>>,
) -> Result<(), RepoError> {
    let mut fields: Vec<(&str, AttributeValue)> = Vec::new();
    if let Some(name) = display_name {
        fields.push(("display_name", AttributeValue::S(name.to_owned())));
    }
    if let Some(color) = avatar_color {
        fields.push(("avatar_color", AttributeValue::S(color.to_owned())));
    }
    if let Some(avatar) = avatar_media_id {
        let value = match avatar {
            Some(id) => AttributeValue::S(id.to_string()),
            None => AttributeValue::Null(true),
        };
        fields.push(("avatar_media_id", value));
    }

    if fields.is_empty() {
        return Ok(());
    }

    let update = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(USER_PROFILE_SK.into()))
        .condition_expression("attribute_exists(#pk)")
        .expression_attribute_names("#pk", attr::PK);

    let (update, assignments) = set_fields(update, fields);
    update
        .update_expression(format!("SET {}", assignments.join(", ")))
        .send()
        .await?;
    Ok(())
}

pub async fn update_last_login(
    repo: &Repo,
    user_id: &UserId,
    when_iso: &str,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(USER_PROFILE_SK.into()))
        .update_expression("SET #last = :last")
        .expression_attribute_names("#last", "last_login_at")
        .expression_attribute_values(":last", AttributeValue::S(when_iso.into()))
        .send()
        .await?;
    Ok(())
}
