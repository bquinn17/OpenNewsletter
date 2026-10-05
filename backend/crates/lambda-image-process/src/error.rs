//! Errors that should make the Lambda invocation fail so the S3-direct
//! retry kicks in (`plans/08-media-uploads.md` §5` error policy — a
//! transient AWS failure, not a content problem). Content problems
//! (oversized, undecodable, malformed key, missing row) are terminal and
//! recorded on the row inline in `handler.rs`; they never become a
//! `TransientError`.

#[derive(Debug, thiserror::Error)]
pub enum TransientError {
    #[error("object store error: {0}")]
    Store(#[from] crate::storage::StoreError),
    #[error("repository error: {0}")]
    Repo(#[from] persistence::RepoError),
    #[error("image processing task panicked: {0}")]
    Join(String),
}
