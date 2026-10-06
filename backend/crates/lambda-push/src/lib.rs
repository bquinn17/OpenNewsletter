//! Push subscription routes (`plans/03-api-contract.md` §10) and the
//! cycle-open / publication notification fan-outs
//! (`plans/07-notifications.md` §6, §8). Split into a library so
//! integration tests and `lambda-notify-tick` can call into it directly.

pub mod delivery;
pub mod dispatch;
pub mod fanout;
pub mod handlers;
pub mod metrics;
pub mod payload;
pub mod router;
pub mod sender;
pub mod state;
pub mod validation;
pub mod webpush;
