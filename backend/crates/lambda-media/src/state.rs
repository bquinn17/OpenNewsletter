//! Process-wide state built once per cold start.

use media_delete::MediaDeleter;
use media_urls::MediaUrls;
use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    pub s3: aws_sdk_s3::Client,
    pub media_originals_bucket: String,
    pub avatars_originals_bucket: String,
    /// Presigned S3 GET URLs for image/avatar fields (`docs/design/media.md` §4.3).
    pub media_urls: MediaUrls,
    /// Deletes response/comment images: media originals + processed buckets.
    pub image_deleter: MediaDeleter,
    /// Deletes avatars: avatar originals + processed buckets.
    pub avatar_deleter: MediaDeleter,
}

impl AppState {
    pub fn new(
        repo: Repo,
        s3: aws_sdk_s3::Client,
        media_originals_bucket: String,
        avatars_originals_bucket: String,
        media_urls: MediaUrls,
        image_deleter: MediaDeleter,
        avatar_deleter: MediaDeleter,
    ) -> Self {
        Self {
            repo,
            s3,
            media_originals_bucket,
            avatars_originals_bucket,
            media_urls,
            image_deleter,
            avatar_deleter,
        }
    }
}
