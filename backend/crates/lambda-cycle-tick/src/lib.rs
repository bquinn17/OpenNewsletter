//! Library surface for `lambda-cycle-tick`'s binary, and for other crates'
//! integration tests that need to run the tick synchronously (e.g.
//! `lambda-questions/tests/lifecycle.rs`) without spinning up the Lambda
//! runtime. `main.rs` stays a thin binary shim over this crate.

pub mod dispatch;
pub mod duration;
pub mod notify;
pub mod state;
pub mod tick;
