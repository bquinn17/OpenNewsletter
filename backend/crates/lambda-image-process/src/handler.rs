//! The S3-trigger handler (`docs/design/media.md` §5.2).
//!
//! Records are processed independently: a terminal content problem (too
//! large, undecodable, malformed key, missing/already-finished row) on one
//! record is logged and marks that row `failed`, without touching the rest
//! of the batch. A transient AWS failure (S3 or DynamoDB) on any record
//! makes the whole invocation return `Err` so the Lambda's built-in retry
//! re-delivers the event; reprocessing is safe because every status update
//! is conditioned on `status = pending`.

use aws_lambda_events::event::s3::S3Event;
use chrono::Utc;
use domain::MediaStatus;
use persistence::media;
use persistence::media_status::{self, ImageReady, UpdateOutcome};
use tracing::{info, warn};

use crate::error::TransientError;
use crate::key_parse::{
    decode_s3_key, parse_avatar_key, parse_image_key, ParsedAvatarKey, ParsedImageKey,
};
use crate::process::{process_avatar, process_image, ProcessError};
use crate::state::AppState;

const IMAGE_TOO_LARGE: &str = "IMAGE_TOO_LARGE";
const IMAGE_DECODE_FAILED: &str = "IMAGE_DECODE_FAILED";

/// The `Cache-Control` a processed variant is stored with. `private` because
/// there is no shared cache in front of the processed bucket to benefit from
/// `public` (`docs/design/media.md` §5.1).
fn cache_control(max_age_seconds: u64) -> String {
    format!("private, max-age={max_age_seconds}, immutable")
}

/// Processes every record in the event, continuing past a terminal failure
/// on any individual record. Returns the first transient error encountered
/// (if any) after attempting every record.
pub async fn handle_event(state: &AppState, event: S3Event) -> Result<(), TransientError> {
    let mut first_transient_error = None;
    for record in event.records {
        let bucket = record.s3.bucket.name.unwrap_or_default();
        let raw_key = record.s3.object.key.unwrap_or_default();
        if let Err(err) = handle_record(state, &bucket, &raw_key).await {
            tracing::error!(error = ?err, bucket = %bucket, key = %raw_key, "image-process record failed transiently");
            first_transient_error.get_or_insert(err);
        }
    }
    match first_transient_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

async fn handle_record(
    state: &AppState,
    bucket: &str,
    raw_key: &str,
) -> Result<(), TransientError> {
    let key = decode_s3_key(raw_key);

    if bucket == state.buckets.media_originals {
        handle_image_record(state, bucket, &key).await
    } else if bucket == state.buckets.avatars_originals {
        handle_avatar_record(state, bucket, &key).await
    } else {
        warn!(bucket = %bucket, key = %key, "unrecognized originals bucket; skipping");
        Ok(())
    }
}

async fn handle_image_record(
    state: &AppState,
    bucket: &str,
    key: &str,
) -> Result<(), TransientError> {
    let Some(parsed) = parse_image_key(key) else {
        warn!(bucket = %bucket, key = %key, "malformed image key; skipping");
        return Ok(());
    };

    let row = media::get_image(
        &state.repo,
        &parsed.group_id,
        &parsed.cycle_id,
        &parsed.image_id,
    )
    .await?;
    let Some(row) = row else {
        warn!(group_id = %parsed.group_id, cycle_id = %parsed.cycle_id, image_id = %parsed.image_id, "image row missing; skipping");
        return Ok(());
    };
    if row.status != MediaStatus::Pending {
        info!(image_id = %parsed.image_id, status = ?row.status, "image already processed; skipping");
        return Ok(());
    }

    let Some(content_length) = state.store.content_length(bucket, key).await? else {
        info!(image_id = %parsed.image_id, "original already deleted; skipping");
        return Ok(());
    };
    if content_length > shared::config::MAX_IMAGE_BYTES {
        return fail_image(state, &parsed, bucket, key, IMAGE_TOO_LARGE).await;
    }

    let Some(bytes) = state.store.get(bucket, key).await? else {
        info!(image_id = %parsed.image_id, "original already deleted; skipping");
        return Ok(());
    };
    let processed = tokio::task::spawn_blocking(move || process_image(&bytes))
        .await
        .map_err(|e| TransientError::Join(e.to_string()))?;
    let variants = match processed {
        Ok(v) => v,
        Err(ProcessError::DecodeFailed) => {
            return fail_image(state, &parsed, bucket, key, IMAGE_DECODE_FAILED).await;
        }
        Err(ProcessError::TooLarge) => {
            return fail_image(state, &parsed, bucket, key, IMAGE_TOO_LARGE).await;
        }
    };

    let display_key = format!(
        "img/{}/{}/{}/{}/{}/display.{}",
        parsed.group_id,
        parsed.cycle_id,
        parsed.question_id,
        parsed.user_id,
        parsed.image_id,
        variants.display.format.extension()
    );
    let thumb_key = format!(
        "img/{}/{}/{}/{}/{}/thumb.{}",
        parsed.group_id,
        parsed.cycle_id,
        parsed.question_id,
        parsed.user_id,
        parsed.image_id,
        variants.thumb.format.extension()
    );
    let image_cache_control = cache_control(shared::config::IMAGE_CACHE_MAX_AGE_SECONDS);

    state
        .store
        .put(
            &state.buckets.processed,
            &display_key,
            variants.display.bytes,
            variants.display.format.content_type(),
            &image_cache_control,
        )
        .await?;
    state
        .store
        .put(
            &state.buckets.processed,
            &thumb_key,
            variants.thumb.bytes,
            variants.thumb.format.content_type(),
            &image_cache_control,
        )
        .await?;

    let outcome = media_status::mark_image_ready(
        &state.repo,
        &ImageReady {
            group_id: &parsed.group_id,
            cycle_id: &parsed.cycle_id,
            image_id: &parsed.image_id,
            width: variants.width,
            height: variants.height,
            display_key: &display_key,
            thumb_key: &thumb_key,
            processed_at: Utc::now(),
        },
    )
    .await?;
    match outcome {
        UpdateOutcome::Updated => {
            // The original is no longer needed once the row is ready; a
            // 7-day bucket lifecycle expiry (infra) is the safety net if
            // this delete itself fails (`docs/design/media.md` §5.1).
            delete_original_best_effort(state, bucket, key).await;
        }
        UpdateOutcome::AlreadyFinal => {
            // The conditional update also fails when the row is gone: the image was
            // deleted while this invocation processed it. The deleter saw no
            // processed keys on the pending row, so the objects just written are
            // ours to remove. Leave the original alone either way: the deleter
            // owns it on a delete, and the winning invocation owns it otherwise.
            let row = media::get_image(
                &state.repo,
                &parsed.group_id,
                &parsed.cycle_id,
                &parsed.image_id,
            )
            .await?;
            if row.is_none() {
                info!(image_id = %parsed.image_id, "image deleted during processing; removing its processed objects");
                state
                    .store
                    .delete(&state.buckets.processed, &display_key)
                    .await?;
                state
                    .store
                    .delete(&state.buckets.processed, &thumb_key)
                    .await?;
            } else {
                info!(image_id = %parsed.image_id, "image row finished concurrently; leaving the winner's write in place");
            }
        }
    }
    Ok(())
}

/// Deletes the original once its row is final (`ready` or `failed`). Best
/// effort: a failure here only leaves the original behind (purged by the
/// bucket's lifecycle rule), never fails the invocation — returning `Err`
/// would make Lambda retry the whole record, which would just skip the
/// no-longer-`pending` row without reaching this delete again.
async fn delete_original_best_effort(state: &AppState, bucket: &str, key: &str) {
    if let Err(e) = state.store.delete(bucket, key).await {
        tracing::error!(error = ?e, bucket = %bucket, key = %key, "failed to delete original after processing");
    }
}

async fn fail_image(
    state: &AppState,
    parsed: &ParsedImageKey,
    bucket: &str,
    key: &str,
    reason: &str,
) -> Result<(), TransientError> {
    media_status::mark_image_failed(
        &state.repo,
        &parsed.group_id,
        &parsed.cycle_id,
        &parsed.image_id,
        reason,
        Utc::now(),
    )
    .await?;
    delete_original_best_effort(state, bucket, key).await;
    Ok(())
}

async fn handle_avatar_record(
    state: &AppState,
    bucket: &str,
    key: &str,
) -> Result<(), TransientError> {
    let Some(parsed) = parse_avatar_key(key) else {
        warn!(bucket = %bucket, key = %key, "malformed avatar key; skipping");
        return Ok(());
    };

    let row = media::get_avatar(&state.repo, &parsed.user_id, &parsed.avatar_id).await?;
    let Some(row) = row else {
        warn!(user_id = %parsed.user_id, avatar_id = %parsed.avatar_id, "avatar row missing; skipping");
        return Ok(());
    };
    if row.status != MediaStatus::Pending {
        info!(avatar_id = %parsed.avatar_id, status = ?row.status, "avatar already processed; skipping");
        return Ok(());
    }

    let Some(content_length) = state.store.content_length(bucket, key).await? else {
        info!(avatar_id = %parsed.avatar_id, "original already deleted; skipping");
        return Ok(());
    };
    if content_length > shared::config::MAX_AVATAR_BYTES {
        return fail_avatar(state, &parsed, bucket, key, IMAGE_TOO_LARGE).await;
    }

    let Some(bytes) = state.store.get(bucket, key).await? else {
        info!(avatar_id = %parsed.avatar_id, "original already deleted; skipping");
        return Ok(());
    };
    let processed = tokio::task::spawn_blocking(move || process_avatar(&bytes))
        .await
        .map_err(|e| TransientError::Join(e.to_string()))?;
    let variant = match processed {
        Ok(v) => v,
        Err(ProcessError::DecodeFailed) => {
            return fail_avatar(state, &parsed, bucket, key, IMAGE_DECODE_FAILED).await;
        }
        Err(ProcessError::TooLarge) => {
            return fail_avatar(state, &parsed, bucket, key, IMAGE_TOO_LARGE).await;
        }
    };

    let display_key = format!(
        "avatar/{}/display.{}",
        parsed.avatar_id,
        variant.display.format.extension()
    );
    state
        .store
        .put(
            &state.buckets.processed,
            &display_key,
            variant.display.bytes,
            variant.display.format.content_type(),
            &cache_control(shared::config::AVATAR_CACHE_MAX_AGE_SECONDS),
        )
        .await?;

    let outcome = media_status::mark_avatar_ready(
        &state.repo,
        &parsed.user_id,
        &parsed.avatar_id,
        &display_key,
        Utc::now(),
    )
    .await?;
    match outcome {
        UpdateOutcome::Updated => {
            delete_original_best_effort(state, bucket, key).await;
        }
        UpdateOutcome::AlreadyFinal => {
            // As for images: a missing row means the avatar was deleted mid-processing.
            let row = media::get_avatar(&state.repo, &parsed.user_id, &parsed.avatar_id).await?;
            if row.is_none() {
                info!(avatar_id = %parsed.avatar_id, "avatar deleted during processing; removing its processed object");
                state
                    .store
                    .delete(&state.buckets.processed, &display_key)
                    .await?;
            } else {
                info!(avatar_id = %parsed.avatar_id, "avatar row finished concurrently; leaving the winner's write in place");
            }
        }
    }
    Ok(())
}

async fn fail_avatar(
    state: &AppState,
    parsed: &ParsedAvatarKey,
    bucket: &str,
    key: &str,
    reason: &str,
) -> Result<(), TransientError> {
    media_status::mark_avatar_failed(
        &state.repo,
        &parsed.user_id,
        &parsed.avatar_id,
        reason,
        Utc::now(),
    )
    .await?;
    delete_original_best_effort(state, bucket, key).await;
    Ok(())
}
