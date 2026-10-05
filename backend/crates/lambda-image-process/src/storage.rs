//! S3 access behind a small trait, so the handler can be exercised against an
//! in-memory fake in tests instead of real S3 (`plans/08-media-uploads.md`
//! §5, testability note from the milestone brief).

use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
#[error("object store error: {0}")]
pub struct StoreError(pub String);

#[async_trait]
pub trait ObjectStore: Send + Sync {
    /// The object's size in bytes, without downloading its body — used to
    /// reject an oversized upload before paying for a GET
    /// (`plans/08-media-uploads.md` §3.2).
    async fn content_length(&self, bucket: &str, key: &str) -> Result<u64, StoreError>;
    async fn get(&self, bucket: &str, key: &str) -> Result<Vec<u8>, StoreError>;
    async fn put(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
        cache_control: &str,
    ) -> Result<(), StoreError>;
    async fn delete(&self, bucket: &str, key: &str) -> Result<(), StoreError>;
}

pub struct S3Store {
    client: aws_sdk_s3::Client,
}

impl S3Store {
    pub fn new(client: aws_sdk_s3::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ObjectStore for S3Store {
    async fn content_length(&self, bucket: &str, key: &str) -> Result<u64, StoreError> {
        let head = self
            .client
            .head_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StoreError(format!("{e:?}")))?;
        Ok(head.content_length().unwrap_or_default().max(0) as u64)
    }

    async fn get(&self, bucket: &str, key: &str) -> Result<Vec<u8>, StoreError> {
        let object = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StoreError(format!("{e:?}")))?;
        let bytes = object
            .body
            .collect()
            .await
            .map_err(|e| StoreError(format!("{e:?}")))?
            .into_bytes();
        Ok(bytes.to_vec())
    }

    async fn put(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
        cache_control: &str,
    ) -> Result<(), StoreError> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body.into())
            .content_type(content_type)
            .cache_control(cache_control)
            .send()
            .await
            .map_err(|e| StoreError(format!("{e:?}")))?;
        Ok(())
    }

    async fn delete(&self, bucket: &str, key: &str) -> Result<(), StoreError> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| StoreError(format!("{e:?}")))?;
        Ok(())
    }
}

/// In-memory [`ObjectStore`] fake, used by this crate's own unit and
/// integration tests in place of real S3. Not feature-gated: integration
/// tests under `tests/` link the library as an ordinary external crate, so a
/// `cfg(test)` gate here would hide it from them — see `tests/handler.rs`.
pub mod fake {
    use super::{ObjectStore, StoreError};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone)]
    pub struct StoredObject {
        pub bytes: Vec<u8>,
        pub content_type: String,
        pub cache_control: String,
    }

    #[derive(Default)]
    pub struct FakeStore {
        objects: Mutex<HashMap<(String, String), StoredObject>>,
    }

    impl FakeStore {
        pub fn new() -> Self {
            Self::default()
        }

        /// Seeds an object as if it had already landed in the bucket (e.g.
        /// an uploaded original), for a test's arrange step.
        pub async fn seed(&self, bucket: &str, key: &str, bytes: Vec<u8>) {
            self.objects.lock().await.insert(
                (bucket.to_string(), key.to_string()),
                StoredObject {
                    bytes,
                    content_type: String::new(),
                    cache_control: String::new(),
                },
            );
        }

        pub async fn get_object(&self, bucket: &str, key: &str) -> Option<StoredObject> {
            self.objects
                .lock()
                .await
                .get(&(bucket.to_string(), key.to_string()))
                .cloned()
        }

        pub async fn contains(&self, bucket: &str, key: &str) -> bool {
            self.objects
                .lock()
                .await
                .contains_key(&(bucket.to_string(), key.to_string()))
        }
    }

    #[async_trait]
    impl ObjectStore for FakeStore {
        async fn content_length(&self, bucket: &str, key: &str) -> Result<u64, StoreError> {
            self.get_object(bucket, key)
                .await
                .map(|o| o.bytes.len() as u64)
                .ok_or_else(|| StoreError("not found".into()))
        }

        async fn get(&self, bucket: &str, key: &str) -> Result<Vec<u8>, StoreError> {
            self.get_object(bucket, key)
                .await
                .map(|o| o.bytes)
                .ok_or_else(|| StoreError("not found".into()))
        }

        async fn put(
            &self,
            bucket: &str,
            key: &str,
            body: Vec<u8>,
            content_type: &str,
            cache_control: &str,
        ) -> Result<(), StoreError> {
            self.objects.lock().await.insert(
                (bucket.to_string(), key.to_string()),
                StoredObject {
                    bytes: body,
                    content_type: content_type.to_string(),
                    cache_control: cache_control.to_string(),
                },
            );
            Ok(())
        }

        async fn delete(&self, bucket: &str, key: &str) -> Result<(), StoreError> {
            self.objects
                .lock()
                .await
                .remove(&(bucket.to_string(), key.to_string()));
            Ok(())
        }
    }
}
