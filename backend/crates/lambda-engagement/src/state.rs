//! Process-wide state built once per cold start. Reads the same env vars as
//! `lambda-newsletters::state::AppState` so the inline published-newsletter
//! view and the dedicated engagement routes hydrate image/avatar URLs
//! identically.

use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// CloudFront origin, e.g. `https://cdn.example.com`. Empty (and
    /// [`AppState::image_url`] returns `None`) until infra wires
    /// `CDN_BASE_URL` for this Lambda.
    pub cdn_base_url: String,
}

impl AppState {
    /// Absolute CloudFront URL for a processed image's S3 key — mirrors
    /// `lambda-newsletters::state::AppState::image_url`.
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
