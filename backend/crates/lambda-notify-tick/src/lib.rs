//! Library surface for `lambda-notify-tick`'s binary. `main.rs` stays a thin
//! binary shim over this crate. Reuses `push::state::AppState` directly (it
//! already carries everything a deadline-reminder fan-out needs: the repo,
//! the VAPID secret, and the push sender) rather than defining a second,
//! near-identical state type.

pub mod dispatch;
pub mod tick;
