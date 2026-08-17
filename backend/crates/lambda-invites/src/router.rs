//! Method + path dispatch for the invite routes.

use crate::handlers;
use crate::state::AppState;
use domain::{ApiError, GroupId, UserId};
use lambda_http::http::Method;
use lambda_http::{Body, Request, RequestExt, Response};
use persistence::auth;
use shared::http;

pub async fn route(state: &AppState, req: Request) -> Response<Body> {
    let correlation_id = http::correlation_id(&req);
    match dispatch(state, &req).await {
        Ok(response) => response,
        Err(err) => {
            if err.http_status() >= 500 {
                tracing::error!(code = err.code.as_str(), detail = %err.detail, correlation_id, "request failed");
            } else {
                tracing::info!(code = err.code.as_str(), detail = %err.detail, correlation_id, "request rejected");
            }
            http::problem_response(&err, &correlation_id)
        }
    }
}

async fn dispatch(state: &AppState, req: &Request) -> Result<Response<Body>, ApiError> {
    let correlation_id = http::correlation_id(req);
    let path = request_path(req);
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    match (req.method(), segments.as_slice()) {
        (&Method::POST, ["admin", "invites"]) => {
            let user_id = caller(state, req).await?;
            let invite = handlers::create_invite(state, &user_id, http::parse_body(req)?).await?;
            Ok(http::json_response(201, &invite, &correlation_id))
        }

        (&Method::GET, ["admin", "groups", group_id, "invites"]) => {
            let user_id = caller(state, req).await?;
            let invites = handlers::list_invites(state, &user_id, &GroupId::new(*group_id)).await?;
            Ok(http::json_response(200, &invites, &correlation_id))
        }

        (&Method::POST, ["admin", "invites", code, "revoke"]) => {
            let user_id = caller(state, req).await?;
            handlers::revoke_invite(state, &user_id, code).await?;
            Ok(http::no_content(&correlation_id))
        }

        // Deliberately not behind `caller` — a first-time redeemer has a valid JWT
        // but no user row yet, which is exactly what this route creates.
        (&Method::POST, ["invites", "redeem"]) => {
            let claims = http::auth_claims(req)?;
            let result = handlers::redeem(state, &claims, http::parse_body(req)?).await?;
            Ok(http::json_response(200, &result, &correlation_id))
        }

        (method, _) => Err(ApiError::not_found(format!("no route for {method} {path}"))),
    }
}

async fn caller(state: &AppState, req: &Request) -> Result<UserId, ApiError> {
    let claims = http::auth_claims(req)?;
    auth::require_user_id(&state.repo, &claims.sub).await
}

/// API Gateway's `rawPath` excludes the stage; fall back to the URI for local calls.
fn request_path(req: &Request) -> String {
    let raw = req.raw_http_path();
    if raw.is_empty() {
        req.uri().path().to_owned()
    } else {
        raw.to_owned()
    }
}
