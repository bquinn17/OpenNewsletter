//! Binary shim — wires AWS clients, the `ffmpeg` transcoder and env-configured
//! bucket names into `audio_process::state::AppState`, then hands every
//! invocation to `audio_process::handler::handle_event`.

use audio_process::handler;
use audio_process::s3_event::S3Event;
use audio_process::state::{AppState, Buckets};
use audio_process::storage::S3Store;
use audio_process::transcode::FfmpegTranscoder;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use persistence::Repo;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;
    let media_originals =
        std::env::var("MEDIA_ORIGINALS_BUCKET").map_err(|_| "MEDIA_ORIGINALS_BUCKET is not set")?;
    let processed = std::env::var("PROCESSED_BUCKET").map_err(|_| "PROCESSED_BUCKET is not set")?;
    let ffmpeg_path = std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "/opt/bin/ffmpeg".to_owned());

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        store: Arc::new(S3Store::new(aws_sdk_s3::Client::new(&aws_config))),
        buckets: Buckets {
            media_originals,
            processed,
        },
        transcoder: Arc::new(FfmpegTranscoder::new(ffmpeg_path)),
    });

    run(service_fn(move |event: LambdaEvent<S3Event>| {
        let state = Arc::clone(&state);
        async move {
            handler::handle_event(&state, event.payload).await?;
            Ok::<(), Error>(())
        }
    }))
    .await
}
