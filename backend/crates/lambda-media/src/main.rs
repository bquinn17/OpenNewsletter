//! `lambda-media` — upload, avatar, and CloudFront signed-cookie routes
//! (`docs/design/api.md` §9).

use lambda_http::{service_fn, Error};
use media::{router, state::AppState};
use media_delete::MediaDeleter;
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
        // `sha256` (`docs/design/media.md` §3.2).
        .request_checksum_calculation(aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired)
        .build();

    let s3 = aws_sdk_s3::Client::from_conf(s3_config);
    let image_deleter =
        MediaDeleter::from_env(&aws_config, "MEDIA_ORIGINALS_BUCKET", "PROCESSED_BUCKET")?;
    let avatar_deleter = image_deleter
        .with_buckets_from_env("AVATARS_ORIGINALS_BUCKET", "AVATARS_PROCESSED_BUCKET")?;
    let media_originals_bucket = image_deleter.originals_bucket.clone();
    let avatars_originals_bucket = avatar_deleter.originals_bucket.clone();

    let state = Arc::new(AppState::new(
        Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        s3,
        aws_sdk_secretsmanager::Client::new(&aws_config),
        media_originals_bucket,
        avatars_originals_bucket,
        std::env::var("CDN_BASE_URL").unwrap_or_default(),
        std::env::var("CDN_KEY_PAIR_ID").unwrap_or_default(),
        std::env::var("APP_SECRET_ARN").unwrap_or_default(),
        std::env::var("MEDIA_COOKIE_DOMAIN").unwrap_or_default(),
        image_deleter,
        avatar_deleter,
    ));

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
