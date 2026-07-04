//! lambda-notify-tick — EventBridge-triggered notification tick.
//!
//! Full implementation lands in M11. This stub accepts any invocation and returns
//! a placeholder so the Lambda can be deployed via CDK from M3.5 onward.
//!
//! When ENV=dev and ApiStack exists (M11), the route
//! `POST /admin/dev/tick/notify` invokes this same binary for fast-forward
//! testing without waiting for the EventBridge schedule.

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde_json::{json, Value};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(_event: LambdaEvent<Value>) -> Result<Value, Error> {
    Ok(json!({
        "status": "stub",
        "message": "notify-tick not yet implemented — real logic lands in M11"
    }))
}
