//! Process-wide state built once per cold start.

use crate::cloudfront;
use domain::{ApiError, AvatarId};
use persistence::Repo;
use rsa::RsaPrivateKey;
use tokio::sync::OnceCell;

pub struct AppState {
    pub repo: Repo,
    pub s3: aws_sdk_s3::Client,
    pub secrets: aws_sdk_secretsmanager::Client,
    pub media_originals_bucket: String,
    pub avatars_originals_bucket: String,
    /// CloudFront origin, e.g. `https://cdn.example.com`. Empty (and both URL
    /// helpers return `None`) until infra wires `CDN_BASE_URL` for this
    /// Lambda — mirrors `lambda-newsletters::state::AppState`.
    pub cdn_base_url: String,
    pub cdn_key_pair_id: String,
    /// Secrets Manager ARN of the secret whose `SecretString` is the raw RSA
    /// private-key PEM used to sign CloudFront policies.
    pub cdn_signing_secret_arn: String,
    /// `Set-Cookie` `Domain` for `/media-cookie` (`08-media-uploads.md` §4.2).
    /// Empty in dev, where there's no registrable domain shared with the CDN
    /// — `GET /media-cookie` then emits no `Set-Cookie` headers at all, and
    /// the SPA falls back to signed query params (§4.5).
    pub media_cookie_domain: String,
    signing_key: OnceCell<RsaPrivateKey>,
}

impl AppState {
    // One field per env var `main.rs` reads; a builder would be more ceremony
    // than the one call site justifies.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: Repo,
        s3: aws_sdk_s3::Client,
        secrets: aws_sdk_secretsmanager::Client,
        media_originals_bucket: String,
        avatars_originals_bucket: String,
        cdn_base_url: String,
        cdn_key_pair_id: String,
        cdn_signing_secret_arn: String,
        media_cookie_domain: String,
    ) -> Self {
        Self {
            repo,
            s3,
            secrets,
            media_originals_bucket,
            avatars_originals_bucket,
            cdn_base_url,
            cdn_key_pair_id,
            cdn_signing_secret_arn,
            media_cookie_domain,
            signing_key: OnceCell::new(),
        }
    }

    /// Absolute CloudFront URL for a processed image's S3 key
    /// (`plans/03-api-contract.md` §5.2).
    pub fn image_url(&self, key: &str) -> Option<String> {
        if self.cdn_base_url.is_empty() {
            return None;
        }
        Some(format!(
            "{}/{}",
            self.cdn_base_url.trim_end_matches('/'),
            key.trim_start_matches('/')
        ))
    }

    pub fn avatar_url(&self, avatar_id: &AvatarId) -> Option<String> {
        if self.cdn_base_url.is_empty() {
            return None;
        }
        Some(format!(
            "{}/avatar/{}/display.webp",
            self.cdn_base_url.trim_end_matches('/'),
            avatar_id
        ))
    }

    /// The RSA private key used to sign CloudFront policies, fetched from
    /// Secrets Manager on first use and cached for the life of the warm
    /// Lambda container (`plans/08-media-uploads.md` §4.3).
    pub async fn signing_key(&self) -> Result<&RsaPrivateKey, ApiError> {
        self.signing_key
            .get_or_try_init(|| async {
                let resp = self
                    .secrets
                    .get_secret_value()
                    .secret_id(&self.cdn_signing_secret_arn)
                    .send()
                    .await
                    .map_err(|e| {
                        tracing::error!(error = ?e, "failed to fetch cloudfront signing key");
                        ApiError::internal("failed to load cloudfront signing key")
                    })?;
                let pem = resp.secret_string().ok_or_else(|| {
                    tracing::error!("cloudfront signing secret has no SecretString");
                    ApiError::internal("cloudfront signing secret has no SecretString")
                })?;
                cloudfront::parse_private_key(pem).map_err(|e| {
                    tracing::error!(error = ?e, "failed to parse cloudfront signing key");
                    ApiError::internal("failed to parse cloudfront signing key")
                })
            })
            .await
    }

    /// Escape hatch for tests: inject a key directly so they never need a
    /// Secrets Manager call (`plans/08-media-uploads.md` §4.3 — "Design
    /// `AppState` so tests can inject an in-memory RSA key"). A no-op if the
    /// key has already been loaded.
    pub fn set_signing_key_for_test(&self, key: RsaPrivateKey) {
        let _ = self.signing_key.set(key);
    }
}
