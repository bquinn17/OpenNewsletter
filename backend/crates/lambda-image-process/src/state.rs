//! Process-wide state built once per cold start.

use crate::storage::ObjectStore;
use persistence::Repo;
use std::sync::Arc;

/// The three buckets this Lambda is wired to
/// (`docs/design/infrastructure.md` §5.3): two originals buckets (so a
/// response/comment image's original can be told apart from an avatar's —
/// `docs/design/media.md` §11.2, see the README for the contradiction this
/// surfaced) and the one processed bucket both write their variants into
/// (they need the same lifecycle and CORS rules, so one bucket serves both).
pub struct Buckets {
    pub media_originals: String,
    pub avatars_originals: String,
    pub processed: String,
}

pub struct AppState {
    pub repo: Repo,
    pub store: Arc<dyn ObjectStore>,
    pub buckets: Buckets,
}
