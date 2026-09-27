//! Response (answer) draft/publish routes (`plans/03-api-contract.md` §7). Split
//! into a library so integration tests can call the handlers directly.

pub mod handlers;
pub mod router;
pub mod state;
pub mod validation;
