//! `lambda-push` — push subscription routes and the cycle-open /
//! publication notification fan-outs, dispatched from one binary by event
//! shape (`plans/07-notifications.md`, M11 decision D2).
//!
//! `recursion_limit` raised for the same reason as `lambda-cycle-tick`'s
//! binary: routing through this crate's own library boundary nests the
//! generated `async fn` state machine type deep enough that rustc's default
//! layout-computation depth limit is no longer enough.
#![recursion_limit = "256"]

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use persistence::Repo;
use push::dispatch;
use push::sender::ReqwestPushSender;
use push::state::AppState;
use serde_json::Value;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;
    let vapid_secret_arn =
        std::env::var("VAPID_SECRET_ARN").map_err(|_| "VAPID_SECRET_ARN is not set")?;

    let state = Arc::new(AppState::new(
        Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        aws_sdk_secretsmanager::Client::new(&aws_config),
        vapid_secret_arn,
        Arc::new(ReqwestPushSender::new()),
        std::env::var("ENV").unwrap_or_default(),
    ));

    run(service_fn(move |event: LambdaEvent<Value>| {
        let state = Arc::clone(&state);
        async move { Ok::<Value, Error>(dispatch::handle(&state, event.payload).await) }
    }))
    .await
}
