//! `lambda-media` — upload, avatar, and CloudFront signed-cookie routes
//! (`plans/03-api-contract.md` §9).

use lambda_http::{service_fn, Error};
use media::{router, state::AppState};
use persistence::Repo;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let s3_config = aws_sdk_s3::config::Builder::from(&aws_config)
        // Newer aws-sdk-s3 defaults to signing a CRC32 checksum of the (empty,
        // for a presigned PUT) body, which breaks a browser's direct upload —
        // only sign a checksum when the caller explicitly asked for one via
        // `sha256` (`plans/08-media-uploads.md` §3.2).
        .request_checksum_calculation(aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired)
        .build();

    let state = Arc::new(AppState::new(
        Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        aws_sdk_s3::Client::from_conf(s3_config),
        aws_sdk_secretsmanager::Client::new(&aws_config),
        std::env::var("MEDIA_ORIGINALS_BUCKET").map_err(|_| "MEDIA_ORIGINALS_BUCKET is not set")?,
        std::env::var("AVATARS_ORIGINALS_BUCKET")
            .map_err(|_| "AVATARS_ORIGINALS_BUCKET is not set")?,
        std::env::var("CDN_BASE_URL").unwrap_or_default(),
        std::env::var("CDN_KEY_PAIR_ID").unwrap_or_default(),
        std::env::var("CDN_SIGNING_SECRET_ARN").unwrap_or_default(),
        std::env::var("MEDIA_COOKIE_DOMAIN").unwrap_or_default(),
    ));

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
