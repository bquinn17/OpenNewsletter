//! lambda-cycle-tick — EventBridge-triggered lifecycle tick.
//!
//! Full implementation lands in M5. This stub accepts any invocation and returns
//! a placeholder so the Lambda can be deployed via CDK from M3.5 onward.
//!
//! When ENV=dev and ApiStack exists (M5), the route
//! `POST /admin/dev/tick/cycle` invokes this same binary for fast-forward
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
        "message": "cycle-tick not yet implemented — real logic lands in M5"
    }))
}
