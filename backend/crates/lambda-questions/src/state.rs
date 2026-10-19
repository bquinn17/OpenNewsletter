//! Process-wide state built once per cold start.

use media_urls::MediaUrls;
use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// Presigned S3 GET URLs for avatar fields (`docs/design/media.md` §4.3).
    pub media_urls: MediaUrls,
}
