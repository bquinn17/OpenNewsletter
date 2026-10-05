//! Binary shim — wires AWS clients and env-configured bucket names into
//! `image_process::state::AppState`, then hands every invocation to
//! `image_process::handler::handle_event`.

use aws_lambda_events::event::s3::S3Event;
use image_process::handler;
use image_process::state::{AppState, Buckets};
use image_process::storage::S3Store;
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
    let avatars_originals = std::env::var("AVATARS_ORIGINALS_BUCKET")
        .map_err(|_| "AVATARS_ORIGINALS_BUCKET is not set")?;
    let processed = std::env::var("PROCESSED_BUCKET").map_err(|_| "PROCESSED_BUCKET is not set")?;
    let avatars_processed = std::env::var("AVATARS_PROCESSED_BUCKET")
        .map_err(|_| "AVATARS_PROCESSED_BUCKET is not set")?;

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        store: Arc::new(S3Store::new(aws_sdk_s3::Client::new(&aws_config))),
        buckets: Buckets {
            media_originals,
            avatars_originals,
            processed,
            avatars_processed,
        },
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
