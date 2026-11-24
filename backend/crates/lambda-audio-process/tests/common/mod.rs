//! Shared test setup for `lambda-audio-process` integration tests. Reuses
//! `persistence`'s DDB-local table setup via the `test-utils` feature
//! rather than duplicating it.
#![allow(dead_code)]

use audio_process::transcode::{AudioTranscoder, TranscodeError};
use domain::{AudioId, AudioMedia, AudioMimeType, CycleId, GroupId, ImagePurpose, MediaStatus};
use domain::{QuestionId, Timestamp, UserId};
use persistence::{test_factories, Repo};
use serde_json::json;
use std::sync::Mutex;

pub async fn make_repo() -> (persistence::test_factories::TestTable, Repo) {
    test_factories::make_test_repo().await
}

pub fn pending_audio(
    group_id: &str,
    cycle_id: &str,
    audio_id: &str,
    question_id: &str,
    user_id: &str,
) -> AudioMedia {
    AudioMedia {
        audio_id: AudioId::new(audio_id),
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: Some(QuestionId::new(question_id)),
        purpose: ImagePurpose::Response,
        mime_type: AudioMimeType::Webm,
        original_key: format!(
            "audio-uploads/{group_id}/{cycle_id}/{question_id}/{user_id}/{audio_id}.webm"
        ),
        clip_key: None,
        status: MediaStatus::Pending,
        bytes: 0,
        duration_ms: None,
        peaks: None,
        caption: None,
        uploaded_at: "2026-06-02T12:00:00Z".parse::<Timestamp>().unwrap(),
        processed_at: None,
        error_message: None,
        attached_comment_id: None,
    }
}

/// Builds an `audio_process::s3_event::S3Event` with one record.
/// Mirrors `lambda-image-process`'s own fixture.
pub fn s3_event(bucket: &str, key: &str) -> audio_process::s3_event::S3Event {
    let value = json!({
        "Records": [{
            "eventVersion": "2.1",
            "eventSource": "aws:s3",
            "awsRegion": "us-east-1",
            "eventTime": "2026-06-02T12:00:00.000Z",
            "eventName": "ObjectCreated:Put",
            "userIdentity": { "principalId": "EXAMPLE" },
            "requestParameters": { "sourceIPAddress": "127.0.0.1" },
            "responseElements": {
                "x-amz-request-id": "EXAMPLE123456789",
                "x-amz-id-2": "EXAMPLE123/EXAMPLE123"
            },
            "s3": {
                "s3SchemaVersion": "1.0",
                "configurationId": "testConfigRule",
                "bucket": {
                    "name": bucket,
                    "ownerIdentity": { "principalId": "EXAMPLE" },
                    "arn": format!("arn:aws:s3:::{bucket}")
                },
                "object": {
                    "key": key,
                    "size": 1024,
                    "eTag": "0123456789abcdef0123456789abcdef",
                    "sequencer": "0A1B2C3D4E5F678901"
                }
            }
        }]
    });
    serde_json::from_value(value).expect("s3 event fixture deserializes")
}

/// A canned [`AudioTranscoder`] for handler-level tests that don't need a
/// real `ffmpeg` binary: `decode_samples`/`transcode_opus` return whatever
/// was configured, regardless of the input bytes.
pub struct FakeTranscoder {
    decode_result: Mutex<Result<Vec<i16>, String>>,
    transcode_result: Mutex<Result<Vec<u8>, String>>,
}

impl FakeTranscoder {
    /// `sample_count` samples of silence at a nonzero level, decoding to
    /// `duration_ms` at `audio_process::transcode::DECODE_SAMPLE_RATE_HZ`,
    /// and a non-empty fake "clip.webm" payload on transcode.
    pub fn with_duration_ms(duration_ms: u64) -> Self {
        let sample_count =
            (duration_ms * u64::from(audio_process::transcode::DECODE_SAMPLE_RATE_HZ)) / 1000;
        let samples = vec![1000i16; sample_count as usize];
        Self {
            decode_result: Mutex::new(Ok(samples)),
            transcode_result: Mutex::new(Ok(b"fake-opus-webm-bytes".to_vec())),
        }
    }

    pub fn decode_fails() -> Self {
        Self {
            decode_result: Mutex::new(Err("no audio stream".into())),
            transcode_result: Mutex::new(Ok(Vec::new())),
        }
    }
}

impl AudioTranscoder for FakeTranscoder {
    fn decode_samples(&self, _input: &[u8]) -> Result<Vec<i16>, TranscodeError> {
        self.decode_result
            .lock()
            .unwrap()
            .clone()
            .map_err(TranscodeError::NonZeroExit)
    }

    fn transcode_opus(&self, _input: &[u8]) -> Result<Vec<u8>, TranscodeError> {
        self.transcode_result
            .lock()
            .unwrap()
            .clone()
            .map_err(TranscodeError::NonZeroExit)
    }
}
