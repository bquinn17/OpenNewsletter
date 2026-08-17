//! Invite routes and the Cognito `PreSignUp` trigger. Both binaries in this crate
//! share the handler and persistence code here (`plans/05-auth-flow.md` §3).

pub mod code;
pub mod dto;
pub mod handlers;
pub mod presignup;
pub mod router;
pub mod state;
