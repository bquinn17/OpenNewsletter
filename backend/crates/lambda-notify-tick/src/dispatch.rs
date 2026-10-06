//! Event-type dispatch and the dev-only HTTP route.
//!
//! This Lambda is invoked two ways (`plans/07-notifications.md` §7,
//! `plans/03-api-contract.md` §11a.2):
//!
//! - An EventBridge Scheduler rule, firing every 15 minutes with a plain
//!   scheduled-event payload (no `requestContext`).
//! - API Gateway HTTP API v2, proxying `POST /admin/dev/tick/notify` — a
//!   request always carries `requestContext.http`.
//!
//! Mirrors `lambda-cycle-tick/src/dispatch.rs`'s raw-`Value` dispatch for
//! the same reason: `lambda_http::run` can't front both shapes.

use domain::{ApiError, Role};
use persistence::{auth, groups};
use push::state::AppState;
use serde_json::{json, Value};

use crate::tick::{run_notify_tick, NotifyTickSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingEvent {
    Http,
    Scheduled,
}

pub fn classify_event(event: &Value) -> IncomingEvent {
    let is_http = event
        .get("requestContext")
        .and_then(|rc| rc.get("http"))
        .is_some();
    if is_http {
        IncomingEvent::Http
    } else {
        IncomingEvent::Scheduled
    }
}

/// Handle the scheduled invocation: run the tick globally, return the summary
/// for CloudWatch Logs (EventBridge itself ignores the return value).
pub async fn handle_scheduled(state: &AppState) -> Value {
    let summary = run_notify_tick(state).await;
    tracing::info!(
        fanout_count = summary.fanouts.len(),
        "scheduled notify tick complete"
    );
    serde_json::to_value(&summary).unwrap_or_else(|_| json!({ "fanouts": [] }))
}

/// Handle `POST /admin/dev/tick/notify` (`plans/03-api-contract.md` §11a.2).
pub async fn handle_http(state: &AppState, event: &Value) -> Value {
    let correlation_id = correlation_id(event);

    match handle_http_inner(state, event).await {
        Ok(summary) => http_response(200, json!(summary), &correlation_id),
        Err(err) => {
            if err.http_status() >= 500 {
                tracing::error!(code = err.code.as_str(), detail = %err.detail, correlation_id, "dev notify tick request failed");
            } else {
                tracing::info!(code = err.code.as_str(), detail = %err.detail, correlation_id, "dev notify tick request rejected");
            }
            http_response(
                err.http_status(),
                problem_json(&err, &correlation_id),
                &correlation_id,
            )
        }
    }
}

async fn handle_http_inner(state: &AppState, event: &Value) -> Result<NotifyTickSummary, ApiError> {
    if !state.is_dev() {
        return Err(ApiError::not_found(
            "dev routes are only available when ENV=dev",
        ));
    }

    let method = event
        .pointer("/requestContext/http/method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let path = event
        .get("rawPath")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if method != "POST" || path != "/admin/dev/tick/notify" {
        return Err(ApiError::not_found(format!("no route for {method} {path}")));
    }

    let user_id = auth::require_user_id(&state.repo, &jwt_sub(event)?).await?;
    require_admin_of_any_group(state, &user_id).await?;

    Ok(run_notify_tick(state).await)
}

async fn require_admin_of_any_group(
    state: &AppState,
    user_id: &domain::UserId,
) -> Result<(), ApiError> {
    let memberships = groups::list_memberships_for_user(&state.repo, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to list memberships for dev tick admin check");
            ApiError::internal("failed to resolve caller's memberships")
        })?;
    if memberships.iter().any(|m| m.role == Role::Admin) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "caller must be an admin of at least one group",
        ))
    }
}

fn jwt_sub(event: &Value) -> Result<domain::CognitoSub, ApiError> {
    let sub = event
        .pointer("/requestContext/authorizer/jwt/claims/sub")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::unauthenticated("no JWT authorizer claims on the request"))?;
    Ok(domain::CognitoSub::new(sub))
}

fn correlation_id(event: &Value) -> String {
    event
        .pointer("/headers/x-correlation-id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string())
}

fn http_response(status: u16, body: Value, correlation_id: &str) -> Value {
    json!({
        "statusCode": status,
        "headers": {
            "content-type": "application/json",
            "x-correlation-id": correlation_id,
        },
        "body": body.to_string(),
        "isBase64Encoded": false,
    })
}

/// Mirrors `shared::http::problem_response`'s RFC-7807 body
/// (`plans/03-api-contract.md` §1.1), rebuilt here because this crate works
/// with raw JSON events/responses rather than `lambda_http::Request`/`Response`.
fn problem_json(err: &ApiError, correlation_id: &str) -> Value {
    let base = std::env::var("API_BASE_URL").unwrap_or_default();
    let type_uri = if base.is_empty() {
        format!("/errors/{}", err.code.slug())
    } else {
        format!("{}/errors/{}", base.trim_end_matches('/'), err.code.slug())
    };
    json!({
        "type": type_uri,
        "title": err.code.title(),
        "status": err.http_status(),
        "detail": err.detail,
        "code": err.code.as_str(),
        "correlationId": correlation_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_an_api_gateway_v2_request_as_http() {
        let event = json!({
            "version": "2.0",
            "rawPath": "/admin/dev/tick/notify",
            "requestContext": { "http": { "method": "POST", "path": "/admin/dev/tick/notify" } },
        });
        assert_eq!(classify_event(&event), IncomingEvent::Http);
    }

    #[test]
    fn classifies_an_eventbridge_scheduled_event_as_scheduled() {
        let event = json!({
            "version": "0",
            "id": "abc-123",
            "detail-type": "Scheduled Event",
            "source": "aws.scheduler",
            "account": "111111111111",
            "time": "2026-01-01T00:00:00Z",
            "region": "us-east-1",
            "resources": [],
            "detail": {},
        });
        assert_eq!(classify_event(&event), IncomingEvent::Scheduled);
    }

    #[test]
    fn classifies_an_empty_payload_as_scheduled() {
        assert_eq!(classify_event(&json!({})), IncomingEvent::Scheduled);
    }
}
