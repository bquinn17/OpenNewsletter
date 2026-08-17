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
    ImageLimitExceeded,
    ImageTooLarge,
    ImageBadType,
    CandidatePromoted,
    RateLimited,
    Internal,
}

impl ApiErrorCode {
    /// The machine-readable code as it appears on the wire (`03-api-contract.md` §1.1).
    pub fn as_str(self) -> &'static str {
        use ApiErrorCode::*;
        match self {
            Unauthenticated => "UNAUTHENTICATED",
            Forbidden => "FORBIDDEN",
            NotFound => "NOT_FOUND",
            ValidationFailed => "VALIDATION_FAILED",
            InviteInvalid => "INVITE_INVALID",
            InviteExpired => "INVITE_EXPIRED",
            InviteConsumed => "INVITE_CONSUMED",
            MemberCapReached => "MEMBER_CAP_REACHED",
            LastAdmin => "LAST_ADMIN",
            VoteCapReached => "VOTE_CAP_REACHED",
            CycleNotVoting => "CYCLE_NOT_VOTING",
            CycleNotOpen => "CYCLE_NOT_OPEN",
            CycleNotPublished => "CYCLE_NOT_PUBLISHED",
            ImageLimitExceeded => "IMAGE_LIMIT_EXCEEDED",
            ImageTooLarge => "IMAGE_TOO_LARGE",
            ImageBadType => "IMAGE_BAD_TYPE",
            CandidatePromoted => "CANDIDATE_PROMOTED",
            RateLimited => "RATE_LIMITED",
            Internal => "INTERNAL",
        }
    }

    /// Trailing path segment of the RFC-7807 `type` URI.
    pub fn slug(self) -> String {
        self.as_str().to_ascii_lowercase().replace('_', "-")
    }

    /// RFC-7807 `title` — a summary of the error class, invariant per code.
    pub fn title(self) -> &'static str {
        use ApiErrorCode::*;
        match self {
            Unauthenticated => "Not authenticated",
            Forbidden => "Not authorized",
            NotFound => "Not found",
            ValidationFailed => "Validation failed",
            InviteInvalid => "Invite code is not valid",
            InviteExpired => "Invite code has expired",
            InviteConsumed => "Invite code has already been used",
            MemberCapReached => "Group is at its member cap",
            LastAdmin => "Group must keep at least one admin",
            VoteCapReached => "No votes remaining this cycle",
            CycleNotVoting => "Cycle is not in the voting window",
            CycleNotOpen => "Cycle is not open for responses",
            CycleNotPublished => "Cycle is not published",
            ImageLimitExceeded => "Too many images attached",
            ImageTooLarge => "Image is too large",
            ImageBadType => "Unsupported image type",
            CandidatePromoted => "Candidate question is already locked into a cycle",
            RateLimited => "Too many requests",
            Internal => "Internal server error",
        }
    }

    pub fn http_status(self) -> u16 {
        use ApiErrorCode::*;
        match self {
            Unauthenticated => 401,
            Forbidden => 403,
            NotFound => 404,
            ValidationFailed => 422,
            InviteInvalid => 400,
            InviteExpired => 410,
            InviteConsumed | MemberCapReached | LastAdmin | VoteCapReached | CycleNotVoting
            | CycleNotOpen | CycleNotPublished | ImageLimitExceeded | CandidatePromoted => 409,
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
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn unauthenticated(detail: impl Into<String>) -> Self {
        Self::new(ApiErrorCode::Unauthenticated, detail)
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
