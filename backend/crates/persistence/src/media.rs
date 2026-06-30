//! ImageMedia and AvatarMedia (AP17 + avatar lookups).

use crate::error::RepoError;
use crate::keys::{
    attr, avatar_sk, image_gsi1pk, image_gsi1sk, image_pk, image_sk, index, user_pk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{AvatarId, AvatarMedia, CycleId, GroupId, ImageId, ImageMedia, UserId};
use serde_dynamo::{from_item, to_item};

pub async fn get_image(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<Option<ImageMedia>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

pub async fn put_image(repo: &Repo, img: &ImageMedia) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(img)?;
    item.insert(attr::PK.into(), AttributeValue::S(image_pk(&img.group_id, &img.cycle_id)));
    item.insert(attr::SK.into(), AttributeValue::S(image_sk(&img.image_id)));
    item.insert(attr::GSI1PK.into(), AttributeValue::S(image_gsi1pk(&img.user_id)));
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(image_gsi1sk(&img.uploaded_at.to_rfc3339(), &img.image_id)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("ImageMedia".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// AP17 — list a user's image uploads (admin/audit; via GSI1).
pub async fn list_user_images(repo: &Repo, user_id: &UserId) -> Result<Vec<ImageMedia>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_values(":pk", AttributeValue::S(image_gsi1pk(user_id)))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, ImageMedia>(i).map_err(RepoError::from))
        .collect()
}

pub async fn get_avatar(
    repo: &Repo,
    user_id: &UserId,
    avatar_id: &AvatarId,
) -> Result<Option<AvatarMedia>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(avatar_sk(avatar_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

pub async fn put_avatar(repo: &Repo, a: &AvatarMedia) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(a)?;
    item.insert(attr::PK.into(), AttributeValue::S(user_pk(&a.user_id)));
    item.insert(attr::SK.into(), AttributeValue::S(avatar_sk(&a.avatar_id)));
    item.insert(attr::ENTITY.into(), AttributeValue::S("AvatarMedia".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}
