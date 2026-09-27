//! Event-type dispatch and the dev-only HTTP route.
//!
//! This Lambda is invoked two ways (`plans/06-newsletter-lifecycle.md` §5,
//! `plans/03-api-contract.md` §11a.1):
//!
//! - An EventBridge Scheduler rule, firing every 5 minutes with a plain
//!   scheduled-event payload (no `requestContext`).
//! - API Gateway HTTP API v2, proxying `POST /admin/dev/tick/cycle` — a
//!   request always carries `requestContext.http`.
//!
//! `lambda_http::run` can't front both: its request deserializer only
//! recognizes proxy-integration shapes, so an EventBridge payload would fail
//! to parse. Instead this crate takes the event as a raw `serde_json::Value`
//! and switches on shape itself, which is robust to either input by
//! construction rather than by guessing at a library's tolerance for unknown
//! shapes.

use domain::{ApiError, GroupId};
use persistence::{auth, groups, newsletters};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::duration::parse_duration;
use crate::state::AppState;
use crate::tick::{run_tick, TickSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingEvent {
    /// An API Gateway HTTP API v2 proxy request.
    Http,
    /// Anything else — in practice, the EventBridge scheduled tick.
    Scheduled,
}

/// Distinguish the two invocation shapes by the one field that's unique to
/// API Gateway's HTTP API v2 payload and absent from every other event this
/// function could plausibly receive.
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
    let summary = run_tick(&state.repo).await;
    tracing::info!(
        transition_count = summary.transitions.len(),
        "scheduled tick complete"
    );
    serde_json::to_value(&summary).unwrap_or_else(|_| json!({ "transitions": [] }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DevTickBody {
    group_id: Option<String>,
    advance_cycle_closes_by: Option<String>,
}

/// Handle `POST /admin/dev/tick/cycle` (`plans/03-api-contract.md` §11a.1).
/// `event` is the raw API Gateway HTTP API v2 payload; the return value is
/// already shaped as the proxy-integration response API Gateway expects.
pub async fn handle_http(state: &AppState, event: &Value) -> Value {
    let correlation_id = correlation_id(event);

    match handle_http_inner(state, event, &correlation_id).await {
        Ok(summary) => http_response(200, json!(summary), &correlation_id),
        Err(err) => {
            if err.http_status() >= 500 {
                tracing::error!(code = err.code.as_str(), detail = %err.detail, correlation_id, "dev tick request failed");
            } else {
                tracing::info!(code = err.code.as_str(), detail = %err.detail, correlation_id, "dev tick request rejected");
            }
            http_response(
                err.http_status(),
                problem_json(&err, &correlation_id),
                &correlation_id,
            )
        }
    }
}

async fn handle_http_inner(
    state: &AppState,
    event: &Value,
    correlation_id: &str,
) -> Result<TickSummary, ApiError> {
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
    if method != "POST" || path != "/admin/dev/tick/cycle" {
        return Err(ApiError::not_found(format!("no route for {method} {path}")));
    }

    let user_id = auth::require_user_id(&state.repo, &jwt_sub(event)?).await?;

    let body: DevTickBody = match event.get("body").and_then(Value::as_str) {
        Some(raw) if !raw.is_empty() => serde_json::from_str(raw)
            .map_err(|e| ApiError::validation(format!("request body is not valid JSON: {e}")))?,
        _ => DevTickBody::default(),
    };

    match &body.advance_cycle_closes_by {
        Some(duration_raw) => {
            let group_id = body.group_id.as_deref().map(GroupId::new).ok_or_else(|| {
                ApiError::validation("groupId is required when advanceCycleClosesBy is set")
            })?;
            auth::require_membership(&state.repo, &user_id, &group_id, true).await?;

            let duration = parse_duration(duration_raw).map_err(ApiError::validation)?;
            match newsletters::rewind_active_cycle_deadline(&state.repo, &group_id, duration).await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(error = ?e, group_id = %group_id, correlation_id, "failed to rewind cycle deadline");
                    return Err(ApiError::internal("failed to rewind the cycle deadline"));
                }
            }
        }
        None => {
            require_admin_of_any_group(state, &user_id).await?;
        }
    }

    Ok(run_tick(&state.repo).await)
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
    if memberships.iter().any(|m| m.role == domain::Role::Admin) {
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
            "rawPath": "/admin/dev/tick/cycle",
            "requestContext": { "http": { "method": "POST", "path": "/admin/dev/tick/cycle" } },
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
