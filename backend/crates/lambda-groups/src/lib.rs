//! `lambda-groups` — `/config`, `/healthz`, `/me`, and the group + member routes
//! (`plans/03-api-contract.md` §2, §3.5, §3.6, §4). Split into a library so
//! integration tests can call the handlers directly (mirrors `lambda-questions`
//! / `lambda-responses`).

pub mod group_routes;
pub mod me;
pub mod router;
pub mod state;
pub mod validation;
