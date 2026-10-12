//! `lambda-media` — upload, avatar, and CloudFront signed-cookie routes
//! (`plans/03-api-contract.md` §9).

use lambda_http::{service_fn, Error};
use media::{router, state::AppState};
use media_delete::{
    CdnInvalidator, CloudFrontInvalidator, MediaDeleter, MediaObjectStore, S3ObjectStore,
};
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

    let s3 = aws_sdk_s3::Client::from_conf(s3_config);
    let media_originals_bucket =
        std::env::var("MEDIA_ORIGINALS_BUCKET").map_err(|_| "MEDIA_ORIGINALS_BUCKET is not set")?;
    let avatars_originals_bucket = std::env::var("AVATARS_ORIGINALS_BUCKET")
        .map_err(|_| "AVATARS_ORIGINALS_BUCKET is not set")?;
    let store: Arc<dyn MediaObjectStore> = Arc::new(S3ObjectStore::new(s3.clone()));
    let cdn: Arc<dyn CdnInvalidator> = Arc::new(CloudFrontInvalidator::new(
        aws_sdk_cloudfront::Client::new(&aws_config),
        std::env::var("CDN_DISTRIBUTION_ID").map_err(|_| "CDN_DISTRIBUTION_ID is not set")?,
    ));
    let image_deleter = MediaDeleter {
        store: store.clone(),
        cdn: cdn.clone(),
        originals_bucket: media_originals_bucket.clone(),
        processed_bucket: std::env::var("PROCESSED_BUCKET")
            .map_err(|_| "PROCESSED_BUCKET is not set")?,
    };
    let avatar_deleter = MediaDeleter {
        store,
        cdn,
        originals_bucket: avatars_originals_bucket.clone(),
        processed_bucket: std::env::var("AVATARS_PROCESSED_BUCKET")
            .map_err(|_| "AVATARS_PROCESSED_BUCKET is not set")?,
    };

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
