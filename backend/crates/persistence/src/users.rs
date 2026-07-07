//! User and Cognito-sub-lookup access (AP1, plus auth resolution).

use crate::error::RepoError;
use crate::keys::{attr, cognito_sub_pk, user_pk, COGNITO_SUB_SK, USER_PROFILE_SK};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{CognitoSub, CognitoSubLookup, User, UserId};
use serde_dynamo::{from_item, to_item};

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
