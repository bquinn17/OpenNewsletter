//! Process-wide state built once per cold start.

use crate::storage::ObjectStore;
use persistence::Repo;
use std::sync::Arc;

/// The four buckets this Lambda is wired to
/// (`plans/01-infrastructure-cdk.md` §5.3). `media_originals`/
/// `avatars_originals` are read from the env even though CDK's §5.3 listing
/// doesn't name them, because branching on the triggering bucket's name is
/// how a single Lambda tells a response/comment image apart from an avatar
/// (`plans/08-media-uploads.md` §11.2) — see the README for the contradiction
/// this surfaced.
pub struct Buckets {
    pub media_originals: String,
    pub avatars_originals: String,
    pub processed: String,
    pub avatars_processed: String,
}

pub struct AppState {
    pub repo: Repo,
    pub store: Arc<dyn ObjectStore>,
    pub buckets: Buckets,
}
