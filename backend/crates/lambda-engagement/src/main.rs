//! `lambda-engagement` — comment and reaction routes
//! (`plans/03-api-contract.md` §8).

use engagement::{router, state::AppState};
use lambda_http::{service_fn, Error};
use media_delete::{CloudFrontInvalidator, MediaDeleter, S3ObjectStore};
use persistence::Repo;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let image_deleter = MediaDeleter {
        store: Arc::new(S3ObjectStore::new(aws_sdk_s3::Client::new(&aws_config))),
        cdn: Arc::new(CloudFrontInvalidator::new(
            aws_sdk_cloudfront::Client::new(&aws_config),
            std::env::var("CDN_DISTRIBUTION_ID").map_err(|_| "CDN_DISTRIBUTION_ID is not set")?,
        )),
        originals_bucket: std::env::var("MEDIA_ORIGINALS_BUCKET")
            .map_err(|_| "MEDIA_ORIGINALS_BUCKET is not set")?,
        processed_bucket: std::env::var("PROCESSED_BUCKET")
            .map_err(|_| "PROCESSED_BUCKET is not set")?,
    };

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        cdn_base_url: std::env::var("CDN_BASE_URL").unwrap_or_default(),
        image_deleter,
    });

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
