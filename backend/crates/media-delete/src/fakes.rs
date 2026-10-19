//! An in-memory recording fake for tests in this and the calling crates. Not
//! feature-gated: other crates' integration tests link this library as an
//! ordinary dependency, so a `cfg(test)` gate would hide it from them.

use crate::{MediaDeleter, MediaObjectStore, ObjectStoreError};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Records every `(bucket, key)` deleted; [`Self::set_failing`] makes every
/// later delete fail instead.
#[derive(Default)]
pub struct RecordingObjectStore {
    deleted: Mutex<Vec<(String, String)>>,
    failing: AtomicBool,
}

impl RecordingObjectStore {
    pub fn set_failing(&self, failing: bool) {
        self.failing.store(failing, Ordering::SeqCst);
    }

    pub async fn deleted(&self) -> Vec<(String, String)> {
        self.deleted.lock().await.clone()
    }
}

#[async_trait]
impl MediaObjectStore for RecordingObjectStore {
    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), ObjectStoreError> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(ObjectStoreError("injected failure".into()));
        }
        self.deleted
            .lock()
            .await
            .push((bucket.to_owned(), key.to_owned()));
        Ok(())
    }
}

/// A [`MediaDeleter`] over a fresh fake, returned alongside it so a test can
/// inspect or break it.
pub fn fake_deleter(
    originals_bucket: &str,
    processed_bucket: &str,
) -> (MediaDeleter, Arc<RecordingObjectStore>) {
    let store = Arc::new(RecordingObjectStore::default());
    let deleter = MediaDeleter {
        store: store.clone(),
        originals_bucket: originals_bucket.to_owned(),
        processed_bucket: processed_bucket.to_owned(),
    };
    (deleter, store)
}
