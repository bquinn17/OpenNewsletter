//! `tracing` setup for Lambda entry points.

use tracing_subscriber::EnvFilter;

/// Install the JSON subscriber. Call once, first thing in `main`.
///
/// CloudWatch already stamps each line with a timestamp, and the Lambda runtime
/// supplies the request ID, so both are omitted from the event payload.
pub fn init() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_env("RUST_LOG"))
        .with_target(false)
        .without_time()
        .with_current_span(false)
        .init();
}
