//! Presigned S3 GET URLs for every image/avatar field the API serves
//! (`docs/design/media.md` §4.3). There is no CDN (§4): every response that
//! carries an image hands out a short-lived presigned GET URL for the one
//! processed bucket instead.
//!
//! A library crate, not a Lambda — `lambda-media`, `lambda-newsletters`,
//! `lambda-engagement`, `lambda-groups` and `lambda-questions` each hold one
//! [`MediaUrls`] in their `AppState` rather than five copies of the same
//! `format!`.

use aws_sdk_s3::config::{RequestChecksumCalculation, ResponseChecksumValidation};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use domain::{ApiError, AvatarId, ImageMedia, MediaStatus};
use shared::config::{MEDIA_URL_SIGN_WINDOW_SECONDS, MEDIA_URL_TTL_SECONDS};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A Lambda environment variable `MediaUrls` needs is missing.
#[derive(Debug, thiserror::Error)]
#[error("{0} is not set")]
pub struct MissingEnvVar(pub String);

/// A `ready` image's presigned variant URLs, plus the original's dimensions.
#[derive(Debug)]
pub struct ReadyImageUrls {
    pub display_url: String,
    pub thumb_url: String,
    pub width: u32,
    pub height: u32,
}

/// Issues presigned S3 GET URLs for the one processed bucket
/// (`docs/design/media.md` §4.1).
#[derive(Clone)]
pub struct MediaUrls {
    s3: S3Client,
    bucket: String,
}

impl MediaUrls {
    pub fn new(s3: S3Client, bucket: impl Into<String>) -> Self {
        Self {
            s3,
            bucket: bucket.into(),
        }
    }

    /// Reads `PROCESSED_BUCKET` and builds its own S3 client. The checksum
    /// settings below matter for presigned GETs specifically
    /// (`docs/design/media.md` §4.2): a browser `<img>` request sends only
    /// the `Host` header, so the presigned URL must sign nothing else. A
    /// client that defaults to adding `x-amz-checksum-mode` would make every
    /// such GET a 403.
    pub fn from_env(aws_config: &aws_config::SdkConfig) -> Result<Self, MissingEnvVar> {
        let bucket = env_var("PROCESSED_BUCKET")?;
        let s3_config = aws_sdk_s3::config::Builder::from(aws_config)
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .build();
        Ok(Self::new(S3Client::from_conf(s3_config), bucket))
    }

    /// Presigned GET for a processed image's S3 key.
    pub async fn image_url(&self, key: &str) -> Result<String, ApiError> {
        self.presign_get_at(key, SystemTime::now()).await
    }

    /// Presigned GET for an avatar's display variant
    /// (`docs/design/media.md` §11.1 — key layout unchanged by the move off
    /// CloudFront, only the bucket it lives in).
    pub async fn avatar_url(&self, avatar_id: &AvatarId) -> Result<String, ApiError> {
        self.presign_get_at(&avatar_key(avatar_id), SystemTime::now())
            .await
    }

    /// [`Self::avatar_url`] for a user who may have no avatar.
    pub async fn optional_avatar_url(
        &self,
        avatar_id: Option<&AvatarId>,
    ) -> Result<Option<String>, ApiError> {
        match avatar_id {
            Some(id) => Ok(Some(self.avatar_url(id).await?)),
            None => Ok(None),
        }
    }

    /// The display/thumb URLs for an image, or `None` until it's `ready` with
    /// both variants and its dimensions recorded.
    pub async fn ready_image_urls(
        &self,
        image: &ImageMedia,
    ) -> Result<Option<ReadyImageUrls>, ApiError> {
        if image.status != MediaStatus::Ready {
            return Ok(None);
        }
        let (Some(display_key), Some(thumb_key), Some(width), Some(height)) = (
            &image.display_key,
            &image.thumb_key,
            image.width,
            image.height,
        ) else {
            return Ok(None);
        };
        Ok(Some(ReadyImageUrls {
            display_url: self.image_url(display_key).await?,
            thumb_url: self.image_url(thumb_key).await?,
            width,
            height,
        }))
    }

    /// Presigns a GET for `key`, with its start time floored to
    /// [`MEDIA_URL_SIGN_WINDOW_SECONDS`] so repeat requests inside the same
    /// window — from one warm Lambda container — produce byte-identical
    /// URLs, keeping `<img src>` stable across refetches
    /// (`docs/design/media.md` §4.2). Expiry is the floored start plus
    /// [`MEDIA_URL_TTL_SECONDS`].
    ///
    /// Takes `now` explicitly so tests can pin it; callers otherwise always
    /// pass [`SystemTime::now`].
    ///
    /// The Lambda execution role's credentials can themselves expire before
    /// the URL's TTL does (IAM role credentials are typically valid for
    /// 1-12 hours); the frontend recovers from a 403 by refetching the
    /// page's data, which mints a fresh URL from a (by then) fresh credential
    /// (`docs/design/media.md` §4.4).
    pub(crate) async fn presign_get_at(
        &self,
        key: &str,
        now: SystemTime,
    ) -> Result<String, ApiError> {
        let presigning_config = PresigningConfig::builder()
            .start_time(window_start(now))
            .expires_in(Duration::from_secs(MEDIA_URL_TTL_SECONDS))
            .build()
            .map_err(|e| {
                tracing::error!(error = ?e, "failed to build presigning config");
                ApiError::internal("failed to build presign config")
            })?;

        let presigned = self
            .s3
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_config)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, bucket = %self.bucket, key, "failed to presign media url");
                ApiError::internal("failed to presign media url")
            })?;

        Ok(presigned.uri().to_owned())
    }
}

/// `avatar/{id}/display.webp` (`docs/design/media.md` §11.1).
fn avatar_key(avatar_id: &AvatarId) -> String {
    format!("avatar/{avatar_id}/display.webp")
}

/// Floors `now` to the start of its [`MEDIA_URL_SIGN_WINDOW_SECONDS`] window.
fn window_start(now: SystemTime) -> SystemTime {
    let window = MEDIA_URL_SIGN_WINDOW_SECONDS;
    let since_epoch = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    UNIX_EPOCH + Duration::from_secs(since_epoch - (since_epoch % window))
}

fn env_var(name: &str) -> Result<String, MissingEnvVar> {
    std::env::var(name).map_err(|_| MissingEnvVar(name.to_owned()))
}

/// Test-only construction, gated like `persistence::test_factories`
/// (`docs/development/coding-standards.md` §2.7): available to this crate's
/// own `#[cfg(test)]` unit tests and, via the `test-utils` feature, to the
/// Lambda crates' integration tests that build an `AppState` directly.
#[cfg(any(test, feature = "test-utils"))]
impl MediaUrls {
    /// A `MediaUrls` over static credentials and a fixed region — presigning
    /// is pure local signing, no network call, so no real AWS account is
    /// needed in tests.
    pub fn for_tests(bucket: impl Into<String>) -> Self {
        use aws_sdk_s3::config::{Credentials, Region};

        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .credentials_provider(Credentials::new("test", "test", None, None, "test"))
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            .build();
        Self::new(S3Client::from_conf(config), bucket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::AvatarId;

    const BUCKET: &str = "test-processed";

    fn media_urls() -> MediaUrls {
        MediaUrls::for_tests(BUCKET)
    }

    #[tokio::test]
    async fn image_url_points_at_the_bucket_and_key() {
        let urls = media_urls();
        let url = urls
            .image_url("img/g1/202606/q1/u1/i1/display.webp")
            .await
            .unwrap();
        assert!(
            url.starts_with(&format!(
                "https://{BUCKET}.s3.us-east-1.amazonaws.com/img/g1/202606/q1/u1/i1/display.webp?"
            )),
            "{url}"
        );
    }

    #[tokio::test]
    async fn the_url_expires_in_one_hour_and_carries_a_signature() {
        let urls = media_urls();
        let url = urls.image_url("img/k").await.unwrap();
        assert!(url.contains("X-Amz-Expires=3600"), "{url}");
        assert!(url.contains("X-Amz-Signature="), "{url}");
    }

    #[tokio::test]
    async fn only_the_host_header_is_signed_and_no_checksum_mode_is_added() {
        let urls = media_urls();
        let url = urls.image_url("img/k").await.unwrap();
        assert!(url.contains("X-Amz-SignedHeaders=host"), "{url}");
        assert!(
            !url.to_ascii_lowercase().contains("checksum-mode"),
            "a signed x-amz-checksum-mode would 403 every plain browser GET: {url}"
        );
    }

    #[tokio::test]
    async fn two_calls_in_the_same_sign_window_produce_identical_urls() {
        let urls = media_urls();
        let t0 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let t1 = t0 + Duration::from_secs(MEDIA_URL_SIGN_WINDOW_SECONDS - 1);

        let a = urls.presign_get_at("img/k", t0).await.unwrap();
        let b = urls.presign_get_at("img/k", t1).await.unwrap();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn calls_in_different_sign_windows_produce_different_urls() {
        let urls = media_urls();
        let t0 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let t1 = t0 + Duration::from_secs(MEDIA_URL_SIGN_WINDOW_SECONDS);

        let a = urls.presign_get_at("img/k", t0).await.unwrap();
        let b = urls.presign_get_at("img/k", t1).await.unwrap();
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn avatar_url_uses_the_display_webp_key_layout() {
        let urls = media_urls();
        let url = urls.avatar_url(&AvatarId::new("av-1")).await.unwrap();
        assert!(
            url.starts_with(&format!(
                "https://{BUCKET}.s3.us-east-1.amazonaws.com/avatar/av-1/display.webp?"
            )),
            "{url}"
        );
    }
}
