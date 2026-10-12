//! Entity structs mirroring `plans/02-data-model-dynamodb.md`. Pure data — no I/O.

use crate::ids::*;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

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

/// What an uploaded image is destined to attach to. Set at `POST /uploads` time;
/// the per-answer image cap counts `Response` rows only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImagePurpose {
    Response,
    Comment,
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
    /// When set (stored lowercased), only a caller whose verified ID-token
    /// `email` matches may redeem (`plans/02-data-model-dynamodb.md` §2.4).
    /// Absent on rows written before M13.5, which reads as unbound.
    #[serde(default)]
    pub bound_email: Option<String>,
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

impl CandidateQuestion {
    /// The order candidates promote in, and the order `sort=top` lists them:
    /// `vote_count` DESC, then `submitted_at` ASC (the older suggestion wins a
    /// tie), then `question_id` ASC (`plans/06-newsletter-lifecycle.md` §5.2).
    pub fn promotion_order(a: &Self, b: &Self) -> Ordering {
        b.vote_count
            .cmp(&a.vote_count)
            .then_with(|| a.submitted_at.cmp(&b.submitted_at))
            .then_with(|| a.question_id.as_ref().cmp(b.question_id.as_ref()))
    }
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

impl Response {
    /// Whether this answer has content worth publishing: a text answer with a
    /// non-whitespace body or at least one image, or a poll answer with a pick.
    /// Non-empty drafts auto-publish at cycle close and count towards reminder
    /// totals (`plans/06-newsletter-lifecycle.md` §5.3, `07-notifications.md` §7.2).
    pub fn is_non_empty(&self) -> bool {
        match self.kind {
            QuestionKind::Text => self.has_text_body() || !self.image_media_ids.is_empty(),
            QuestionKind::Poll => self.poll_option_id.is_some(),
        }
    }

    /// Whether the body has any non-whitespace text.
    pub fn has_text_body(&self) -> bool {
        self.body.as_deref().is_some_and(|b| !b.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMedia {
    pub image_id: ImageId,
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: Option<QuestionId>,
    pub purpose: ImagePurpose,
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
    /// Why `status` is `failed`: `IMAGE_TOO_LARGE` or `IMAGE_DECODE_FAILED`
    /// (`plans/08-media-uploads.md` §3.2). A deleted image has no row at all.
    #[serde(default)]
    pub error_message: Option<String>,
    /// The comment this `purpose=comment` image is attached to, if any. Set
    /// and cleared in the same transaction as the comment write (`03`
    /// §8.2, §9.4). `#[serde(default)]` so rows written before M10 (which
    /// never had this attribute) load as `None`. Not exposed on
    /// `ImageMediaResponse`.
    #[serde(default)]
    pub attached_comment_id: Option<CommentId>,
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
    /// Why `status` is `failed`; same values as `ImageMedia::error_message`.
    #[serde(default)]
    pub error_message: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn candidate(id: &str, votes: u32, day: u32) -> CandidateQuestion {
        CandidateQuestion {
            question_id: QuestionId::new(id),
            group_id: GroupId::new("g"),
            next_cycle_id: CycleId::new("202606"),
            kind: QuestionKind::Text,
            prompt: "Best hike?".into(),
            poll_options: None,
            vote_count: votes,
            submitted_by: UserId::new("u1"),
            is_anonymous: false,
            submitted_at: Utc.with_ymd_and_hms(2026, 5, day, 0, 0, 0).unwrap(),
        }
    }

    #[test]
    fn promotion_order_is_votes_then_oldest_then_id() {
        let mut pool = [
            candidate("q-c", 1, 1),
            candidate("q-b", 3, 9),
            candidate("q-z", 3, 2),
            candidate("q-y", 1, 1),
        ];
        pool.sort_by(CandidateQuestion::promotion_order);
        let ids: Vec<&str> = pool.iter().map(|c| c.question_id.as_str()).collect();
        assert_eq!(ids, ["q-z", "q-b", "q-c", "q-y"]);
    }

    fn answer(
        kind: QuestionKind,
        body: Option<&str>,
        images: &[&str],
        pick: Option<&str>,
    ) -> Response {
        Response {
            response_id: ResponseId::new("r"),
            user_id: UserId::new("u"),
            question_id: QuestionId::new("q"),
            group_id: GroupId::new("g"),
            cycle_id: CycleId::new("202606"),
            kind,
            status: ResponseStatus::Draft,
            body: body.map(str::to_owned),
            poll_option_id: pick.map(PollOptionId::new),
            image_media_ids: images.iter().map(|i| ImageId::new(*i)).collect(),
            updated_at: Utc::now(),
            published_at: None,
        }
    }

    #[test]
    fn a_text_answer_needs_words_or_an_image() {
        use QuestionKind::Text;
        assert!(answer(Text, Some("Lake 22."), &[], None).is_non_empty());
        assert!(answer(Text, Some(" "), &["img-1"], None).is_non_empty());
        assert!(!answer(Text, Some(" \n\t"), &[], None).is_non_empty());
        assert!(!answer(Text, None, &[], None).is_non_empty());
    }

    #[test]
    fn a_poll_answer_needs_a_pick() {
        use QuestionKind::Poll;
        assert!(answer(Poll, None, &[], Some("opt-a")).is_non_empty());
        assert!(!answer(Poll, None, &[], None).is_non_empty());
    }
}
