//! ImageMedia and AvatarMedia (AP17 + avatar lookups).

use crate::error::{self, RepoError};
use crate::keys::{
    attr, avatar_sk, image_gsi1pk, image_gsi1sk, image_pk, image_sk, index, user_pk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{AvatarId, AvatarMedia, CycleId, GroupId, ImageId, ImageMedia, QuestionId, UserId};
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;

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
    item.insert(
        attr::PK.into(),
        AttributeValue::S(image_pk(&img.group_id, &img.cycle_id)),
    );
    item.insert(attr::SK.into(), AttributeValue::S(image_sk(&img.image_id)));
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(image_gsi1pk(&img.user_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(image_gsi1sk(img.uploaded_at, &img.image_id)),
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

/// The caller's `purpose=response` image count for `(cycleId, questionId)`,
/// excluding `failed` rows (`03-api-contract.md` §9.1, `08-media-uploads.md`
/// §3.1 — the 10-images-per-answer cap). A `Query` of the cycle's `IMG#`
/// range filtered on `user_id`/`question_id`/`purpose`/`status`, not GSI1 —
/// the GSI1 inversion is keyed by uploader across every cycle they've ever
/// uploaded to, which is the wrong scope for a per-answer cap. Concurrent
/// uploads can race past the cap between this count and the next
/// `POST /uploads`; acceptable, since `lambda-responses`' save independently
/// caps `imageMediaIds` at the same limit.
pub async fn count_active_response_images(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    user_id: &UserId,
    question_id: &QuestionId,
) -> Result<usize, RepoError> {
    let mut total = 0usize;
    let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;

    loop {
        let mut request = repo
            .client
            .query()
            .table_name(&repo.table)
            .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
            .filter_expression(
                "user_id = :user_id AND question_id = :question_id AND purpose = :purpose \
                 AND #status <> :failed",
            )
            .expression_attribute_names("#pk", attr::PK)
            .expression_attribute_names("#sk", attr::SK)
            .expression_attribute_names("#status", "status")
            .expression_attribute_values(":pk", AttributeValue::S(image_pk(group_id, cycle_id)))
            .expression_attribute_values(":prefix", AttributeValue::S("IMG#".into()))
            .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
            .expression_attribute_values(":question_id", AttributeValue::S(question_id.to_string()))
            .expression_attribute_values(":purpose", AttributeValue::S("response".into()))
            .expression_attribute_values(":failed", AttributeValue::S("failed".into()));
        if let Some(key) = exclusive_start_key.take() {
            request = request.set_exclusive_start_key(Some(key));
        }

        let resp = request.send().await?;
        total += resp.count.max(0) as usize;

        match resp.last_evaluated_key {
            Some(key) if !key.is_empty() => exclusive_start_key = Some(key),
            _ => break,
        }
    }

    Ok(total)
}

/// `PATCH /uploads/{imageId}` (§9.6) — overwrite just the caption. The caller
/// already resolved ownership via [`get_image`], so this doesn't re-check it.
pub async fn set_image_caption(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
    caption: Option<&str>,
) -> Result<(), RepoError> {
    let value = match caption {
        Some(c) => AttributeValue::S(c.to_owned()),
        None => AttributeValue::Null(true),
    };
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .update_expression("SET caption = :caption")
        .expression_attribute_values(":caption", value)
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}

/// `DELETE /uploads/{imageId}` (§9.4) — unconditional; marks the row `failed`
/// with `errorMessage: "DELETED"` so it's idempotent (an already-`failed` row
/// is a no-op success).
pub async fn mark_image_deleted(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .update_expression("SET #status = :failed, error_message = :msg")
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(":failed", AttributeValue::S("failed".into()))
        .expression_attribute_values(":msg", AttributeValue::S("DELETED".into()))
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}

/// `DELETE /avatars/{avatarId}` (§9.7) — same shape as [`mark_image_deleted`].
pub async fn mark_avatar_deleted(
    repo: &Repo,
    user_id: &UserId,
    avatar_id: &AvatarId,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(avatar_sk(avatar_id)))
        .update_expression("SET #status = :failed, error_message = :msg")
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(":failed", AttributeValue::S("failed".into()))
        .expression_attribute_values(":msg", AttributeValue::S("DELETED".into()))
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}
