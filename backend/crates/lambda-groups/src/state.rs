//! Process-wide state built once per cold start.

use media_delete::MediaDeleter;
use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// Group-default settings from `shared::config::default_group_settings`
    /// (`plans/01-infrastructure-cdk.md` §6.4). Echoed verbatim by `GET /config`.
    /// A field (rather than calling the function inline) so tests can stub it.
    pub group_defaults: serde_json::Value,
    pub vapid_public_key: String,
    /// CloudFront origin for avatar URLs, e.g. `https://cdn.example.com`.
    pub cdn_base_url: String,
    /// Deletes the avatar a `PATCH /me` replaces (`plans/08-media-uploads.md` §11.4).
    pub avatar_deleter: MediaDeleter,
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
