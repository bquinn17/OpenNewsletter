//! `lambda-audio-process` — the S3 trigger that turns an uploaded voice-clip
//! original into its processed Opus/WebM clip and waveform
//! (`docs/design/media.md` §12). `main.rs` stays a thin binary shim over this crate so integration
//! tests can drive `handler::handle_event` directly against DynamoDB Local
//! and the in-memory `storage::fake::FakeStore`.

pub mod error;
pub mod handler;
pub mod key_parse;
pub mod process;
pub mod s3_event;
pub mod state;
pub mod storage;
pub mod transcode;
