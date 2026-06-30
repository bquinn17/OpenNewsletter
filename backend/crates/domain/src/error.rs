//! API error catalog mirroring `plans/03-api-contract.md` §1.1.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorCode {
    Unauthenticated,
    Forbidden,
    NotFound,
    ValidationFailed,
    InviteInvalid,
    InviteExpired,
    InviteConsumed,
    MemberCapReached,
    LastAdmin,
    VoteCapReached,
    CycleNotVoting,
    CycleNotOpen,
    CycleNotPublished,
    ResponseVersionConflict,
    ImageLimitExceeded,
    ImageTooLarge,
    ImageBadType,
    CandidatePromoted,
    RateLimited,
    Internal,
}

impl ApiErrorCode {
    pub fn http_status(self) -> u16 {
        use ApiErrorCode::*;
        match self {
            Unauthenticated => 401,
            Forbidden => 403,
            NotFound => 404,
            ValidationFailed => 422,
            InviteInvalid => 400,
            InviteExpired => 410,
            InviteConsumed | MemberCapReached | LastAdmin | VoteCapReached
            | CycleNotVoting | CycleNotOpen | CycleNotPublished | ResponseVersionConflict
            | ImageLimitExceeded | CandidatePromoted => 409,
            ImageTooLarge => 413,
            ImageBadType => 415,
            RateLimited => 429,
            Internal => 500,
        }
    }
}

#[derive(Debug, Error)]
#[error("{code:?}: {detail}")]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub detail: String,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, detail: impl Into<String>) -> Self {
        Self { code, detail: detail.into() }
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Forbidden, detail)
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::NotFound, detail)
    }

    pub fn validation(detail: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::ValidationFailed, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Internal, detail)
    }

    pub fn http_status(&self) -> u16 {
        self.code.http_status()
    }
}
