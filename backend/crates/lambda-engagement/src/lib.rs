//! Comment and reaction routes (`docs/design/api.md` §8). Split into a
//! library so integration tests can call the handlers directly.

pub mod handlers;
pub mod router;
pub mod state;
pub mod validation;
