//! Entity structs mirroring `docs/design/data-model.md`. Pure data — no I/O.

use crate::ids::*;
use crate::Timestamp;
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

/// Normalized (parameters stripped, lowercased) MIME type accepted by
/// `POST /audio-uploads` (`docs/design/media.md` §12,
/// `docs/design/media.md` §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioMimeType {
    #[serde(rename = "audio/webm")]
    Webm,
    #[serde(rename = "audio/ogg")]
    Ogg,
    #[serde(rename = "audio/mp4")]
    Mp4,
    #[serde(rename = "audio/x-m4a")]
    XM4a,
    #[serde(rename = "audio/aac")]
    Aac,
    #[serde(rename = "audio/mpeg")]
    Mpeg,
    #[serde(rename = "audio/wav")]
    Wav,
    #[serde(rename = "audio/x-wav")]
    XWav,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub user_id: UserId,
    pub cognito_sub: CognitoSub,
    pub email: String,
    pub display_name: String,
    pub avatar_color: String,
    pub avatar_media_id: Option<AvatarId>,
    pub created_at: Timestamp,
    pub last_login_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMembership {
    pub user_id: UserId,
    pub group_id: GroupId,
    pub role: Role,
    pub joined_at: Timestamp,
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
    pub created_at: Timestamp,
    pub created_by: UserId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub code: InviteCode,
    pub group_id: GroupId,
    pub created_by: UserId,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub status: InviteStatus,
    pub consumed_by: Option<UserId>,
    pub consumed_at: Option<Timestamp>,
    pub role_on_redeem: Role,
    /// When set (stored lowercased), only a caller whose verified ID-token
    /// `email` matches may redeem (`docs/design/data-model.md` §2.4).
    /// Absent on rows written before M13.5, which reads as unbound.
    #[serde(default)]
    pub bound_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Newsletter {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub status: NewsletterStatus,
    pub vote_window_open_at: Timestamp,
    pub vote_window_close_at: Timestamp,
    pub response_open_at: Timestamp,
    pub response_close_at: Timestamp,
    pub published_at: Option<Timestamp>,
    pub next_transition_at: Option<Timestamp>,
    pub locked_question_ids: Vec<QuestionId>,
    pub notified_offsets_hours: Vec<u32>,
    pub notified_on_open: bool,
    /// The group's `questionsPerCycle` and `votesPerUserPerCycle` when this
    /// cycle was created, so a later settings change only reaches cycles
    /// created after it (`docs/design/lifecycle.md` §6, D10). `None` on rows
    /// written before the snapshot existed; those read the group's current
    /// value ([`Newsletter::questions_to_promote`], [`Newsletter::vote_cap`]).
    #[serde(default)]
    pub questions_per_cycle: Option<u32>,
    #[serde(default)]
    pub votes_per_user_per_cycle: Option<u32>,
}

impl Newsletter {
    /// How many candidates promote into this cycle (§5.2).
    pub fn questions_to_promote(&self, current: &CycleSettings) -> u32 {
        self.questions_per_cycle
            .unwrap_or(current.questions_per_cycle)
    }

    /// How many votes each member may cast on this cycle's candidates.
    pub fn vote_cap(&self, current: &CycleSettings) -> u32 {
        self.votes_per_user_per_cycle
            .unwrap_or(current.votes_per_user_per_cycle)
    }
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
    pub submitted_at: Timestamp,
}

impl CandidateQuestion {
    /// The order candidates promote in, and the order `sort=top` lists them:
    /// `vote_count` DESC, then `submitted_at` ASC (the older suggestion wins a
    /// tie), then `question_id` ASC (`docs/design/lifecycle.md` §5.2).
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
    pub voted_at: Timestamp,
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
    pub locked_at: Timestamp,
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
    /// Up to `MAX_AUDIO_CLIPS_PER_RESPONSE` voice clips, text answers only
    /// (`docs/design/media.md` §12). `#[serde(default)]` so rows
    /// written before voice clips shipped load as an empty list.
    #[serde(default)]
    pub audio_media_ids: Vec<AudioId>,
    pub updated_at: Timestamp,
    pub published_at: Option<Timestamp>,
    /// Reactions on this answer from members other than its author
    /// (`docs/design/data-model.md` §2.9). Absent reads as 0; kept by the
    /// reaction transaction (§4 #8). `#[serde(default)]` so rows written
    /// before the leaderboard feature load as `0`.
    #[serde(default)]
    pub reaction_count: u32,
}

impl Response {
    /// Whether this answer has content worth publishing: a text answer with a
    /// non-whitespace body, at least one image, or at least one voice clip, or
    /// a poll answer with a pick. Non-empty drafts auto-publish at cycle close
    /// and count towards reminder totals (`docs/design/lifecycle.md` §5.3,
    /// `docs/design/notifications.md` §7.2, `docs/design/data-model.md` §4 #4a).
    pub fn is_non_empty(&self) -> bool {
        match self.kind {
            QuestionKind::Text => {
                self.has_text_body()
                    || !self.image_media_ids.is_empty()
                    || !self.audio_media_ids.is_empty()
            }
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
    pub uploaded_at: Timestamp,
    pub processed_at: Option<Timestamp>,
    /// Why `status` is `failed`: `IMAGE_TOO_LARGE` or `IMAGE_DECODE_FAILED`
    /// (`docs/design/media.md` §3.2). A deleted image has no row at all.
    #[serde(default)]
    pub error_message: Option<String>,
    /// The comment this `purpose=comment` image is attached to, if any. Set
    /// and cleared in the same transaction as the comment write
    /// (`docs/design/api.md` §8.2, §9.4). `#[serde(default)]` so rows written before M10 (which
    /// never had this attribute) load as `None`. Not exposed on
    /// `ImageMediaResponse`.
    #[serde(default)]
    pub attached_comment_id: Option<CommentId>,
}

/// One uploaded voice clip (`docs/design/data-model.md` "AudioMedia",
/// `docs/design/media.md` §12). Mirrors [`ImageMedia`], with
/// duration/waveform peaks in place of width/height.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMedia {
    pub audio_id: AudioId,
    pub user_id: UserId,
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: Option<QuestionId>,
    pub purpose: ImagePurpose,
    pub mime_type: AudioMimeType,
    pub original_key: String,
    /// S3 key in the processed bucket (`aud/.../clip.webm`) — `None` until
    /// processed.
    pub clip_key: Option<String>,
    pub status: MediaStatus,
    pub bytes: u64,
    pub duration_ms: Option<u64>,
    /// `AUDIO_PEAK_COUNT` waveform bucket amplitudes, 0-255, scaled so the
    /// loudest bucket is 255 (`docs/design/media.md` §12). `None`
    /// until processed.
    pub peaks: Option<Vec<u8>>,
    pub caption: Option<String>,
    pub uploaded_at: Timestamp,
    pub processed_at: Option<Timestamp>,
    /// Why `status` is `failed`: `AUDIO_TOO_LARGE`, `AUDIO_TOO_LONG` or
    /// `AUDIO_DECODE_FAILED`.
    #[serde(default)]
    pub error_message: Option<String>,
    /// The comment this `purpose=comment` clip is attached to, if any. Same
    /// contract as `ImageMedia::attached_comment_id`.
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
    pub uploaded_at: Timestamp,
    pub processed_at: Option<Timestamp>,
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
    /// At most one voice clip (`docs/design/media.md` §12).
    /// `#[serde(default)]` so rows written before voice clips shipped load
    /// as `None`.
    #[serde(default)]
    pub audio_media_id: Option<AudioId>,
    pub created_at: Timestamp,
    pub edited_at: Option<Timestamp>,
    pub deleted_at: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub group_id: GroupId,
    pub cycle_id: CycleId,
    pub question_id: QuestionId,
    pub answer_user_id: UserId,
    pub reactor_user_id: UserId,
    pub emoji: String,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushSubscription {
    pub user_id: UserId,
    pub endpoint_hash: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub created_at: Timestamp,
    pub last_success_at: Option<Timestamp>,
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

/// One member's leaderboard counters in one group (`docs/design/data-model.md`
/// §2.17). Kept on its own row, not on `GroupMembership`, so the stats survive
/// a leave + rejoin. Every writer upserts with `UpdateItem`, so a missing row
/// reads as all zeros.
///
/// The counters are signed: an `ADD -1` against history written before this
/// feature shipped can transiently dip a counter below 0 until
/// `scripts/backfill_member_stats.py` runs. `#[serde(default)]` on each so a
/// row written by an earlier, narrower writer (e.g. one that only ever set
/// `questions_asked`) still deserializes. The API clamps every value at 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberStats {
    pub group_id: GroupId,
    pub user_id: UserId,
    #[serde(default)]
    pub editions_answered: i64,
    #[serde(default)]
    pub reactions_received: i64,
    #[serde(default)]
    pub reactions_given: i64,
    #[serde(default)]
    pub comments_written: i64,
    #[serde(default)]
    pub questions_asked: i64,
    #[serde(default)]
    pub current_streak: i64,
    #[serde(default)]
    pub longest_streak: i64,
    /// `yyyymm` of the edition this member's streak was last settled for;
    /// absent until the first settlement (`docs/design/lifecycle.md` §5.3).
    #[serde(default)]
    pub streak_cycle: Option<CycleId>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            submitted_at: format!("2026-05-{day:02}T00:00:00Z").parse().unwrap(),
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
        answer_with_audio(kind, body, images, &[], pick)
    }

    fn answer_with_audio(
        kind: QuestionKind,
        body: Option<&str>,
        images: &[&str],
        audio: &[&str],
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
            audio_media_ids: audio.iter().map(|i| AudioId::new(*i)).collect(),
            updated_at: crate::Timestamp::now(),
            published_at: None,
            reaction_count: 0,
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
    fn a_text_answer_with_only_a_voice_clip_is_non_empty() {
        use QuestionKind::Text;
        assert!(answer_with_audio(Text, None, &[], &["aud-1"], None).is_non_empty());
        assert!(!answer_with_audio(Text, Some(" "), &[], &[], None).is_non_empty());
    }

    #[test]
    fn a_poll_answer_needs_a_pick() {
        use QuestionKind::Poll;
        assert!(answer(Poll, None, &[], Some("opt-a")).is_non_empty());
        assert!(!answer(Poll, None, &[], None).is_non_empty());
    }
}
