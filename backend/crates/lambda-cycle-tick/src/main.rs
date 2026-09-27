//! `lambda-cycle-tick` — runs the newsletter lifecycle state machine
//! (`plans/06-newsletter-lifecycle.md` §5).
//!
//! One binary, two invocation sources: the EventBridge Scheduler rule (every
//! 5 minutes) and, when `ENV=dev`, API Gateway's `POST /admin/dev/tick/cycle`
//! (`plans/03-api-contract.md` §11a.1). See `dispatch.rs` for how the two are
//! told apart.
//!
//! `recursion_limit` raised: routing `dispatch`'s async fns through this crate's
//! own library boundary (so `lambda-questions`' end-to-end test can call
//! `cycle_tick::tick::run_tick` directly) makes the generated `async fn` state
//! machine type nesting deep enough that rustc's default layout-computation
//! depth limit is no longer enough.
#![recursion_limit = "256"]

use cycle_tick::{dispatch, state::AppState};
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use persistence::Repo;
use serde_json::Value;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let table = std::env::var("TABLE_NAME").map_err(|_| "TABLE_NAME is not set")?;

    let state = Arc::new(AppState {
        repo: Repo::new(aws_sdk_dynamodb::Client::new(&aws_config), table),
        env: std::env::var("ENV").unwrap_or_default(),
    });

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
