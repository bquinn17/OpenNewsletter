//! A minimal serde model of an S3 event notification (`docs/design/media.md`
//! §5.2), standing in for `aws_lambda_events::event::s3::S3Event`.
//!
//! `aws_lambda_events`' `s3` feature pulls in `chrono` (for `eventTime`) in
//! every version, including 1.x, so this crate defines its own model instead
//! — only the fields [`handler::handle_event`](crate::handler::handle_event)
//! actually reads: the bucket name and object key of each record. Every
//! other field in AWS's notification JSON (`eventVersion`, `eventSource`,
//! `awsRegion`, `eventTime`, `eventName`, `userIdentity`,
//! `requestParameters`, `responseElements`, `s3.s3SchemaVersion`,
//! `s3.configurationId`, `s3.bucket.ownerIdentity`, `s3.bucket.arn`,
//! `s3.object.size`, `s3.object.eTag`, `s3.object.sequencer`, ...) is
//! deserialized and discarded by serde's default "ignore unknown fields"
//! behavior, so adding a field to a real payload never breaks this.

use serde::Deserialize;

/// The top-level S3 event notification body, as delivered by the S3 ->
/// Lambda trigger.
#[derive(Debug, Clone, Deserialize)]
pub struct S3Event {
    #[serde(default, rename = "Records")]
    pub records: Vec<S3EventRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3EventRecord {
    pub s3: S3Entity,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Entity {
    pub bucket: S3Bucket,
    pub object: S3Object,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Bucket {
    /// `None` is never produced by a real S3 notification; kept optional
    /// (like `aws_lambda_events`' own type) so a malformed/truncated event
    /// fails at `handle_record`'s `unwrap_or_default` rather than at parse.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3Object {
    /// URL-encoded exactly as S3 delivers it; [`crate::key_parse::decode_s3_key`]
    /// decodes it before use.
    #[serde(default)]
    pub key: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    /// A full, realistic `ObjectCreated:Put` notification, with every field
    /// AWS documents for it, including a key with `+` and both `%2B` and
    /// `%20` escapes.
    #[test]
    fn deserializes_a_realistic_object_created_put_notification() {
        let value = json!({
            "Records": [{
                "eventVersion": "2.1",
                "eventSource": "aws:s3",
                "awsRegion": "us-east-1",
                "eventTime": "2026-06-02T12:00:00.000Z",
                "eventName": "ObjectCreated:Put",
                "userIdentity": { "principalId": "AWS:EXAMPLE" },
                "requestParameters": { "sourceIPAddress": "127.0.0.1" },
                "responseElements": {
                    "x-amz-request-id": "EXAMPLE123456789",
                    "x-amz-id-2": "EXAMPLE123/EXAMPLE123/EXAMPLE123456789EXAMPLE123456789EXAMPLE1234="
                },
                "s3": {
                    "s3SchemaVersion": "1.0",
                    "configurationId": "testConfigRule",
                    "bucket": {
                        "name": "opennewsletter-media-originals",
                        "ownerIdentity": { "principalId": "EXAMPLE" },
                        "arn": "arn:aws:s3:::opennewsletter-media-originals"
                    },
                    "object": {
                        "key": "uploads/g1/202606/q1/u1/img%2B1%20name.jpg",
                        "size": 102_400,
                        "eTag": "0123456789abcdef0123456789abcdef",
                        "sequencer": "0A1B2C3D4E5F678901"
                    }
                }
            }]
        });

        let event: S3Event = serde_json::from_value(value).expect("event deserializes");
        assert_eq!(event.records.len(), 1);
        let record = &event.records[0];
        assert_eq!(
            record.s3.bucket.name.as_deref(),
            Some("opennewsletter-media-originals")
        );
        assert_eq!(
            record.s3.object.key.as_deref(),
            Some("uploads/g1/202606/q1/u1/img%2B1%20name.jpg")
        );
    }

    /// Tolerates an event missing fields this crate doesn't read (`eventTime`,
    /// `userIdentity`, etc.) — a real payload AWS could in principle trim.
    #[test]
    fn tolerates_missing_unused_fields() {
        let value = json!({
            "Records": [{
                "s3": {
                    "bucket": { "name": "b" },
                    "object": { "key": "k" }
                }
            }]
        });
        let event: S3Event = serde_json::from_value(value).expect("event deserializes");
        assert_eq!(event.records[0].s3.bucket.name.as_deref(), Some("b"));
        assert_eq!(event.records[0].s3.object.key.as_deref(), Some("k"));
    }
}
