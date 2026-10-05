//! Wire types (HTTP request/response DTOs) mirroring `shared/openapi.yaml`.
//!
//! These are deliberately separate from `crate::entities`: DynamoDB attributes are
//! snake_case, the HTTP contract is camelCase, and the two are allowed to drift (a
//! `Group` row carries `created_by`, the API does not expose it). Sections below are
//! grouped by route family and each names the `plans/03-api-contract.md` section it
//! implements. `domain/tests/openapi_contract.rs` round-trips the YAML's examples
//! against these types — any new route's DTOs belong here, with an entry added to
//! that test's mapping table in the same PR.

use crate::{
    AvatarId, AvatarMedia, CommentId, CycleId, CycleSettings, Group, GroupId, ImageId, ImageMedia,
    ImageMimeType, ImagePurpose, Invite, InviteStatus, MediaStatus, NewsletterStatus,
    NotificationSettings, PollOptionId, QuestionId, QuestionKind, Response, ResponseId,
    ResponseStatus, Role, User, UserId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ===== `03-api-contract.md` §2 — Bootstrap / metadata routes =====

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipSummary {
    pub group_id: GroupId,
    pub role: Role,
    pub group_name: String,
    /// IANA timezone, so clients can label cycles without a `GET /groups/{g}` per group.
    pub timezone: String,
    /// `GradientSlug` for the group's switcher swatch.
    pub gradient: String,
}

/// `GET /config` response (§2.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// `null` until the caller redeems their first invite.
    pub user_id: Option<UserId>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub vapid_public_key: String,
    pub group_defaults: serde_json::Value,
    pub memberships: Vec<MembershipSummary>,
}

/// `GET /me` / `PATCH /me` response (§2.2, §2.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub avatar_color: String,
    pub avatar_media_id: Option<AvatarId>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl UserResponse {
    pub fn new(user: User, avatar_url: Option<String>) -> Self {
        Self {
            user_id: user.user_id,
            email: user.email,
            display_name: user.display_name,
            avatar_color: user.avatar_color,
            avatar_media_id: user.avatar_media_id,
            avatar_url,
            created_at: user.created_at,
        }
    }
}

/// `PATCH /me` request body (§2.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchMeRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_color: Option<String>,
    /// Absent leaves the photo alone; an explicit `null` clears it.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub avatar_media_id: Option<Option<AvatarId>>,
}

// ===== `03-api-contract.md` §3 — Invite & membership routes =====

/// `POST /admin/invites` request body (§3.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateInviteRequest {
    pub group_id: GroupId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_days: Option<u32>,
    pub role_on_redeem: Role,
}

/// `POST /admin/invites` response (§3.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteResponse {
    pub code: String,
    pub group_id: GroupId,
    pub expires_at: DateTime<Utc>,
    pub role_on_redeem: Role,
}

/// One row of `GET /admin/groups/{groupId}/invites` (§3.2).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteSummary {
    pub code: String,
    pub group_id: GroupId,
    pub status: InviteStatus,
    pub role_on_redeem: Role,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_by: Option<UserId>,
    pub consumed_at: Option<DateTime<Utc>>,
}

impl InviteSummary {
    pub fn new(invite: Invite, display_code: String) -> Self {
        Self {
            code: display_code,
            group_id: invite.group_id,
            status: invite.status,
            role_on_redeem: invite.role_on_redeem,
            created_by: invite.created_by,
            created_at: invite.created_at,
            expires_at: invite.expires_at,
            consumed_by: invite.consumed_by,
            consumed_at: invite.consumed_at,
        }
    }
}

/// `GET /admin/groups/{groupId}/invites` response (§3.2).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteListResponse {
    pub items: Vec<InviteSummary>,
    /// Always `null` in M4 — the invite list is bounded by the member soft cap.
    pub next_cursor: Option<String>,
}

/// `POST /invites/redeem` request body (§3.4).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RedeemRequest {
    pub code: String,
}

/// `POST /invites/redeem` response (§3.4).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemResponse {
    pub group_id: GroupId,
    pub group_name: String,
    pub role: Role,
}

// ===== `03-api-contract.md` §4 — Group routes =====

/// `GET /groups` response (§4.1) — the same join `/config` returns, standalone.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipListResponse {
    pub memberships: Vec<MembershipSummary>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberResponse {
    pub user_id: UserId,
    pub display_name: String,
    pub role: Role,
    pub avatar_color: String,
    pub avatar_url: Option<String>,
    pub joined_at: DateTime<Utc>,
    pub editions_answered: u32,
}

/// Wire mirror of `crate::CycleSettings`, camelCase and without the persistence-only
/// `autoPublish` flag (not part of the public contract — `03-api-contract.md` §4.2/§4.3
/// never mention it). Kept distinct from the entity so the entity can stay snake_case
/// for its DynamoDB attributes without leaking into the HTTP response.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleSettingsResponse {
    pub questions_per_cycle: u32,
    pub votes_per_user_per_cycle: u32,
    pub response_window_days: u32,
}

impl From<CycleSettings> for CycleSettingsResponse {
    fn from(settings: CycleSettings) -> Self {
        Self {
            questions_per_cycle: settings.questions_per_cycle,
            votes_per_user_per_cycle: settings.votes_per_user_per_cycle,
            response_window_days: settings.response_window_days,
        }
    }
}

/// Wire mirror of `crate::NotificationSettings` — see `CycleSettingsResponse` for why
/// this can't just be the entity type.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettingsResponse {
    pub offsets_hours_before_close: Vec<u32>,
    pub on_cycle_open: bool,
}

impl From<NotificationSettings> for NotificationSettingsResponse {
    fn from(settings: NotificationSettings) -> Self {
        Self {
            offsets_hours_before_close: settings.offsets_hours_before_close,
            on_cycle_open: settings.on_cycle_open,
        }
    }
}

/// `GET /groups/{groupId}` / `PATCH /groups/{groupId}` response (§4.2, §4.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResponse {
    pub group_id: GroupId,
    pub name: String,
    pub timezone: String,
    pub gradient: String,
    pub cycle_settings: CycleSettingsResponse,
    pub notification_settings: NotificationSettingsResponse,
    pub member_count: u32,
    pub member_soft_cap: u32,
    pub created_at: DateTime<Utc>,
    pub members: Vec<MemberResponse>,
}

impl GroupResponse {
    pub fn new(group: Group, members: Vec<MemberResponse>) -> Self {
        Self {
            group_id: group.group_id,
            name: group.name,
            timezone: group.timezone,
            gradient: group.gradient,
            cycle_settings: group.cycle_settings.into(),
            notification_settings: group.notification_settings.into(),
            member_count: group.member_count,
            member_soft_cap: group.member_soft_cap,
            created_at: group.created_at,
            members,
        }
    }
}

/// `PATCH /groups/{groupId}` request body — `cycleSettings` sub-patch (§4.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CycleSettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub questions_per_cycle: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes_per_user_per_cycle: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_window_days: Option<u32>,
}

/// `PATCH /groups/{groupId}` request body — `notificationSettings` sub-patch (§4.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationSettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offsets_hours_before_close: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_cycle_open: Option<bool>,
}

/// `PATCH /groups/{groupId}` request body (§4.3). Every field optional; absent means
/// "leave as-is".
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchGroupRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle_settings: Option<CycleSettingsPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_settings: Option<NotificationSettingsPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_soft_cap: Option<u32>,
}

/// `PATCH /groups/{groupId}/members/{userId}` request body (§3.6).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchMemberRequest {
    pub role: Role,
}

// ===== `03-api-contract.md` §5/§6 — Newsletter and candidate-question routes =====

/// "Who asked this" for a candidate or locked question (§6.1), redacted to `None`
/// when `isAnonymous=true` and the caller is neither the submitter nor a group admin.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskedBy {
    pub user_id: UserId,
    pub display_name: String,
    pub avatar_color: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollOptionResponse {
    pub option_id: PollOptionId,
    pub label: String,
}

/// One row of `GET /groups/{groupId}/candidate-questions` (§6.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateItemResponse {
    pub question_id: QuestionId,
    pub kind: QuestionKind,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_options: Option<Vec<PollOptionResponse>>,
    pub asked_by: Option<AskedBy>,
    pub is_anonymous: bool,
    pub vote_count: u32,
    pub voted_by_me: bool,
    pub submitted_at: DateTime<Utc>,
}

/// `GET /groups/{groupId}/candidate-questions` response (§6.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateListResponse {
    pub next_cycle_id: CycleId,
    pub votes_per_user_per_cycle: u32,
    pub my_vote_count: u32,
    pub items: Vec<CandidateItemResponse>,
    pub next_cursor: Option<String>,
}

/// One poll option in a `POST /groups/{groupId}/candidate-questions` request (§6.2).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePollOptionRequest {
    pub label: String,
}

/// `POST /groups/{groupId}/candidate-questions` request body (§6.2).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCandidateRequest {
    pub kind: QuestionKind,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_options: Option<Vec<CreatePollOptionRequest>>,
    /// Defaults to `false` — attributed to the submitter (§6.2).
    #[serde(default)]
    pub is_anonymous: bool,
}

/// `POST`/`DELETE .../votes` response (§6.3, §6.4) — both return this same shape.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateVoteResponse {
    pub question_id: QuestionId,
    pub vote_count: u32,
    pub voted_by_me: bool,
    pub my_vote_count: u32,
}

/// One row of `GET /groups/{groupId}/newsletters` (§5.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsletterSummary {
    pub cycle_id: CycleId,
    pub status: NewsletterStatus,
    pub response_open_at: DateTime<Utc>,
    pub response_close_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub question_count: u32,
    pub my_draft_count: u32,
    pub my_published_count: u32,
}

/// `GET /groups/{groupId}/newsletters` response (§5.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsletterListResponse {
    pub items: Vec<NewsletterSummary>,
    pub next_cursor: Option<String>,
}

/// The caller's own draft/published answer, embedded in the `open`-cycle detail
/// view (§5.2). Never another member's answer.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyResponseSummary {
    pub response_id: ResponseId,
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub image_media_ids: Vec<ImageId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_option_id: Option<PollOptionId>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

/// One locked question in the `open`-cycle detail view (§5.2).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestionResponse {
    pub question_id: QuestionId,
    pub kind: QuestionKind,
    pub prompt: String,
    pub display_order: u32,
    pub asked_by: Option<AskedBy>,
    pub is_anonymous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_options: Option<Vec<PollOptionResponse>>,
    pub my_response: Option<MyResponseSummary>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedImageResponse {
    pub image_id: ImageId,
    pub display_url: String,
    pub thumb_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedCommentResponse {
    pub comment_id: CommentId,
    pub author_user_id: UserId,
    pub display_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactionGroupResponse {
    pub emoji: String,
    pub count: u32,
    pub reacted_by_me: bool,
}

/// One member's published answer, hydrated with comments and reaction tallies
/// (§5.2). Only ever appears once the cycle is `published`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedAnswerResponse {
    pub response_id: ResponseId,
    pub user_id: UserId,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub images: Vec<PublishedImageResponse>,
    pub published_at: DateTime<Utc>,
    pub comments: Vec<PublishedCommentResponse>,
    pub reaction_groups: Vec<ReactionGroupResponse>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedPollOptionResponse {
    pub option_id: PollOptionId,
    pub label: String,
    pub vote_count: u32,
}

/// One locked question in the `published`-cycle detail view (§5.2). Exactly one
/// of `answers` (text) / `options`+`myVoteOptionId` (poll) is present, matching `kind`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedQuestionResponse {
    pub question_id: QuestionId,
    pub kind: QuestionKind,
    pub prompt: String,
    pub display_order: u32,
    pub asked_by: Option<AskedBy>,
    pub is_anonymous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answers: Option<Vec<PublishedAnswerResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<PublishedPollOptionResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub my_vote_option_id: Option<PollOptionId>,
}

/// `GET /groups/{groupId}/newsletters/{cycleId}` response (§5.2). Shape depends on
/// `status`. `archived` is a bare 410 (`ApiErrorCode::NewsletterArchived`), not
/// modeled here — see `06-newsletter-lifecycle.md` §1 (archival is deferred).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum NewsletterDetailResponse {
    #[serde(rename_all = "camelCase")]
    Voting {
        candidates: Vec<CandidateItemResponse>,
    },
    #[serde(rename_all = "camelCase")]
    Open {
        cycle_id: CycleId,
        response_open_at: DateTime<Utc>,
        response_close_at: DateTime<Utc>,
        questions: Vec<OpenQuestionResponse>,
    },
    #[serde(rename_all = "camelCase")]
    Published {
        cycle_id: CycleId,
        response_close_at: DateTime<Utc>,
        published_at: DateTime<Utc>,
        questions: Vec<PublishedQuestionResponse>,
    },
}

// ===== `03-api-contract.md` §7 — Response routes (drafts + publish) =====

/// A saved response — the full shape, returned by every §7 route (§7.1-§7.3).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseDto {
    pub response_id: ResponseId,
    pub user_id: UserId,
    pub question_id: QuestionId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub kind: QuestionKind,
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_option_id: Option<PollOptionId>,
    pub image_media_ids: Vec<ImageId>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

impl From<Response> for ResponseDto {
    fn from(r: Response) -> Self {
        Self {
            response_id: r.response_id,
            user_id: r.user_id,
            question_id: r.question_id,
            group_id: r.group_id,
            cycle_id: r.cycle_id,
            kind: r.kind,
            status: r.status,
            body: r.body,
            poll_option_id: r.poll_option_id,
            image_media_ids: r.image_media_ids,
            updated_at: r.updated_at,
            published_at: r.published_at,
        }
    }
}

/// `GET /groups/{groupId}/newsletters/{cycleId}/my-responses` response (§7.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MyResponsesList {
    pub items: Vec<ResponseDto>,
}

/// `PUT .../my-response` request body (§7.3), tagged by `kind`. Image captions
/// are set separately via `PATCH /uploads/{imageId}` (§9.6) — not part of this
/// payload.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum SaveResponseRequest {
    #[serde(rename_all = "camelCase")]
    Text {
        body: String,
        image_media_ids: Vec<ImageId>,
        publish: bool,
    },
    #[serde(rename_all = "camelCase")]
    Poll {
        poll_option_id: PollOptionId,
        publish: bool,
    },
}

// ===== `03-api-contract.md` §9 — Media routes =====

/// `POST /uploads` request body (§9.1). `mimeType` is deliberately a bare
/// `String`, not `ImageMimeType` — an unrecognized value (e.g. an SVG) must
/// reach the handler as `IMAGE_BAD_TYPE` (415), not fail JSON deserialization
/// into a generic 422.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateUploadRequest {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    /// Defaults to `response` when absent (`08-media-uploads.md` §3.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<ImagePurpose>,
    pub mime_type: String,
    pub byte_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// `POST /uploads` response (§9.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUploadResponse {
    pub image_id: ImageId,
    pub upload_url: String,
    /// Every header the presigned request was signed with, lower-cased. The
    /// client sends them all verbatim on the PUT. A `BTreeMap` keeps the
    /// iteration order stable for callers/tests.
    pub headers: BTreeMap<String, String>,
    pub expires_in_seconds: u64,
}

/// `GET /uploads/{imageId}`, `PATCH /uploads/{imageId}`, and
/// `POST /uploads/{imageId}/complete` response (§9.2, §9.3, §9.6).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMediaResponse {
    pub image_id: ImageId,
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: Option<QuestionId>,
    pub purpose: ImagePurpose,
    pub mime_type: ImageMimeType,
    pub status: MediaStatus,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub caption: Option<String>,
    /// Absolute CloudFront URL; `null` until `status=ready`.
    pub display_url: Option<String>,
    /// Absolute CloudFront URL; `null` until `status=ready`.
    pub thumb_url: Option<String>,
    pub error_message: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

impl ImageMediaResponse {
    pub fn new(image: ImageMedia, display_url: Option<String>, thumb_url: Option<String>) -> Self {
        Self {
            image_id: image.image_id,
            user_id: image.user_id,
            group_id: image.group_id,
            cycle_id: image.cycle_id,
            question_id: image.question_id,
            purpose: image.purpose,
            mime_type: image.mime_type,
            status: image.status,
            bytes: image.bytes,
            width: image.width,
            height: image.height,
            caption: image.caption,
            display_url,
            thumb_url,
            error_message: image.error_message,
            uploaded_at: image.uploaded_at,
            processed_at: image.processed_at,
        }
    }
}

/// `PATCH /uploads/{imageId}` request body (§9.6). `caption` is a required
/// key (per the YAML schema) whose value is either a string or `null` —
/// there is no "absent" state to distinguish, unlike `PatchMeRequest`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PatchUploadRequest {
    pub caption: Option<String>,
}

/// `GET /media-cookie` response (§9.5).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCookieResponse {
    pub policy: String,
    pub signature: String,
    pub key_pair_id: String,
    pub expires_at: DateTime<Utc>,
}

/// `POST /avatars` request body (§9.7). `mimeType` is a bare `String` for the
/// same reason as [`CreateUploadRequest::mime_type`].
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAvatarRequest {
    pub mime_type: String,
    pub byte_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// `POST /avatars` response (§9.7).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAvatarResponse {
    pub avatar_id: AvatarId,
    pub upload_url: String,
    pub headers: BTreeMap<String, String>,
    pub expires_in_seconds: u64,
}

/// `GET /avatars/{avatarId}` response (§9.7).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarMediaResponse {
    pub avatar_id: AvatarId,
    pub mime_type: ImageMimeType,
    pub status: MediaStatus,
    pub bytes: u64,
    /// Absolute, unsigned CloudFront URL; `null` until `status=ready`.
    pub avatar_url: Option<String>,
    pub error_message: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

impl AvatarMediaResponse {
    pub fn new(avatar: AvatarMedia, avatar_url: Option<String>) -> Self {
        Self {
            avatar_id: avatar.avatar_id,
            mime_type: avatar.mime_type,
            status: avatar.status,
            bytes: avatar.bytes,
            avatar_url,
            error_message: avatar.error_message,
            uploaded_at: avatar.uploaded_at,
            processed_at: avatar.processed_at,
        }
    }
}

// ===== `03-api-contract.md` §11 — Health =====

/// `GET /healthz` response (§11.1).
///
/// `version`/`build_at` come from the `BUILD_SHA`/`BUILD_AT` environment variables
/// at compile time (`make build-lambdas` sets both). A local `cargo build` has
/// neither, so `version` is `"unknown"` and `build_at` is `None`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub build_at: Option<String>,
}

// ===== `03-api-contract.md` §1.1 — RFC-7807 error shape =====

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

/// RFC-7807 Problem Details, as emitted by `shared::http::problem_response` (§1.1).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: String,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_errors: Option<Vec<FieldError>>,
}

/// Distinguishes an absent JSON field from an explicit `null`.
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_me_request_round_trips_an_explicit_avatar_clear() {
        let json = serde_json::json!({
            "displayName": "Quinn",
            "avatarColor": "teal",
            "avatarMediaId": null,
        });
        let parsed: PatchMeRequest = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed.avatar_media_id, Some(None));
        assert_eq!(serde_json::to_value(&parsed).unwrap(), json);
    }

    #[test]
    fn patch_me_request_omits_untouched_fields_on_reserialize() {
        let json = serde_json::json!({ "displayName": "Quinn" });
        let parsed: PatchMeRequest = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(parsed.avatar_media_id, None);
        assert_eq!(serde_json::to_value(&parsed).unwrap(), json);
    }

    #[test]
    fn newsletter_detail_response_tags_each_variant_by_status() {
        let voting = NewsletterDetailResponse::Voting {
            candidates: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(&voting).unwrap(),
            serde_json::json!({ "status": "voting", "candidates": [] })
        );

        let published = NewsletterDetailResponse::Published {
            cycle_id: CycleId::new("202606"),
            response_close_at: Utc::now(),
            published_at: Utc::now(),
            questions: Vec::new(),
        };
        let value = serde_json::to_value(&published).unwrap();
        assert_eq!(value["status"], "published");
        assert_eq!(value["cycleId"], "202606");
    }

    #[test]
    fn save_response_request_tags_text_and_poll_variants_by_kind() {
        let text_json = serde_json::json!({
            "kind": "text",
            "body": "Lake 22, hands down.",
            "imageMediaIds": ["01H..."],
            "publish": false,
        });
        let parsed: SaveResponseRequest = serde_json::from_value(text_json.clone()).unwrap();
        assert_eq!(
            parsed,
            SaveResponseRequest::Text {
                body: "Lake 22, hands down.".into(),
                image_media_ids: vec![ImageId::new("01H...")],
                publish: false,
            }
        );
        assert_eq!(serde_json::to_value(&parsed).unwrap(), text_json);

        let poll_json = serde_json::json!({
            "kind": "poll",
            "pollOptionId": "01H...",
            "publish": true,
        });
        let parsed: SaveResponseRequest = serde_json::from_value(poll_json.clone()).unwrap();
        assert_eq!(
            parsed,
            SaveResponseRequest::Poll {
                poll_option_id: PollOptionId::new("01H..."),
                publish: true,
            }
        );
        assert_eq!(serde_json::to_value(&parsed).unwrap(), poll_json);
    }

    #[test]
    fn create_candidate_request_defaults_is_anonymous_to_false() {
        let json = serde_json::json!({ "kind": "text", "prompt": "What's your favorite hike?" });
        let parsed: CreateCandidateRequest = serde_json::from_value(json).unwrap();
        assert!(!parsed.is_anonymous);
        assert_eq!(parsed.poll_options, None);
    }
}
