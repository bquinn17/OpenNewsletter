//! Cognito `PreSignUp` trigger.
//!
//! A pure pass-through that auto-confirms federated accounts. It never sees the
//! invite code — the hosted UI does not reliably forward `clientMetadata` from the
//! front channel, so invite consumption happens later against a real JWT via
//! `POST /invites/redeem` (`plans/05-auth-flow.md` §3).

use serde_json::Value;

const EXTERNAL_PROVIDER: &str = "PreSignUp_ExternalProvider";

/// Echo the event back with the auto-confirm flags set. Cognito requires the whole
/// event object in the response, not just the `response` block.
pub fn handle(mut event: Value) -> Value {
    let is_federated = event
        .get("triggerSource")
        .and_then(Value::as_str)
        .is_some_and(|source| source == EXTERNAL_PROVIDER);

    let response = event
        .as_object_mut()
        .map(|event| {
            event
                .entry("response")
                .or_insert_with(|| Value::Object(Default::default()))
        })
        .and_then(Value::as_object_mut);

    if let Some(response) = response {
        response.insert("autoConfirmUser".into(), Value::Bool(true));
        response.insert("autoVerifyEmail".into(), Value::Bool(is_federated));
    }

    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn federated_signups_are_confirmed_and_verified() {
        let event = handle(json!({
            "triggerSource": EXTERNAL_PROVIDER,
            "userName": "Google_1234",
            "response": {}
        }));

        assert_eq!(event["response"]["autoConfirmUser"], json!(true));
        assert_eq!(event["response"]["autoVerifyEmail"], json!(true));
    }

    #[test]
    fn non_federated_signups_are_confirmed_but_not_email_verified() {
        let event = handle(json!({
            "triggerSource": "PreSignUp_AdminCreateUser",
            "response": {}
        }));

        assert_eq!(event["response"]["autoConfirmUser"], json!(true));
        assert_eq!(event["response"]["autoVerifyEmail"], json!(false));
    }

    #[test]
    fn a_missing_response_block_is_created() {
        let event = handle(json!({ "triggerSource": EXTERNAL_PROVIDER }));
        assert_eq!(event["response"]["autoConfirmUser"], json!(true));
    }

    #[test]
    fn the_rest_of_the_event_is_echoed_untouched() {
        let event = handle(json!({
            "triggerSource": EXTERNAL_PROVIDER,
            "userPoolId": "us-east-1_abc",
            "request": { "userAttributes": { "email": "member@example.com" } }
        }));

        assert_eq!(event["userPoolId"], json!("us-east-1_abc"));
        assert_eq!(
            event["request"]["userAttributes"]["email"],
            json!("member@example.com")
        );
    }
}
