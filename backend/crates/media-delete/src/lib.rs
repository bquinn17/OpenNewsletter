//! Real deletion of an uploaded image or avatar (`plans/08-media-uploads.md`
//! §8.0, §11.4; `03-api-contract.md` §9.4, §9.7): the S3 objects first, then
//! a CloudFront invalidation, then the DynamoDB row. Any S3 or invalidation
//! failure returns an error and leaves the row, so a retry redoes the steps,
//! all of which are idempotent.
//!
//! Used by `lambda-media` (`DELETE /uploads`, `DELETE /avatars`),
//! `lambda-engagement` (a comment releasing its image) and `lambda-groups`
//! (`PATCH /me` replacing an avatar).

mod clients;
pub mod fakes;

pub use clients::{CloudFrontInvalidator, S3ObjectStore};

use async_trait::async_trait;
use domain::{AvatarMedia, ImageMedia};
use persistence::{media, Repo, RepoError};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("object store error: {0}")]
pub struct ObjectStoreError(pub String);

#[derive(Debug, thiserror::Error)]
#[error("cdn invalidation error: {0}")]
pub struct InvalidationError(pub String);

#[derive(Debug, thiserror::Error)]
pub enum MediaDeleteError {
    #[error("failed to delete s3://{bucket}/{key}: {source}")]
    Object {
        bucket: String,
        key: String,
        source: ObjectStoreError,
    },
    #[error("failed to invalidate {path}: {source}")]
    Invalidation {
        path: String,
        source: InvalidationError,
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

#[allow(clippy::double_must_use)]
#[async_trait]
pub trait CdnInvalidator: Send + Sync {
    /// Invalidate `paths` (each starting with `/`, optionally ending in `*`).
    async fn invalidate(&self, paths: &[String]) -> Result<(), InvalidationError>;
}

/// Everything needed to delete one kind of media: the shared clients plus
/// the originals/processed bucket pair it lives in (images and avatars use
/// different buckets).
#[derive(Clone)]
pub struct MediaDeleter {
    pub store: Arc<dyn MediaObjectStore>,
    pub cdn: Arc<dyn CdnInvalidator>,
    pub originals_bucket: String,
    pub processed_bucket: String,
}

/// Delete an image's original, display and thumb objects, invalidate the
/// display and thumb paths, then delete its `ImageMedia` row.
pub async fn delete_image_media(
    repo: &Repo,
    deleter: &MediaDeleter,
    image: &ImageMedia,
) -> Result<(), MediaDeleteError> {
    let processed = [image.display_key.as_deref(), image.thumb_key.as_deref()];
    delete_objects(deleter, &image.original_key, &processed).await?;
    media::delete_image_row(repo, &image.group_id, &image.cycle_id, &image.image_id).await?;
    tracing::info!(group_id = %image.group_id, cycle_id = %image.cycle_id, image_id = %image.image_id, "image deleted");
    Ok(())
}

/// Delete an avatar's original and display objects, invalidate the display
/// path, then delete its `AvatarMedia` row.
pub async fn delete_avatar_media(
    repo: &Repo,
    deleter: &MediaDeleter,
    avatar: &AvatarMedia,
) -> Result<(), MediaDeleteError> {
    let processed = [avatar.display_key.as_deref()];
    delete_objects(deleter, &avatar.original_key, &processed).await?;
    media::delete_avatar_row(repo, &avatar.user_id, &avatar.avatar_id).await?;
    tracing::info!(user_id = %avatar.user_id, avatar_id = %avatar.avatar_id, "avatar deleted");
    Ok(())
}

/// The S3 and CDN half of a delete. Each processed key is invalidated by its
/// exact path. A wildcard would also work, but CloudFront allows only 15
/// wildcard invalidations in progress per distribution, against 3,000 exact
/// paths, and a burst of deletes would hit that cap. A row with no processed
/// key (still pending, or failed) was never served, so there is nothing to
/// invalidate.
async fn delete_objects(
    deleter: &MediaDeleter,
    original_key: &str,
    processed_keys: &[Option<&str>],
) -> Result<(), MediaDeleteError> {
    delete_object(deleter, &deleter.originals_bucket, original_key).await?;
    for key in processed_keys.iter().flatten() {
        delete_object(deleter, &deleter.processed_bucket, key).await?;
    }
    let paths: Vec<String> = processed_keys
        .iter()
        .flatten()
        .map(|key| invalidation_path(key))
        .collect();
    if !paths.is_empty() {
        deleter
            .cdn
            .invalidate(&paths)
            .await
            .map_err(|source| MediaDeleteError::Invalidation {
                path: paths.join(", "),
                source,
            })?;
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

/// `img/g/c/q/u/i/display.webp` → `/img/g/c/q/u/i/display.webp`. Processed
/// keys are laid out to match the CDN paths they're served at
/// (`08-media-uploads.md` §4).
fn invalidation_path(processed_key: &str) -> String {
    format!("/{}", processed_key.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_key_invalidates_its_exact_path() {
        assert_eq!(
            invalidation_path("img/g1/202606/q1/u1/i1/display.webp"),
            "/img/g1/202606/q1/u1/i1/display.webp"
        );
    }

    #[test]
    fn an_avatar_key_invalidates_its_exact_path() {
        assert_eq!(
            invalidation_path("avatar/av1/display.webp"),
            "/avatar/av1/display.webp"
        );
    }
}
