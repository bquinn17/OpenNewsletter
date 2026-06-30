//! Entity structs mirroring `plans/02-data-model-dynamodb.md`. Pure data — no I/O.

use crate::ids::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NewsletterStatus {
    Voting,
    Open,
    Published,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InviteStatus {
    Pending,
    Consumed,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaStatus {
    Pending,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuestionKind {
    Text,
    Poll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMimeType {
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/webp")]
    Webp,
    #[serde(rename = "image/gif")]
    Gif,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub user_id: UserId,
    pub cognito_sub: CognitoSub,
    pub email: String,
    pub display_name: String,
    pub avatar_color: String,
    pub avatar_media_id: Option<AvatarId>,
    pub created_at: DateTime<Utc>,
    pub last_login_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembership {
    pub user_id: UserId,
    pub group_id: GroupId,
    pub role: Role,
    pub joined_at: DateTime<Utc>,
    pub editions_answered: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleSettings {
    pub questions_per_cycle: u32,
    pub votes_per_user_per_cycle: u32,
    pub response_window_days: u32,
    pub auto_publish: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationSettings {
    pub offsets_hours_before_close: Vec<u32>,
    pub on_cycle_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub group_id: GroupId,
    pub name: String,
    pub timezone: String,
    pub cycle_settings: CycleSettings,
    pub notification_settings: NotificationSettings,
    pub member_count: u32,
    pub member_soft_cap: u32,
    pub gradient: String,
    pub created_at: DateTime<Utc>,
    pub created_by: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub code: InviteCode,
    pub group_id: GroupId,
    pub created_by: UserId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: InviteStatus,
    pub consumed_by: Option<UserId>,
    pub consumed_at: Option<DateTime<Utc>>,
    pub role_on_redeem: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Newsletter {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub status: NewsletterStatus,
    pub vote_window_open_at: DateTime<Utc>,
    pub vote_window_close_at: DateTime<Utc>,
    pub response_open_at: DateTime<Utc>,
    pub response_close_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub next_transition_at: Option<DateTime<Utc>>,
    pub locked_question_ids: Vec<QuestionId>,
    pub notified_offsets_hours: Vec<u32>,
    pub notified_on_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollOption {
    pub option_id: PollOptionId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateQuestion {
    pub question_id: QuestionId,
    pub group_id: GroupId,
    pub next_cycle_id: CycleId,
    pub kind: QuestionKind,
    pub prompt: String,
    pub poll_options: Option<Vec<PollOption>>,
    pub vote_count: u32,
    pub submitted_by: UserId,
    pub is_anonymous: bool,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateVote {
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub voted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedQuestion {
    pub question_id: QuestionId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub kind: QuestionKind,
    pub prompt: String,
    pub poll_options: Option<Vec<PollOption>>,
    pub display_order: u32,
    pub submitted_by: UserId,
    pub is_anonymous: bool,
    pub locked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub response_id: ResponseId,
    pub user_id: UserId,
    pub question_id: QuestionId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub kind: QuestionKind,
    pub status: ResponseStatus,
    pub body: Option<String>,
    pub poll_option_id: Option<PollOptionId>,
    pub image_media_ids: Vec<ImageId>,
    pub updated_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMedia {
    pub image_id: ImageId,
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: Option<QuestionId>,
    pub mime_type: ImageMimeType,
    pub original_key: String,
    pub display_key: Option<String>,
    pub thumb_key: Option<String>,
    pub status: MediaStatus,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub caption: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvatarMedia {
    pub avatar_id: AvatarId,
    pub user_id: UserId,
    pub mime_type: ImageMimeType,
    pub original_key: String,
    pub display_key: Option<String>,
    pub status: MediaStatus,
    pub bytes: u64,
    pub uploaded_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub comment_id: CommentId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub answer_user_id: UserId,
    pub author_user_id: UserId,
    pub body: String,
    pub image_media_id: Option<ImageId>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub answer_user_id: UserId,
    pub reactor_user_id: UserId,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushSubscription {
    pub user_id: UserId,
    pub endpoint_hash: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub created_at: DateTime<Utc>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub failure_count: u32,
    pub user_agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationPref {
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_open: bool,
    pub deadline_reminders: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CognitoSubLookup {
    pub cognito_sub: CognitoSub,
    pub user_id: UserId,
}
