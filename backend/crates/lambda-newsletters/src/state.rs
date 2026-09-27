//! Process-wide state built once per cold start.

use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// CloudFront origin, e.g. `https://cdn.example.com`. Empty (and both URL
    /// helpers return `None`) until infra wires `CDN_BASE_URL` for this Lambda
    /// — mirrors `lambda-groups::state::AppState`.
    pub cdn_base_url: String,
}

impl AppState {
    pub fn avatar_url(&self, avatar_id: &domain::AvatarId) -> Option<String> {
        if self.cdn_base_url.is_empty() {
            return None;
        }
        Some(format!(
            "{}/avatar/{}/display.webp",
            self.cdn_base_url.trim_end_matches('/'),
            avatar_id
        ))
    }

    /// Absolute CloudFront URL for a processed image's S3 key
    /// (`plans/03-api-contract.md` §5.2 — "Image URLs are absolute CloudFront URLs").
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
}
