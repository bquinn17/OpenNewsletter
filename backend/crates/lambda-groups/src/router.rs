//! Method + path dispatch for the routes `ApiStack` points at this Lambda.

use crate::dto::MembershipListResponse;
use crate::state::AppState;
use crate::{group_routes, me};
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
        (&Method::GET, ["healthz"]) => Ok(http::json_response(
            200,
            &serde_json::json!({ "status": "ok" }),
            &correlation_id,
        )),

        (&Method::GET, ["config"]) => {
            let claims = http::auth_claims(req)?;
            let config = me::get_config(state, &claims).await?;
            Ok(http::json_response(200, &config, &correlation_id))
        }

        (&Method::GET, ["me"]) => {
            let user_id = caller(state, req).await?;
            let profile = me::get_me(state, &user_id).await?;
            Ok(http::json_response(200, &profile, &correlation_id))
        }

        (&Method::PATCH, ["me"]) => {
            let user_id = caller(state, req).await?;
            let profile = me::patch_me(state, &user_id, http::parse_body(req)?).await?;
            Ok(http::json_response(200, &profile, &correlation_id))
        }

        (&Method::GET, ["groups"]) => {
            let user_id = caller(state, req).await?;
            let memberships = me::list_memberships(state, &user_id).await?;
            Ok(http::json_response(
                200,
                &MembershipListResponse { memberships },
                &correlation_id,
            ))
        }

        (&Method::GET, ["groups", group_id]) => {
            let user_id = caller(state, req).await?;
            let group = group_routes::get_group(state, &user_id, &GroupId::new(*group_id)).await?;
            Ok(http::json_response(200, &group, &correlation_id))
        }

        (&Method::PATCH, ["groups", group_id]) => {
            let user_id = caller(state, req).await?;
            let group = group_routes::patch_group(
                state,
                &user_id,
                &GroupId::new(*group_id),
                http::parse_body(req)?,
            )
            .await?;
            Ok(http::json_response(200, &group, &correlation_id))
        }

        (&Method::DELETE, ["groups", group_id, "members", target]) => {
            let user_id = caller(state, req).await?;
            group_routes::remove_member(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &UserId::new(*target),
            )
            .await?;
            Ok(http::no_content(&correlation_id))
        }

        (&Method::PATCH, ["groups", group_id, "members", target]) => {
            let user_id = caller(state, req).await?;
            let member = group_routes::patch_member(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &UserId::new(*target),
                http::parse_body(req)?,
            )
            .await?;
            Ok(http::json_response(200, &member, &correlation_id))
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
