//! Real deletion of an uploaded image or avatar (`docs/design/media.md`
//! §8.0, §11.4; `docs/design/api.md` §9.4, §9.7): the S3 objects, then the
//! DynamoDB row. Any S3 failure returns an error and leaves the row, so a
//! retry redoes the steps, all of which are idempotent.
//!
//! Used by `lambda-media` (`DELETE /uploads`, `DELETE /avatars`),
//! `lambda-engagement` (a comment releasing its image) and `lambda-groups`
//! (`PATCH /me` replacing an avatar).

mod clients;
pub mod fakes;

pub use clients::S3ObjectStore;

use async_trait::async_trait;
use domain::{AvatarMedia, ImageMedia};
use persistence::{media, Repo, RepoError};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("object store error: {0}")]
pub struct ObjectStoreError(pub String);

#[derive(Debug, thiserror::Error)]
pub enum MediaDeleteError {
    #[error("failed to delete s3://{bucket}/{key}: {source}")]
    Object {
        bucket: String,
        key: String,
        source: ObjectStoreError,
    },
    #[error("failed to delete the media row: {0}")]
    Row(#[from] RepoError),
}

// `async_trait` marks its boxed-future return `#[must_use]`, which clippy's
// `double_must_use` flags on every method. Macro-generated, not ours to fix.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait MediaObjectStore: Send + Sync {
    /// Delete one object. A missing object is success.
    async fn delete_object(&self, bucket: &str, key: &str) -> Result<(), ObjectStoreError>;
}

/// Everything needed to delete one kind of media: the shared client plus
/// the originals/processed bucket pair it lives in (images and avatars use
/// different originals buckets but the same processed bucket —
/// `docs/design/media.md` §4.1).
#[derive(Clone)]
pub struct MediaDeleter {
    pub store: Arc<dyn MediaObjectStore>,
    pub originals_bucket: String,
    pub processed_bucket: String,
}

/// A Lambda environment variable the deleter needs is missing.
#[derive(Debug, thiserror::Error)]
#[error("{0} is not set")]
pub struct MissingEnvVar(pub String);

impl MediaDeleter {
    /// The real S3 client, for the bucket pair named by the two env vars.
    pub fn from_env(
        aws_config: &aws_config::SdkConfig,
        originals_bucket_var: &str,
        processed_bucket_var: &str,
    ) -> Result<Self, MissingEnvVar> {
        Ok(Self {
            store: Arc::new(S3ObjectStore::new(aws_sdk_s3::Client::new(aws_config))),
            originals_bucket: env_var(originals_bucket_var)?,
            processed_bucket: env_var(processed_bucket_var)?,
        })
    }

    /// The same client for another bucket pair (`lambda-media` deletes both
    /// images and avatars).
    pub fn with_buckets_from_env(
        &self,
        originals_bucket_var: &str,
        processed_bucket_var: &str,
    ) -> Result<Self, MissingEnvVar> {
        Ok(Self {
            store: Arc::clone(&self.store),
            originals_bucket: env_var(originals_bucket_var)?,
            processed_bucket: env_var(processed_bucket_var)?,
        })
    }
}

fn env_var(name: &str) -> Result<String, MissingEnvVar> {
    std::env::var(name).map_err(|_| MissingEnvVar(name.to_owned()))
}

/// Delete an image's original, display and thumb objects, then delete its
/// `ImageMedia` row.
///
/// `image` is the caller's read of the row. If processing finishes before the
/// row delete, it has written processed objects that read didn't list, so the
/// row delete fails its status condition and this rereads the row and goes
/// again. A row leaves `pending` only once, so that happens at most once.
pub async fn delete_image_media(
    repo: &Repo,
    deleter: &MediaDeleter,
    image: &ImageMedia,
) -> Result<(), MediaDeleteError> {
    let mut current = image.clone();
    for attempt in 1..=MAX_DELETE_ATTEMPTS {
        let processed = [current.display_key.as_deref(), current.thumb_key.as_deref()];
        delete_objects(deleter, &current.original_key, &processed).await?;
        match media::delete_image_row(
            repo,
            &current.group_id,
            &current.cycle_id,
            &current.image_id,
            current.status,
        )
        .await
        {
            Ok(()) => break,
            Err(RepoError::ConditionalCheckFailed) if attempt < MAX_DELETE_ATTEMPTS => {
                tracing::info!(image_id = %current.image_id, "image finished processing during its delete; deleting again");
                match media::get_image(
                    repo,
                    &current.group_id,
                    &current.cycle_id,
                    &current.image_id,
                )
                .await?
                {
                    Some(fresh) => current = fresh,
                    None => break,
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    tracing::info!(group_id = %image.group_id, cycle_id = %image.cycle_id, image_id = %image.image_id, "image deleted");
    Ok(())
}

/// Delete an avatar's original and display objects, then delete its
/// `AvatarMedia` row. Rereads and goes again on a status change, as
/// [`delete_image_media`] does.
pub async fn delete_avatar_media(
    repo: &Repo,
    deleter: &MediaDeleter,
    avatar: &AvatarMedia,
) -> Result<(), MediaDeleteError> {
    let mut current = avatar.clone();
    for attempt in 1..=MAX_DELETE_ATTEMPTS {
        let processed = [current.display_key.as_deref()];
        delete_objects(deleter, &current.original_key, &processed).await?;
        match media::delete_avatar_row(repo, &current.user_id, &current.avatar_id, current.status)
            .await
        {
            Ok(()) => break,
            Err(RepoError::ConditionalCheckFailed) if attempt < MAX_DELETE_ATTEMPTS => {
                tracing::info!(avatar_id = %current.avatar_id, "avatar finished processing during its delete; deleting again");
                match media::get_avatar(repo, &current.user_id, &current.avatar_id).await? {
                    Some(fresh) => current = fresh,
                    None => break,
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
    tracing::info!(user_id = %avatar.user_id, avatar_id = %avatar.avatar_id, "avatar deleted");
    Ok(())
}

/// One pass on the caller's read, plus one after the single `pending` →
/// `ready`/`failed` transition a row can make.
const MAX_DELETE_ATTEMPTS: u32 = 2;

/// The S3 half of a delete: the original, then each processed variant the
/// row has a key for. A row with no processed key (still `pending`/`failed`)
/// was never served, so there's nothing else to delete.
async fn delete_objects(
    deleter: &MediaDeleter,
    original_key: &str,
    processed_keys: &[Option<&str>],
) -> Result<(), MediaDeleteError> {
    delete_object(deleter, &deleter.originals_bucket, original_key).await?;
    for key in processed_keys.iter().flatten() {
        delete_object(deleter, &deleter.processed_bucket, key).await?;
    }
    Ok(())
}

async fn delete_object(
    deleter: &MediaDeleter,
    bucket: &str,
    key: &str,
) -> Result<(), MediaDeleteError> {
    deleter
        .store
        .delete_object(bucket, key)
        .await
        .map_err(|source| MediaDeleteError::Object {
            bucket: bucket.to_owned(),
            key: key.to_owned(),
            source,
        })
}
