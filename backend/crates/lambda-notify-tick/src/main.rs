//! `lambda-notify-tick` — runs the deadline-reminder fan-out
//! (`plans/07-notifications.md` §7).
//!
//! One binary, two invocation sources: the EventBridge Scheduler rule (every
//! 15 minutes) and, when `ENV=dev`, API Gateway's `POST /admin/dev/tick/notify`
//! (`plans/03-api-contract.md` §11a.2). See `dispatch.rs` for how the two are
//! told apart.
#![recursion_limit = "256"]

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use notify_tick::dispatch;
use persistence::Repo;
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
        async move { Ok::<Value, Error>(handle(&state, event.payload).await) }
    }))
    .await
}

async fn handle(state: &AppState, event: Value) -> Value {
    match dispatch::classify_event(&event) {
        dispatch::IncomingEvent::Http => dispatch::handle_http(state, &event).await,
        dispatch::IncomingEvent::Scheduled => dispatch::handle_scheduled(state).await,
    }
}
