//! Method + path dispatch for the six `/push/*` routes
//! (`plans/03-api-contract.md` §10).

use crate::handlers;
use crate::state::AppState;
use domain::api::{PushSubscribeRequest, PushUnsubscribeRequest, PutPushPreferenceRequest};
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
        (&Method::POST, ["push", "subscribe"]) => {
            let user_id = caller(state, req).await?;
            let body: PushSubscribeRequest = http::parse_body(req)?;
            let response = handlers::subscribe(state, &user_id, body).await?;
            Ok(http::json_response(201, &response, &correlation_id))
        }

        (&Method::POST, ["push", "unsubscribe"]) => {
            let user_id = caller(state, req).await?;
            let body: PushUnsubscribeRequest = http::parse_body(req)?;
            handlers::unsubscribe(state, &user_id, body).await?;
            Ok(http::no_content(&correlation_id))
        }

        (&Method::GET, ["push", "subscriptions"]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::list_subscriptions(state, &user_id).await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::POST, ["push", "test"]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::send_test(state, &user_id).await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::GET, ["push", "preferences"]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::list_preferences(state, &user_id).await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::PUT, ["push", "preferences", group_id]) => {
            let user_id = caller(state, req).await?;
            let body: PutPushPreferenceRequest = http::parse_body(req)?;
            let response =
                handlers::put_preference(state, &user_id, &GroupId::new(*group_id), body).await?;
            Ok(http::json_response(200, &response, &correlation_id))
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
