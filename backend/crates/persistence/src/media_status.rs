//! Status-transition updates for `ImageMedia`/`AvatarMedia` rows
//! (`plans/02-data-model-dynamodb.md` §2.10, §2.16), written by
//! `lambda-image-process` once a variant has been generated or processing has
//! failed terminally. Every transition is conditioned on `status = pending`
//! so a retried S3 event — this Lambda's idempotency story
//! (`plans/08-media-uploads.md` §5.2) — never clobbers a row another
//! invocation already finished.

use crate::error::{self, RepoError};
use crate::keys::{attr, avatar_sk, image_pk, image_sk, user_pk};
use crate::repo::Repo;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::update_item::{UpdateItemError, UpdateItemOutput};
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Utc};
use domain::{AvatarId, CycleId, GroupId, ImageId, UserId};

/// Outcome of a conditional status-transition update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// The row was `pending` and is now updated.
    Updated,
    /// The row was already `ready` or `failed` — a concurrent or retried
    /// invocation finished it first. Not an error; the caller has nothing
    /// left to do.
    AlreadyFinal,
}

fn outcome_from_update<R: std::fmt::Debug>(
    result: Result<UpdateItemOutput, SdkError<UpdateItemError, R>>,
) -> Result<UpdateOutcome, RepoError> {
    match result {
        Ok(_) => Ok(UpdateOutcome::Updated),
        Err(err) => match error::from_update_item_error(err) {
            RepoError::ConditionalCheckFailed => Ok(UpdateOutcome::AlreadyFinal),
            other => Err(other),
        },
    }
}

/// Fields for [`mark_image_ready`], grouped to stay under clippy's
/// too-many-arguments threshold (mirrors `responses::ResponseSave`).
#[derive(Debug, Clone, Copy)]
pub struct ImageReady<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub image_id: &'a ImageId,
    pub width: u32,
    pub height: u32,
    pub display_key: &'a str,
    pub thumb_key: &'a str,
    pub processed_at: DateTime<Utc>,
}

/// `pending -> ready` for an `ImageMedia` row: records the oriented original's
/// dimensions and both processed-bucket keys.
pub async fn mark_image_ready(
    repo: &Repo,
    ready: &ImageReady<'_>,
) -> Result<UpdateOutcome, RepoError> {
    let ImageReady {
        group_id,
        cycle_id,
        image_id,
        width,
        height,
        display_key,
        thumb_key,
        processed_at,
    } = *ready;
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .update_expression(
            "SET #s = :ready, width = :width, height = :height, \
             display_key = :display_key, thumb_key = :thumb_key, processed_at = :processed_at",
        )
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .expression_attribute_values(":ready", AttributeValue::S("ready".into()))
        .expression_attribute_values(":width", AttributeValue::N(width.to_string()))
        .expression_attribute_values(":height", AttributeValue::N(height.to_string()))
        .expression_attribute_values(":display_key", AttributeValue::S(display_key.into()))
        .expression_attribute_values(":thumb_key", AttributeValue::S(thumb_key.into()))
        .expression_attribute_values(
            ":processed_at",
            AttributeValue::S(processed_at.to_rfc3339()),
        )
        .send()
        .await;
    outcome_from_update(result)
}

/// `pending -> failed` for an `ImageMedia` row.
pub async fn mark_image_failed(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
    error_message: &str,
    processed_at: DateTime<Utc>,
) -> Result<UpdateOutcome, RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .update_expression(
            "SET #s = :failed, error_message = :error_message, processed_at = :processed_at",
        )
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .expression_attribute_values(":failed", AttributeValue::S("failed".into()))
        .expression_attribute_values(":error_message", AttributeValue::S(error_message.into()))
        .expression_attribute_values(
            ":processed_at",
            AttributeValue::S(processed_at.to_rfc3339()),
        )
        .send()
        .await;
    outcome_from_update(result)
}

/// `pending -> ready` for an `AvatarMedia` row.
pub async fn mark_avatar_ready(
    repo: &Repo,
    user_id: &UserId,
    avatar_id: &AvatarId,
    display_key: &str,
    processed_at: DateTime<Utc>,
) -> Result<UpdateOutcome, RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(avatar_sk(avatar_id)))
        .update_expression(
            "SET #s = :ready, display_key = :display_key, processed_at = :processed_at",
        )
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .expression_attribute_values(":ready", AttributeValue::S("ready".into()))
        .expression_attribute_values(":display_key", AttributeValue::S(display_key.into()))
        .expression_attribute_values(
            ":processed_at",
            AttributeValue::S(processed_at.to_rfc3339()),
        )
        .send()
        .await;
    outcome_from_update(result)
}

/// `pending -> failed` for an `AvatarMedia` row.
pub async fn mark_avatar_failed(
    repo: &Repo,
    user_id: &UserId,
    avatar_id: &AvatarId,
    error_message: &str,
    processed_at: DateTime<Utc>,
) -> Result<UpdateOutcome, RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(avatar_sk(avatar_id)))
        .update_expression(
            "SET #s = :failed, error_message = :error_message, processed_at = :processed_at",
        )
        .condition_expression("#s = :pending")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":pending", AttributeValue::S("pending".into()))
        .expression_attribute_values(":failed", AttributeValue::S("failed".into()))
        .expression_attribute_values(":error_message", AttributeValue::S(error_message.into()))
        .expression_attribute_values(
            ":processed_at",
            AttributeValue::S(processed_at.to_rfc3339()),
        )
        .send()
        .await;
    outcome_from_update(result)
}
