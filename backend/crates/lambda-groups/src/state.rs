//! Process-wide state built once per cold start.

use media_delete::MediaDeleter;
use media_urls::MediaUrls;
use persistence::Repo;
use shared::cycle_time::CycleCadence;

pub struct AppState {
    pub repo: Repo,
    /// Group-default settings from `shared::config::default_group_settings`
    /// (`docs/design/infrastructure.md` §6.4). Echoed verbatim by `GET /config`.
    /// A field (rather than calling the function inline) so tests can stub it.
    pub group_defaults: serde_json::Value,
    pub vapid_public_key: String,
    /// Presigned S3 GET URLs for avatar fields (`docs/design/media.md` §4.3).
    pub media_urls: MediaUrls,
    /// Deletes the avatar a `PATCH /me` replaces (`docs/design/media.md` §11.4).
    pub avatar_deleter: MediaDeleter,
    /// This deployment's cycle cadence (`docs/design/lifecycle.md` §13),
    /// read once at cold start from `CYCLE_FREQUENCY`/`CYCLE_WEEKLY_START`.
    /// Echoed by `GET /config` and used to cap `responseWindowDays` and to
    /// reschedule the current `voting` cycle (§6).
    pub cadence: CycleCadence,
}
