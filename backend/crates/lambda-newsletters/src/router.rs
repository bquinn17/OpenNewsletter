//! Method + path dispatch for the routes `ApiStack` points at this Lambda
//! (`plans/03-api-contract.md` §5, §13).

use crate::handlers;
use crate::state::AppState;
use domain::{ApiError, CycleId, GroupId, NewsletterStatus, UserId};
use lambda_http::http::Method;
use lambda_http::{Body, Request, RequestExt, Response};
use persistence::auth;
use shared::config::DEFAULT_LIST_LIMIT;
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
        (&Method::GET, ["groups", group_id, "newsletters"]) => {
            let user_id = caller(state, req).await?;
            let status = parse_status(req)?;
            let limit = parse_limit(req)?;
            let cursor = http::query_param(req, "cursor");
            let response = handlers::list_newsletters(
                state,
                &user_id,
                &GroupId::new(*group_id),
                status,
                limit,
                cursor.as_deref(),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::GET, ["groups", group_id, "newsletters", cycle_id]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::get_newsletter_detail(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &CycleId::new(*cycle_id),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (method, _) => Err(ApiError::not_found(format!("no route for {method} {path}"))),
    }
}

async fn caller(state: &AppState, req: &Request) -> Result<UserId, ApiError> {
    let claims = http::auth_claims(req)?;
    auth::require_user_id(&state.repo, &claims.sub).await
}

fn parse_status(req: &Request) -> Result<Option<NewsletterStatus>, ApiError> {
    match http::query_param(req, "status").as_deref() {
        None => Ok(None),
        Some("voting") => Ok(Some(NewsletterStatus::Voting)),
        Some("open") => Ok(Some(NewsletterStatus::Open)),
        Some("published") => Ok(Some(NewsletterStatus::Published)),
        Some(other) => Err(ApiError::invalid_field(
            "status",
            format!("status must be one of voting, open, published; got `{other}`"),
        )),
    }
}

fn parse_limit(req: &Request) -> Result<u32, ApiError> {
    match http::query_param(req, "limit") {
        None => Ok(DEFAULT_LIST_LIMIT),
        Some(raw) => raw.parse::<u32>().map_err(|_| {
            ApiError::invalid_field(
                "limit",
                format!("limit must be a positive integer, got `{raw}`"),
            )
        }),
    }
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
