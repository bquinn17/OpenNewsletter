//! Cognito `PreSignUp` trigger.
//!
//! A pure pass-through that auto-confirms accounts. It never sees the
//! invite code — the hosted UI does not reliably forward `clientMetadata` from the
//! front channel, so invite consumption happens later against a real JWT via
//! `POST /invites/redeem` (`docs/design/auth.md` §3).

use serde_json::Value;

/// Echo the event back with `autoConfirmUser` set. Cognito requires the whole
/// event object in the response, not just the `response` block.
///
/// `autoVerifyEmail` is always false. `email_verified` must come only from the
/// Google IdP's attribute mapping, because `POST /invites/redeem` trusts it for
/// bound invites (`docs/design/auth.md` §4.2). Auto-verifying here would
/// mark Facebook addresses verified too, and a Google address verified on
/// first sign-in whatever Google said.
pub fn handle(mut event: Value) -> Value {
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
        response.insert("autoVerifyEmail".into(), Value::Bool(false));
    }

    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const EXTERNAL_PROVIDER: &str = "PreSignUp_ExternalProvider";

    #[test]
    fn federated_signups_are_confirmed_but_not_email_verified() {
        for user_name in ["Google_1234", "Facebook_5678"] {
            let event = handle(json!({
                "triggerSource": EXTERNAL_PROVIDER,
                "userName": user_name,
                "response": {}
            }));

            assert_eq!(event["response"]["autoConfirmUser"], json!(true));
            assert_eq!(event["response"]["autoVerifyEmail"], json!(false));
        }
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
