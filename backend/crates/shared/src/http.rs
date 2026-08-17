//! HTTP boundary helpers shared by every request-handling Lambda:
//! claim extraction, correlation IDs, and the RFC-7807 error shape from
//! `plans/03-api-contract.md` §1.1.

use domain::{ApiError, ApiErrorCode, CognitoSub};
use lambda_http::request::RequestContext;
use lambda_http::{Body, Request, RequestExt, Response};
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Claims lifted from the API Gateway JWT authorizer.
///
/// API Gateway has already validated issuer, audience, signature and expiry, so
/// handlers trust these values without re-verifying (`plans/05-auth-flow.md` §6).
#[derive(Debug, Clone)]
pub struct AuthClaims {
    pub sub: CognitoSub,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl AuthClaims {
    /// The email claim, which only an ID token carries. Handlers that must create
    /// a `User` row need it; the SPA is specified to send the ID token.
    pub fn require_email(&self) -> Result<&str, ApiError> {
        self.email.as_deref().ok_or_else(|| {
            ApiError::validation("request must carry an ID token with an email claim")
        })
    }
}

pub fn auth_claims(req: &Request) -> Result<AuthClaims, ApiError> {
    let claims = match req.request_context_ref() {
        Some(RequestContext::ApiGatewayV2(ctx)) => ctx
            .authorizer
            .as_ref()
            .and_then(|a| a.jwt.as_ref())
            .map(|j| &j.claims),
        _ => None,
    }
    .ok_or_else(|| ApiError::unauthenticated("no JWT authorizer claims on the request"))?;

    let sub = claims
        .get("sub")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::unauthenticated("JWT claims carry no sub"))?;

    Ok(AuthClaims {
        sub: CognitoSub::new(sub.as_str()),
        email: claims.get("email").filter(|s| !s.is_empty()).cloned(),
        name: claims.get("name").filter(|s| !s.is_empty()).cloned(),
    })
}

/// The caller's correlation ID, or a freshly minted one when absent.
pub fn correlation_id(req: &Request) -> String {
    req.headers()
        .get(CORRELATION_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::now_v7().to_string())
}

pub fn parse_body<T: DeserializeOwned>(req: &Request) -> Result<T, ApiError> {
    serde_json::from_slice(req.body().as_ref())
        .map_err(|e| ApiError::validation(format!("request body is not valid JSON: {e}")))
}

pub fn json_response<T: Serialize>(status: u16, body: &T, correlation_id: &str) -> Response<Body> {
    let payload = match serde_json::to_string(body) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::error!(error = ?e, correlation_id, "failed to serialize response body");
            return problem_response(
                &ApiError::internal("failed to serialize response"),
                correlation_id,
            );
        }
    };
    build(status, "application/json", payload, correlation_id)
}

pub fn no_content(correlation_id: &str) -> Response<Body> {
    build(204, "application/json", String::new(), correlation_id)
}

/// RFC-7807 Problem Details body (`plans/03-api-contract.md` §1.1).
pub fn problem_response(err: &ApiError, correlation_id: &str) -> Response<Body> {
    let problem = serde_json::json!({
        "type": error_type_uri(err.code),
        "title": err.code.title(),
        "status": err.http_status(),
        "detail": err.detail,
        "code": err.code.as_str(),
        "correlationId": correlation_id,
    });
    let payload = serde_json::to_string(&problem).unwrap_or_else(|_| "{}".to_owned());
    build(
        err.http_status(),
        "application/problem+json",
        payload,
        correlation_id,
    )
}

fn error_type_uri(code: ApiErrorCode) -> String {
    match std::env::var("API_BASE_URL") {
        Ok(base) if !base.is_empty() => {
            format!("{}/errors/{}", base.trim_end_matches('/'), code.slug())
        }
        _ => format!("/errors/{}", code.slug()),
    }
}

fn build(status: u16, content_type: &str, body: String, correlation_id: &str) -> Response<Body> {
    // A builder failure here means a malformed status or header value, neither of
    // which is reachable from the fixed inputs above.
    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header(CORRELATION_ID_HEADER, correlation_id)
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::Empty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_http::http::Request as HttpRequest;

    fn request_with_header(value: &str) -> Request {
        HttpRequest::builder()
            .header(CORRELATION_ID_HEADER, value)
            .body(Body::Empty)
            .expect("valid test request")
    }

    #[test]
    fn correlation_id_is_echoed_from_the_request() {
        let req = request_with_header("01HXCORRELATION");
        assert_eq!(correlation_id(&req), "01HXCORRELATION");
    }

    #[test]
    fn correlation_id_is_generated_when_absent() {
        let req = HttpRequest::builder()
            .body(Body::Empty)
            .expect("valid test request");
        assert!(!correlation_id(&req).is_empty());
    }

    #[test]
    fn missing_authorizer_claims_are_unauthenticated() {
        let req = HttpRequest::builder()
            .body(Body::Empty)
            .expect("valid test request");
        let err = auth_claims(&req).expect_err("no authorizer context");
        assert_eq!(err.code, ApiErrorCode::Unauthenticated);
    }

    #[test]
    fn problem_body_carries_the_machine_readable_code() {
        let err = ApiError::forbidden("not a member of group 01HG2");
        let response = problem_response(&err, "01HXCORRELATION");
        assert_eq!(response.status(), 403);

        let Body::Text(body) = response.body() else {
            panic!("expected a text body");
        };
        let parsed: serde_json::Value =
            serde_json::from_str(body).expect("problem body is valid JSON");
        assert_eq!(parsed["code"], "FORBIDDEN");
        assert_eq!(parsed["correlationId"], "01HXCORRELATION");
        assert_eq!(parsed["detail"], "not a member of group 01HG2");
    }
}
