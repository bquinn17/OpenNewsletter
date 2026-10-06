//! Event-shape dispatch for the single `push-api` binary: API Gateway HTTP
//! routes vs. the internal fan-out invoke from `lambda-cycle-tick` (M11
//! decision D2). `requestContext` is checked first — an HTTP caller can
//! never forge an `internal` event, since API Gateway always wraps bodies
//! in that envelope. Anything matching neither shape is logged at ERROR and
//! dropped (mirrors `lambda-cycle-tick/src/dispatch.rs`).

use crate::state::AppState;
use crate::{fanout, router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use domain::{CycleId, GroupId};
use lambda_http::Body;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncomingEvent {
    /// An API Gateway HTTP API v2 proxy request.
    Http,
    /// The async-invoke envelope from `lambda-cycle-tick`.
    Internal,
    /// Neither shape — logged and dropped.
    Unknown,
}

pub fn classify_event(event: &Value) -> IncomingEvent {
    if event
        .get("requestContext")
        .and_then(|rc| rc.get("http"))
        .is_some()
    {
        IncomingEvent::Http
    } else if event.get("internal").is_some() {
        IncomingEvent::Internal
    } else {
        IncomingEvent::Unknown
    }
}

pub async fn handle(state: &AppState, event: Value) -> Value {
    match classify_event(&event) {
        IncomingEvent::Http => handle_http(state, &event).await,
        IncomingEvent::Internal => {
            handle_internal(state, &event).await;
            json!({})
        }
        IncomingEvent::Unknown => {
            tracing::error!(
                "push-api received an event matching neither the HTTP nor internal shape"
            );
            json!({})
        }
    }
}

async fn handle_http(state: &AppState, event: &Value) -> Value {
    let request = match lambda_http::request::from_str(&event.to_string()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = ?e, "failed to parse API Gateway event");
            return api_gateway_error_value();
        }
    };
    let response = router::route(state, request).await;
    to_api_gateway_value(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InternalEvent {
    internal: String,
    group_id: String,
    cycle_id: String,
}

async fn handle_internal(state: &AppState, event: &Value) {
    let parsed: InternalEvent = match serde_json::from_value(event.clone()) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = ?e, "malformed internal fanout event");
            return;
        }
    };
    let group_id = GroupId::new(parsed.group_id);
    let cycle_id = CycleId::new(parsed.cycle_id);
    match parsed.internal.as_str() {
        "cycle_open_fanout" => fanout::cycle_open_fanout(state, &group_id, &cycle_id).await,
        "publication_fanout" => fanout::publication_fanout(state, &group_id, &cycle_id).await,
        other => tracing::error!(internal = other, "unknown internal fanout kind"),
    }
}

fn api_gateway_error_value() -> Value {
    json!({
        "statusCode": 500,
        "headers": {"content-type": "application/problem+json"},
        "body": r#"{"code":"INTERNAL","detail":"failed to parse request"}"#,
        "isBase64Encoded": false,
    })
}

/// Converts the `http::Response<Body>` our router builds into the API
/// Gateway HTTP API v2 proxy-response JSON shape (mirrors
/// `lambda-cycle-tick/src/dispatch.rs::http_response`, generalized to carry
/// real headers and a body built elsewhere).
fn to_api_gateway_value(response: lambda_http::Response<Body>) -> Value {
    let (parts, body) = response.into_parts();
    let (is_base64_encoded, body_str) = match body {
        Body::Empty => (false, String::new()),
        Body::Text(t) => (false, t),
        Body::Binary(b) => (true, STANDARD.encode(b)),
        _ => (false, String::new()),
    };

    let mut headers = serde_json::Map::new();
    for (name, value) in parts.headers.iter() {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_owned(), json!(v));
        }
    }

    json!({
        "statusCode": parts.status.as_u16(),
        "headers": headers,
        "body": body_str,
        "isBase64Encoded": is_base64_encoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_an_api_gateway_v2_request_as_http() {
        let event = json!({
            "version": "2.0",
            "rawPath": "/push/subscriptions",
            "requestContext": { "http": { "method": "GET", "path": "/push/subscriptions" } },
        });
        assert_eq!(classify_event(&event), IncomingEvent::Http);
    }

    #[test]
    fn classifies_an_internal_fanout_event() {
        let event = json!({
            "internal": "cycle_open_fanout",
            "groupId": "01HG2",
            "cycleId": "202610",
        });
        assert_eq!(classify_event(&event), IncomingEvent::Internal);
    }

    #[test]
    fn classifies_an_empty_payload_as_unknown() {
        assert_eq!(classify_event(&json!({})), IncomingEvent::Unknown);
    }

    #[test]
    fn requestcontext_wins_over_a_forged_internal_field() {
        // An HTTP caller can never reach `handle_internal`, even if they
        // somehow got `internal` into their JSON body — `requestContext` is
        // checked first and only API Gateway can set it.
        let event = json!({
            "requestContext": { "http": { "method": "POST", "path": "/push/subscribe" } },
            "body": r#"{"internal":"cycle_open_fanout"}"#,
        });
        assert_eq!(classify_event(&event), IncomingEvent::Http);
    }
}
