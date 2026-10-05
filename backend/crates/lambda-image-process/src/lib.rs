//! `lambda-image-process` — the S3 trigger that turns an uploaded original
//! into its display/thumb (or 256x256 avatar) variants
//! (`plans/08-media-uploads.md` §5, §11). `main.rs` stays a thin binary shim
//! over this crate so integration tests can drive `handler::handle_event`
//! directly against DynamoDB Local and the in-memory `storage::fake::FakeStore`.

pub mod error;
pub mod handler;
pub mod key_parse;
pub mod process;
pub mod state;
pub mod storage;
