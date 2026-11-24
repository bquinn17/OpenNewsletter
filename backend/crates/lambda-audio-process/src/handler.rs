//! The S3-trigger handler (`docs/design/media.md` §12).
//!
//! Records are processed independently: a terminal content problem (too
//! large, undecodable, too long, malformed key, missing/already-finished
//! row) on one record is logged and marks that row `failed`, without
//! touching the rest of the batch. A transient AWS failure (S3 or DynamoDB)
//! on any record makes the whole invocation return `Err` so the Lambda's
//! built-in retry re-delivers the event; reprocessing is safe because every
//! status update is conditioned on `status = pending`.

use crate::s3_event::S3Event;
use domain::{MediaStatus, Timestamp};
use persistence::media;
use persistence::media_status::{self, AudioReady, UpdateOutcome};
use shared::config::{AUDIO_PEAK_COUNT, MAX_AUDIO_BYTES};
use tracing::{info, warn};

use crate::error::TransientError;
use crate::key_parse::{decode_s3_key, parse_audio_key, ParsedAudioKey};
use crate::process::{compute_peaks, duration_ms, duration_within_cap};
use crate::state::AppState;
use crate::transcode::{TranscodeError, DECODE_SAMPLE_RATE_HZ};

const AUDIO_TOO_LARGE: &str = "AUDIO_TOO_LARGE";
const AUDIO_TOO_LONG: &str = "AUDIO_TOO_LONG";
const AUDIO_DECODE_FAILED: &str = "AUDIO_DECODE_FAILED";

/// The `Cache-Control` a processed clip is stored with. `private` because
/// there is no shared cache in front of the processed bucket to benefit
/// from `public` (`docs/design/media.md` §5.1, mirrored for clips).
fn cache_control() -> String {
    format!(
        "private, max-age={}, immutable",
        shared::config::AUDIO_CACHE_MAX_AGE_SECONDS
    )
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
            tracing::error!(error = ?err, bucket = %bucket, key = %raw_key, "audio-process record failed transiently");
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

    if bucket != state.buckets.media_originals {
        warn!(bucket = %bucket, key = %key, "unrecognized originals bucket; skipping");
        return Ok(());
    }

    let Some(parsed) = parse_audio_key(&key) else {
        warn!(bucket = %bucket, key = %key, "malformed audio key; skipping");
        return Ok(());
    };

    let row = media::get_audio(
        &state.repo,
        &parsed.group_id,
        &parsed.cycle_id,
        &parsed.audio_id,
    )
    .await?;
    let Some(row) = row else {
        warn!(group_id = %parsed.group_id, cycle_id = %parsed.cycle_id, audio_id = %parsed.audio_id, "audio row missing; skipping");
        return Ok(());
    };
    if row.status != MediaStatus::Pending {
        info!(audio_id = %parsed.audio_id, status = ?row.status, "clip already processed; skipping");
        return Ok(());
    }

    let Some(content_length) = state.store.content_length(bucket, &key).await? else {
        info!(audio_id = %parsed.audio_id, "original already deleted; skipping");
        return Ok(());
    };
    if content_length > MAX_AUDIO_BYTES {
        return fail(state, &parsed, bucket, &key, AUDIO_TOO_LARGE).await;
    }

    let Some(bytes) = state.store.get(bucket, &key).await? else {
        info!(audio_id = %parsed.audio_id, "original already deleted; skipping");
        return Ok(());
    };

    let transcoder = state.transcoder.clone();
    let decode_bytes = bytes.clone();
    let decoded = tokio::task::spawn_blocking(move || transcoder.decode_samples(&decode_bytes))
        .await
        .map_err(|e| TransientError::Join(e.to_string()))?;
    let samples = match decoded {
        Ok(samples) => samples,
        Err(TranscodeError::NonZeroExit(_) | TranscodeError::NoSamples | TranscodeError::Io(_)) => {
            return fail(state, &parsed, bucket, &key, AUDIO_DECODE_FAILED).await;
        }
    };

    let duration = duration_ms(samples.len(), DECODE_SAMPLE_RATE_HZ);
    if !duration_within_cap(duration) {
        return fail(state, &parsed, bucket, &key, AUDIO_TOO_LONG).await;
    }
    let peaks = compute_peaks(&samples, AUDIO_PEAK_COUNT);

    let transcoder = state.transcoder.clone();
    let transcode_bytes = bytes;
    let transcoded =
        tokio::task::spawn_blocking(move || transcoder.transcode_opus(&transcode_bytes))
            .await
            .map_err(|e| TransientError::Join(e.to_string()))?;
    let clip_bytes = match transcoded {
        Ok(bytes) => bytes,
        Err(_) => return fail(state, &parsed, bucket, &key, AUDIO_DECODE_FAILED).await,
    };

    let clip_key = format!(
        "aud/{}/{}/{}/{}/{}/clip.webm",
        parsed.group_id, parsed.cycle_id, parsed.question_id, parsed.user_id, parsed.audio_id
    );
    state
        .store
        .put(
            &state.buckets.processed,
            &clip_key,
            clip_bytes,
            "audio/webm",
            &cache_control(),
        )
        .await?;

    let outcome = media_status::mark_audio_ready(
        &state.repo,
        &AudioReady {
            group_id: &parsed.group_id,
            cycle_id: &parsed.cycle_id,
            audio_id: &parsed.audio_id,
            duration_ms: duration,
            peaks: &peaks,
            clip_key: &clip_key,
            processed_at: Timestamp::now(),
        },
    )
    .await?;
    match outcome {
        UpdateOutcome::Updated => {
            // The original is no longer needed once the row is ready; a
            // 7-day bucket lifecycle expiry (infra) is the safety net if
            // this delete itself fails (`docs/design/media.md` §5.1,
            // extended to `audio-uploads/` for clips).
            delete_original_best_effort(state, bucket, &key).await;
        }
        UpdateOutcome::AlreadyFinal => {
            // Lost condition also fires when the row is gone: the clip was
            // deleted while this invocation processed it. Remove the clip
            // this invocation just wrote; leave the original alone either
            // way (the deleter owns it on a delete, the winner otherwise).
            let row = media::get_audio(
                &state.repo,
                &parsed.group_id,
                &parsed.cycle_id,
                &parsed.audio_id,
            )
            .await?;
            if row.is_none() {
                info!(audio_id = %parsed.audio_id, "clip deleted during processing; removing its processed object");
                state
                    .store
                    .delete(&state.buckets.processed, &clip_key)
                    .await?;
            } else {
                info!(audio_id = %parsed.audio_id, "clip row finished concurrently; leaving the winner's write in place");
            }
        }
    }
    Ok(())
}

/// Deletes the original once its row is final (`ready` or `failed`). Best
/// effort: a failure here only leaves the original behind (purged by the
/// bucket's lifecycle rule), never fails the invocation.
async fn delete_original_best_effort(state: &AppState, bucket: &str, key: &str) {
    if let Err(e) = state.store.delete(bucket, key).await {
        tracing::error!(error = ?e, bucket = %bucket, key = %key, "failed to delete original after processing");
    }
}

async fn fail(
    state: &AppState,
    parsed: &ParsedAudioKey,
    bucket: &str,
    key: &str,
    reason: &str,
) -> Result<(), TransientError> {
    media_status::mark_audio_failed(
        &state.repo,
        &parsed.group_id,
        &parsed.cycle_id,
        &parsed.audio_id,
        reason,
        Timestamp::now(),
    )
    .await?;
    delete_original_best_effort(state, bucket, key).await;
    Ok(())
}
