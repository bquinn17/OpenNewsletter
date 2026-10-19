//! The real [`MediaObjectStore`], over the AWS SDK.

use crate::{MediaObjectStore, ObjectStoreError};
use async_trait::async_trait;

pub struct S3ObjectStore {
    client: aws_sdk_s3::Client,
}

impl S3ObjectStore {
    pub fn new(client: aws_sdk_s3::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl MediaObjectStore for S3ObjectStore {
    /// Unversioned buckets answer `DeleteObject` on a missing key with 204,
    /// so a missing object needs no special case.
    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), ObjectStoreError> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| ObjectStoreError(format!("{e:?}")))?;
        Ok(())
    }
}
