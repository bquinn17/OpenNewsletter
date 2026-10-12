//! The real [`MediaObjectStore`] and [`CdnInvalidator`], over the AWS SDK.

use crate::{CdnInvalidator, InvalidationError, MediaObjectStore, ObjectStoreError};
use async_trait::async_trait;
use aws_sdk_cloudfront::types::{InvalidationBatch, Paths};

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

pub struct CloudFrontInvalidator {
    client: aws_sdk_cloudfront::Client,
    distribution_id: String,
}

impl CloudFrontInvalidator {
    pub fn new(client: aws_sdk_cloudfront::Client, distribution_id: String) -> Self {
        Self {
            client,
            distribution_id,
        }
    }
}

#[async_trait]
impl CdnInvalidator for CloudFrontInvalidator {
    async fn invalidate(&self, paths: &[String]) -> Result<(), InvalidationError> {
        let quantity = i32::try_from(paths.len())
            .map_err(|_| InvalidationError("too many invalidation paths".into()))?;
        let paths = Paths::builder()
            .quantity(quantity)
            .set_items(Some(paths.to_vec()))
            .build()
            .map_err(|e| InvalidationError(format!("{e:?}")))?;
        // CloudFront dedupes on `CallerReference`, so each request needs its own.
        let batch = InvalidationBatch::builder()
            .paths(paths)
            .caller_reference(uuid::Uuid::new_v4().to_string())
            .build()
            .map_err(|e| InvalidationError(format!("{e:?}")))?;
        self.client
            .create_invalidation()
            .distribution_id(&self.distribution_id)
            .invalidation_batch(batch)
            .send()
            .await
            .map_err(|e| InvalidationError(format!("{e:?}")))?;
        Ok(())
    }
}
