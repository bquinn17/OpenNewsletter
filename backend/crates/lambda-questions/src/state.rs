//! Process-wide state built once per cold start.

use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// CloudFront origin for avatar URLs, e.g. `https://cdn.example.com`. Empty
    /// (and `avatar_url` returns `None`) until infra wires `CDN_BASE_URL` for
    /// this Lambda — mirrors `lambda-groups::state::AppState`.
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
}
