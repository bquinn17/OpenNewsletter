//! Invite routes and the Cognito `PreSignUp` trigger. Both binaries in this crate
//! share the handler and persistence code here (`docs/design/auth.md` §3).

pub mod code;
pub mod handlers;
pub mod presignup;
pub mod router;
pub mod state;
