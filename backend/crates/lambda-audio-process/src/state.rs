//! Process-wide state built once per cold start.

use crate::storage::ObjectStore;
use crate::transcode::AudioTranscoder;
use persistence::Repo;
use std::sync::Arc;

/// The two buckets this Lambda is wired to: the shared media-originals
/// bucket (under the `audio-uploads/` prefix) and the shared processed
/// bucket (under `aud/*`) — `docs/design/media.md` §12.
pub struct Buckets {
    pub media_originals: String,
    pub processed: String,
}

pub struct AppState {
    pub repo: Repo,
    pub store: Arc<dyn ObjectStore>,
    pub buckets: Buckets,
    pub transcoder: Arc<dyn AudioTranscoder>,
}
