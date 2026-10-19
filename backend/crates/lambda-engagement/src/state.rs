//! Process-wide state built once per cold start. Reads the same env vars as
//! `lambda-newsletters::state::AppState` so the inline published-newsletter
//! view and the dedicated engagement routes hydrate image/avatar URLs
//! identically.

use media_delete::MediaDeleter;
use media_urls::MediaUrls;
use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// Presigned S3 GET URLs for image/avatar fields (`docs/design/media.md` §4.3).
    pub media_urls: MediaUrls,
    /// Deletes an image a comment releases (`docs/design/engagement.md` §1.1).
    pub image_deleter: MediaDeleter,
}
