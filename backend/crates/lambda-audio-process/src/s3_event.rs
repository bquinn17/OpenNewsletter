//! A minimal serde model of an S3 event notification, mirroring
//! `lambda-image-process::s3_event`: `aws_lambda_events`' `s3` feature pulls
//! in `chrono`, so this crate models only the bucket name and object key
//! [`handler::handle_event`](crate::handler::handle_event) reads. Every other
//! field in AWS's notification JSON is ignored by serde.

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
    /// URL-encoded exactly as S3 delivers it; [`crate::key_parse`]
    /// decodes it before use.
    #[serde(default)]
    pub key: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn reads_the_bucket_and_key_and_ignores_other_fields() {
        let value = json!({
            "Records": [{
                "eventTime": "2026-06-02T12:00:00.000Z",
                "eventName": "ObjectCreated:Put",
                "s3": {
                    "bucket": { "name": "b", "arn": "arn:aws:s3:::b" },
                    "object": { "key": "audio-uploads/g1/202606/q1/u1/a%2B1.webm", "size": 1024 }
                }
            }]
        });
        let event: S3Event = serde_json::from_value(value).expect("event deserializes");
        assert_eq!(event.records[0].s3.bucket.name.as_deref(), Some("b"));
        assert_eq!(
            event.records[0].s3.object.key.as_deref(),
            Some("audio-uploads/g1/202606/q1/u1/a%2B1.webm")
        );
    }
}
