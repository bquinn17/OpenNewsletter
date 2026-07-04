//! DynamoDB single-table adapter for OpenNewsletter.
//!
//! Module map:
//! - [`keys`] — pure key-builder functions. Single source of truth for `pk`/`sk` shapes.
//! - [`error`] — [`RepoError`] enum.
//! - one module per entity family (`users`, `groups`, `invites`, `newsletters`,
//!   `questions`, `responses`, `engagement`, `media`, `push`).

pub mod error;
pub mod keys;
pub mod repo;

pub mod engagement;
pub mod groups;
pub mod invites;
pub mod media;
pub mod newsletters;
pub mod push;
pub mod questions;
pub mod responses;
pub mod users;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_factories;

pub use error::RepoError;
pub use repo::Repo;
