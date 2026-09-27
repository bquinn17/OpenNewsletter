//! Method + path dispatch for the routes `ApiStack` points at this Lambda
//! (`plans/03-api-contract.md` §6, §13).

use crate::handlers::{self, Sort};
use crate::state::AppState;
use domain::api::CreateCandidateRequest;
use domain::{ApiError, GroupId, QuestionId, UserId};
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
        (&Method::GET, ["groups", group_id, "candidate-questions"]) => {
            let user_id = caller(state, req).await?;
            let sort = Sort::parse(http::query_param(req, "sort").as_deref())?;
            let limit = parse_limit(req)?;
            let cursor = http::query_param(req, "cursor");
            let response = handlers::list_candidates(
                state,
                &user_id,
                &GroupId::new(*group_id),
                sort,
                limit,
                cursor.as_deref(),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::POST, ["groups", group_id, "candidate-questions"]) => {
            let user_id = caller(state, req).await?;
            let body: CreateCandidateRequest = http::parse_body(req)?;
            let response =
                handlers::create_candidate(state, &user_id, &GroupId::new(*group_id), body).await?;
            Ok(http::json_response(201, &response, &correlation_id))
        }

        (&Method::POST, ["groups", group_id, "candidate-questions", question_id, "votes"]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::cast_vote(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &QuestionId::new(*question_id),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::DELETE, ["groups", group_id, "candidate-questions", question_id, "votes"]) => {
            let user_id = caller(state, req).await?;
            let response = handlers::withdraw_vote(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &QuestionId::new(*question_id),
            )
            .await?;
            Ok(http::json_response(200, &response, &correlation_id))
        }

        (&Method::DELETE, ["admin", "groups", group_id, "candidate-questions", question_id]) => {
            let user_id = caller(state, req).await?;
            handlers::admin_delete_candidate(
                state,
                &user_id,
                &GroupId::new(*group_id),
                &QuestionId::new(*question_id),
            )
            .await?;
            Ok(http::no_content(&correlation_id))
        }

        (method, _) => Err(ApiError::not_found(format!("no route for {method} {path}"))),
    }
}

async fn caller(state: &AppState, req: &Request) -> Result<UserId, ApiError> {
    let claims = http::auth_claims(req)?;
    auth::require_user_id(&state.repo, &claims.sub).await
}

fn parse_limit(req: &Request) -> Result<u32, ApiError> {
    match http::query_param(req, "limit") {
        None => Ok(DEFAULT_LIST_LIMIT),
        Some(raw) => raw.parse::<u32>().map_err(|_| {
            ApiError::validation(format!("limit must be a positive integer, got `{raw}`"))
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
