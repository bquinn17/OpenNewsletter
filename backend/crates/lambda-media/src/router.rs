//! Method + path dispatch for the routes `ApiStack` points at this Lambda
//! (`docs/design/api.md` §9).

use crate::handlers;
use crate::state::AppState;
use domain::api::{CreateAvatarRequest, CreateUploadRequest, PatchUploadRequest};
use domain::{ApiError, AvatarId, CycleId, GroupId, ImageId, UserId};
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
        (&Method::POST, ["uploads"]) => {
            let user_id = caller(state, req).await?;
            let body: CreateUploadRequest = http::parse_body(req)?;
            let response = handlers::create_upload(state, &user_id, body).await?;
            Ok(http::json_response(201, &response, &correlation_id))
        }

        (&Method::GET, ["uploads", image_id]) => {
            let user_id = caller(state, req).await?;
            let (group_id, cycle_id) = group_and_cycle_query(req)?;
            let response = handlers::get_upload(
                state,
                &user_id,
                &group_id,
                &cycle_id,
                &ImageId::new(*image_id),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::PATCH, ["uploads", image_id]) => {
            let user_id = caller(state, req).await?;
            let (group_id, cycle_id) = group_and_cycle_query(req)?;
            let body: PatchUploadRequest = http::parse_body(req)?;
            let response = handlers::patch_upload(
                state,
                &user_id,
                &group_id,
                &cycle_id,
                &ImageId::new(*image_id),
                body,
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::DELETE, ["uploads", image_id]) => {
            let user_id = caller(state, req).await?;
            let (group_id, cycle_id) = group_and_cycle_query(req)?;
            handlers::delete_upload(
                state,
                &user_id,
                &group_id,
                &cycle_id,
                &ImageId::new(*image_id),
            )
            .await?;
            Ok(http::no_content(&correlation_id))
        }

        (&Method::POST, ["uploads", image_id, "complete"]) => {
            let user_id = caller(state, req).await?;
            let (group_id, cycle_id) = group_and_cycle_query(req)?;
            let response = handlers::complete_upload(
                state,
                &user_id,
                &group_id,
                &cycle_id,
                &ImageId::new(*image_id),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::POST, ["avatars"]) => {
            let user_id = caller(state, req).await?;
            let body: CreateAvatarRequest = http::parse_body(req)?;
            let response = handlers::create_avatar(state, &user_id, body).await?;
            Ok(http::json_response(201, &response, &correlation_id))
        }

        (&Method::GET, ["avatars", avatar_id]) => {
            let user_id = caller(state, req).await?;
            let response =
                handlers::get_avatar(state, &user_id, &AvatarId::new(*avatar_id)).await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::DELETE, ["avatars", avatar_id]) => {
            let user_id = caller(state, req).await?;
            handlers::delete_avatar(state, &user_id, &AvatarId::new(*avatar_id)).await?;
            Ok(http::no_content(&correlation_id))
        }

        (method, _) => Err(ApiError::not_found(format!("no route for {method} {path}"))),
    }
}

async fn caller(state: &AppState, req: &Request) -> Result<UserId, ApiError> {
    let claims = http::auth_claims(req)?;
    auth::require_user_id(&state.repo, &claims.sub).await
}

fn required_query(req: &Request, key: &str) -> Result<String, ApiError> {
    http::query_param(req, key)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ApiError::invalid_field(key, format!("{key} is required")))
}

/// `groupId`/`cycleId` query params shared by every `/uploads/{imageId}...`
/// route (`docs/design/api.md` §9 — an `ImageMedia` row is keyed by
/// `(groupId, cycleId, imageId)`).
fn group_and_cycle_query(req: &Request) -> Result<(GroupId, CycleId), ApiError> {
    let group_id = required_query(req, "groupId")?;
    let cycle_id = required_query(req, "cycleId")?;
    Ok((GroupId::new(group_id), CycleId::new(cycle_id)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ApiErrorCode;
    use lambda_http::http::Request as HttpRequest;

    fn request(uri: &str) -> Request {
        HttpRequest::builder()
            .uri(uri)
            .body(Body::Empty)
            .expect("valid test request")
    }

    #[test]
    fn required_query_rejects_a_missing_param() {
        let req = request("/uploads/img1");
        let err = required_query(&req, "groupId").expect_err("groupId is missing");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        assert_eq!(err.field_errors[0].field, "groupId");
    }

    #[test]
    fn required_query_rejects_an_empty_param() {
        let req = request("/uploads/img1?groupId=");
        let err = required_query(&req, "groupId").expect_err("groupId is empty");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn required_query_accepts_a_present_param() {
        let req = request("/uploads/img1?groupId=g1");
        assert_eq!(required_query(&req, "groupId").unwrap(), "g1");
    }

    #[test]
    fn group_and_cycle_query_requires_both_params() {
        let req = request("/uploads/img1?groupId=g1");
        let err = group_and_cycle_query(&req).expect_err("cycleId is missing");
        assert_eq!(err.field_errors[0].field, "cycleId");

        let req = request("/uploads/img1?groupId=g1&cycleId=202606");
        let (group_id, cycle_id) = group_and_cycle_query(&req).unwrap();
        assert_eq!(group_id, GroupId::new("g1"));
        assert_eq!(cycle_id, CycleId::new("202606"));
    }
}
