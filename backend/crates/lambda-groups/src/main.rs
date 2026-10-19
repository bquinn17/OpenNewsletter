//! `lambda-groups` — `/config`, `/healthz`, `/me`, and the group + member routes
//! (`docs/design/api.md` §2, §3.5, §3.6, §4).

use groups::router;
use groups::state::AppState;
use lambda_http::{service_fn, Error};
use media_delete::MediaDeleter;
use media_urls::MediaUrls;
use persistence::Repo;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let avatar_deleter =
        MediaDeleter::from_env(&aws_config, "AVATARS_ORIGINALS_BUCKET", "PROCESSED_BUCKET")?;

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        group_defaults: shared::config::default_group_settings(),
        vapid_public_key: std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default(),
        media_urls: MediaUrls::from_env(&aws_config)?,
        avatar_deleter,
    });

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
