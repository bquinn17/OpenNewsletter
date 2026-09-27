//! `lambda-questions` — candidate question and vote routes
//! (`plans/03-api-contract.md` §6).

use lambda_http::{service_fn, Error};
use persistence::Repo;
use questions::{router, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        cdn_base_url: std::env::var("CDN_BASE_URL").unwrap_or_default(),
    });

    lambda_http::run(service_fn(move |req| {
        let state = Arc::clone(&state);
        async move { Ok::<_, Error>(router::route(&state, req).await) }
    }))
    .await
}
