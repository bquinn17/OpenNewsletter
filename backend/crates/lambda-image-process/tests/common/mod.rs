//! Shared test setup for `lambda-image-process` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup via the `test-utils`
//! feature rather than duplicating it.
#![allow(dead_code)]

use chrono::{TimeZone, Utc};
use domain::{AvatarId, AvatarMedia, CycleId, GroupId, ImageId, ImageMedia, ImageMimeType};
use domain::{ImagePurpose, MediaStatus, QuestionId, UserId};
use persistence::{test_factories, Repo};
use serde_json::json;

pub async fn make_repo() -> (
    testcontainers::ContainerAsync<testcontainers_modules::dynamodb_local::DynamoDb>,
    Repo,
) {
    test_factories::make_test_repo().await
}

pub fn pending_image(
    group_id: &str,
    cycle_id: &str,
    image_id: &str,
    question_id: &str,
    user_id: &str,
) -> ImageMedia {
    ImageMedia {
        image_id: ImageId::new(image_id),
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: Some(QuestionId::new(question_id)),
        purpose: ImagePurpose::Response,
        mime_type: ImageMimeType::Jpeg,
        original_key: format!(
            "uploads/{group_id}/{cycle_id}/{question_id}/{user_id}/{image_id}.jpg"
        ),
        display_key: None,
        thumb_key: None,
        status: MediaStatus::Pending,
        bytes: 0,
        width: None,
        height: None,
        caption: None,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap(),
        processed_at: None,
        error_message: None,
        attached_comment_id: None,
    }
}

pub fn pending_avatar(user_id: &str, avatar_id: &str) -> AvatarMedia {
    AvatarMedia {
        avatar_id: AvatarId::new(avatar_id),
        user_id: UserId::new(user_id),
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("uploads/{user_id}/{avatar_id}.jpg"),
        display_key: None,
        status: MediaStatus::Pending,
        bytes: 0,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap(),
        processed_at: None,
        error_message: None,
    }
}

/// Builds an `aws_lambda_events::event::s3::S3Event` with one record, in the
/// real S3-notification JSON shape, for `bucket`/`key` (pass the already
/// URL-encoded key, exactly as S3 would deliver it).
pub fn s3_event(bucket: &str, key: &str) -> aws_lambda_events::event::s3::S3Event {
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
