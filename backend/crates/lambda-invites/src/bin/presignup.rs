//! Cognito `PreSignUp` trigger entry point — `plans/05-auth-flow.md` §3.

use lambda_runtime::{service_fn, Error, LambdaEvent};
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Error> {
    shared::telemetry::init();

    lambda_runtime::run(service_fn(|event: LambdaEvent<Value>| async move {
        Ok::<_, Error>(invites::presignup::handle(event.payload))
    }))
    .await
}
