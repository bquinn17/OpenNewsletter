//! `lambda-groups` — `/config`, `/healthz`, `/me`, and the group + member routes
//! (`plans/03-api-contract.md` §2, §3.5, §3.6, §4).

use groups::router;
use groups::state::AppState;
use lambda_http::{service_fn, Error};
use persistence::Repo;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        group_defaults: std::env::var("CONFIG_JSON")
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        vapid_public_key: std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default(),
        cdn_base_url: std::env::var("CDN_BASE_URL").unwrap_or_default(),
    });

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
