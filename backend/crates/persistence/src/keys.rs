//! Pure key-builder functions for the single-table layout.
//!
//! Every `pk`/`sk`/`gsi1pk`/`gsi1sk`/`gsi2pk`/`gsi2sk` string the application
//! ever writes is constructed here. If a key format changes, change it here
//! and the type system finds every dependent call site.
//!
//! Layouts mirror `plans/02-data-model-dynamodb.md` §2.

use domain::{
    AvatarId, CognitoSub, CycleId, GroupId, ImageId, InviteCode, NewsletterStatus, QuestionId,
    UserId,
};
use shared::config::VOTE_COUNT_PAD_WIDTH;

// ---------- User (§2.1) ----------

pub fn user_pk(user_id: &UserId) -> String {
    format!("USER#{}", user_id)
}

pub const USER_PROFILE_SK: &str = "PROFILE";

// ---------- Cognito-sub lookup (§2.1a) ----------

pub fn cognito_sub_pk(sub: &CognitoSub) -> String {
    format!("COGNITO_SUB#{}", sub)
}

pub const COGNITO_SUB_SK: &str = "USER_ID";

// ---------- GroupMembership (§2.2) ----------

pub fn membership_sk(group_id: &GroupId) -> String {
    format!("GROUP#{}", group_id)
}

pub fn membership_gsi1pk(group_id: &GroupId) -> String {
    format!("GROUP#{}", group_id)
}

pub fn membership_gsi1sk(user_id: &UserId) -> String {
    format!("MEMBER#{}", user_id)
}

// ---------- Group (§2.3) ----------

pub fn group_pk(group_id: &GroupId) -> String {
    format!("GROUP#{}", group_id)
}

pub const GROUP_META_SK: &str = "META";

// ---------- Invite (§2.4) ----------

pub fn invite_pk(code: &InviteCode) -> String {
    format!("INVITE#{}", code)
}

pub const INVITE_SK: &str = "META";

pub fn invite_gsi1pk(group_id: &GroupId) -> String {
    format!("GROUP#{}", group_id)
}

pub fn invite_gsi1sk(code: &InviteCode) -> String {
    format!("INVITE#{}", code)
}

// ---------- Newsletter (§2.5) ----------

pub fn newsletter_sk(cycle_id: &CycleId) -> String {
    format!("NL#{}", cycle_id)
}

pub fn newsletter_gsi2pk(status: NewsletterStatus) -> String {
    let s = match status {
        NewsletterStatus::Voting => "voting",
        NewsletterStatus::Open => "open",
        NewsletterStatus::Published => "published",
        NewsletterStatus::Archived => "archived",
    };
    format!("NL_STATUS#{s}")
}

/// `{nextTransitionAtIso}#{groupId}#{cycleId}` (or sentinel `9999-...` in a
/// terminal state — caller chooses the sentinel).
pub fn newsletter_gsi2sk(
    next_transition_at_iso: &str,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> String {
    format!("{next_transition_at_iso}#{group_id}#{cycle_id}")
}

// ---------- Candidate question (§2.6) ----------

pub fn candidate_pk(group_id: &GroupId, next_cycle_id: &CycleId) -> String {
    format!("GROUP#{}#CYCLE#{}", group_id, next_cycle_id)
}

pub fn candidate_sk(question_id: &QuestionId) -> String {
    format!("QC#{}", question_id)
}

pub fn candidate_gsi1pk(group_id: &GroupId, next_cycle_id: &CycleId) -> String {
    format!("GROUP#{}#CYCLE#{}#VOTES", group_id, next_cycle_id)
}

/// Pads `vote_count` to a fixed width so lexicographic sort matches numeric.
pub fn candidate_gsi1sk(vote_count: u32, question_id: &QuestionId) -> String {
    format!(
        "{:0>width$}#{}",
        vote_count,
        question_id,
        width = VOTE_COUNT_PAD_WIDTH
    )
}

// ---------- Candidate vote (§2.7) ----------

pub fn candidate_vote_pk(
    group_id: &GroupId,
    next_cycle_id: &CycleId,
    voter_user_id: &UserId,
) -> String {
    format!(
        "GROUP#{}#CYCLE#{}#VOTER#{}",
        group_id, next_cycle_id, voter_user_id
    )
}

pub fn candidate_vote_sk(question_id: &QuestionId) -> String {
    format!("QC#{}", question_id)
}

// ---------- Locked question (§2.8) ----------

pub fn locked_pk(group_id: &GroupId, cycle_id: &CycleId) -> String {
    format!("GROUP#{}#NL#{}", group_id, cycle_id)
}

pub fn locked_sk(question_id: &QuestionId) -> String {
    format!("Q#{}", question_id)
}

// ---------- Response (§2.9) ----------

pub fn response_pk(group_id: &GroupId, cycle_id: &CycleId, question_id: &QuestionId) -> String {
    format!(
        "GROUP#{}#NL#{}#Q#{}",
        group_id, cycle_id, question_id
    )
}

pub fn response_sk(user_id: &UserId) -> String {
    format!("A#{}", user_id)
}

pub fn response_gsi1pk(user_id: &UserId, cycle_id: &CycleId) -> String {
    format!("USER#{}#NL#{}", user_id, cycle_id)
}

pub fn response_gsi1sk(question_id: &QuestionId) -> String {
    format!("Q#{}", question_id)
}

// ---------- ImageMedia (§2.10) ----------

pub fn image_pk(group_id: &GroupId, cycle_id: &CycleId) -> String {
    format!("GROUP#{}#NL#{}", group_id, cycle_id)
}

pub fn image_sk(image_id: &ImageId) -> String {
    format!("IMG#{}", image_id)
}

pub fn image_gsi1pk(user_id: &UserId) -> String {
    format!("USER#{}#IMG", user_id)
}

pub fn image_gsi1sk(uploaded_at_iso: &str, image_id: &ImageId) -> String {
    format!("{uploaded_at_iso}#{image_id}")
}

// ---------- Comment / Reaction (§2.11, §2.12) ----------

pub fn engagement_pk(
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> String {
    format!(
        "GROUP#{}#NL#{}#Q#{}#A#{}",
        group_id, cycle_id, question_id, answer_user_id
    )
}

pub fn comment_sk(created_at_iso: &str, comment_id: &domain::CommentId) -> String {
    format!("C#{}#{}", created_at_iso, comment_id)
}

pub fn reaction_sk(reactor_user_id: &UserId, emoji: &str) -> String {
    format!("R#{}#{}", reactor_user_id, emoji)
}

// ---------- Push subscription (§2.13) ----------

pub fn push_sk(endpoint_hash: &str) -> String {
    format!("PUSH#{}", endpoint_hash)
}

// ---------- Notification pref (§2.14) ----------

pub fn npref_sk(group_id: &GroupId) -> String {
    format!("NPREF#{}", group_id)
}

// ---------- Notification idempotency / tick markers (§2.15) ----------

pub fn notified_open_pk(group_id: &GroupId, cycle_id: &CycleId) -> String {
    locked_pk(group_id, cycle_id)
}

pub const NOTIFIED_OPEN_SK: &str = "NOTIFIED#OPEN";

pub fn notified_close_sk(offset_hours: u32) -> String {
    format!("NOTIFIED#CLOSE#{offset_hours}")
}

pub const TICK_PK: &str = "TICK";
pub const TICK_CYCLE_SK: &str = "CYCLE";
pub const TICK_NOTIFY_SK: &str = "NOTIFY";

// ---------- Avatar media (§2.16) ----------

pub fn avatar_sk(avatar_id: &AvatarId) -> String {
    format!("AVATAR#{}", avatar_id)
}

// ---------- Attribute names (centralised so handler code doesn't fight typos) ----------

pub mod attr {
    pub const PK: &str = "pk";
    pub const SK: &str = "sk";
    pub const GSI1PK: &str = "gsi1pk";
    pub const GSI1SK: &str = "gsi1sk";
    pub const GSI2PK: &str = "gsi2pk";
    pub const GSI2SK: &str = "gsi2sk";
    pub const ENTITY: &str = "entity";
    pub const TTL: &str = "ttl";
}

pub mod index {
    pub const GSI1: &str = "gsi1";
    pub const GSI2: &str = "gsi2";
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::*;

    fn uid() -> UserId { UserId::new("01HX1") }
    fn gid() -> GroupId { GroupId::new("01HG2") }
    fn cid() -> CycleId { CycleId::new("202606") }
    fn qid() -> QuestionId { QuestionId::new("01HQ3") }
    fn iid() -> ImageId { ImageId::new("01HI4") }

    #[test]
    fn user_keys() {
        assert_eq!(user_pk(&uid()), "USER#01HX1");
        assert_eq!(USER_PROFILE_SK, "PROFILE");
    }

    #[test]
    fn cognito_lookup_keys() {
        let sub = CognitoSub::new("abc-123");
        assert_eq!(cognito_sub_pk(&sub), "COGNITO_SUB#abc-123");
        assert_eq!(COGNITO_SUB_SK, "USER_ID");
    }

    #[test]
    fn membership_keys() {
        assert_eq!(membership_sk(&gid()), "GROUP#01HG2");
        assert_eq!(membership_gsi1pk(&gid()), "GROUP#01HG2");
        assert_eq!(membership_gsi1sk(&uid()), "MEMBER#01HX1");
    }

    #[test]
    fn group_keys() {
        assert_eq!(group_pk(&gid()), "GROUP#01HG2");
        assert_eq!(GROUP_META_SK, "META");
    }

    #[test]
    fn invite_keys() {
        let code = InviteCode::new("ABCDEFGHJKMNPQRS");
        assert_eq!(invite_pk(&code), "INVITE#ABCDEFGHJKMNPQRS");
        assert_eq!(invite_gsi1pk(&gid()), "GROUP#01HG2");
        assert_eq!(invite_gsi1sk(&code), "INVITE#ABCDEFGHJKMNPQRS");
    }

    #[test]
    fn newsletter_keys() {
        assert_eq!(newsletter_sk(&cid()), "NL#202606");
        assert_eq!(newsletter_gsi2pk(NewsletterStatus::Voting), "NL_STATUS#voting");
        assert_eq!(newsletter_gsi2pk(NewsletterStatus::Open), "NL_STATUS#open");
        assert_eq!(newsletter_gsi2pk(NewsletterStatus::Published), "NL_STATUS#published");
        assert_eq!(
            newsletter_gsi2sk("2026-06-01T00:00:00Z", &gid(), &cid()),
            "2026-06-01T00:00:00Z#01HG2#202606"
        );
    }

    #[test]
    fn candidate_keys_and_padding() {
        assert_eq!(candidate_pk(&gid(), &cid()), "GROUP#01HG2#CYCLE#202606");
        assert_eq!(candidate_sk(&qid()), "QC#01HQ3");
        assert_eq!(candidate_gsi1pk(&gid(), &cid()), "GROUP#01HG2#CYCLE#202606#VOTES");
        assert_eq!(candidate_gsi1sk(7, &qid()), "000007#01HQ3");
        assert_eq!(candidate_gsi1sk(0, &qid()), "000000#01HQ3");
        // pad width keeps lexicographic order matching numeric across realistic counts
        assert!(candidate_gsi1sk(9, &qid()) < candidate_gsi1sk(10, &qid()));
        assert!(candidate_gsi1sk(99, &qid()) < candidate_gsi1sk(100, &qid()));
    }

    #[test]
    fn candidate_vote_keys() {
        assert_eq!(
            candidate_vote_pk(&gid(), &cid(), &uid()),
            "GROUP#01HG2#CYCLE#202606#VOTER#01HX1"
        );
        assert_eq!(candidate_vote_sk(&qid()), "QC#01HQ3");
    }

    #[test]
    fn locked_keys() {
        assert_eq!(locked_pk(&gid(), &cid()), "GROUP#01HG2#NL#202606");
        assert_eq!(locked_sk(&qid()), "Q#01HQ3");
    }

    #[test]
    fn response_keys() {
        assert_eq!(
            response_pk(&gid(), &cid(), &qid()),
            "GROUP#01HG2#NL#202606#Q#01HQ3"
        );
        assert_eq!(response_sk(&uid()), "A#01HX1");
        assert_eq!(response_gsi1pk(&uid(), &cid()), "USER#01HX1#NL#202606");
        assert_eq!(response_gsi1sk(&qid()), "Q#01HQ3");
    }

    #[test]
    fn image_keys() {
        assert_eq!(image_pk(&gid(), &cid()), "GROUP#01HG2#NL#202606");
        assert_eq!(image_sk(&iid()), "IMG#01HI4");
        assert_eq!(image_gsi1pk(&uid()), "USER#01HX1#IMG");
        assert_eq!(
            image_gsi1sk("2026-06-01T00:00:00Z", &iid()),
            "2026-06-01T00:00:00Z#01HI4"
        );
    }

    #[test]
    fn engagement_keys() {
        let answer_user = UserId::new("01HU5");
        assert_eq!(
            engagement_pk(&gid(), &cid(), &qid(), &answer_user),
            "GROUP#01HG2#NL#202606#Q#01HQ3#A#01HU5"
        );
        let cmt = CommentId::new("01HC6");
        assert_eq!(
            comment_sk("2026-06-04T12:00:00Z", &cmt),
            "C#2026-06-04T12:00:00Z#01HC6"
        );
        assert_eq!(reaction_sk(&uid(), "🔥"), "R#01HX1#🔥");
    }

    #[test]
    fn push_and_pref_keys() {
        assert_eq!(push_sk("hash123"), "PUSH#hash123");
        assert_eq!(npref_sk(&gid()), "NPREF#01HG2");
    }

    #[test]
    fn tick_and_idempotency_keys() {
        assert_eq!(notified_open_pk(&gid(), &cid()), "GROUP#01HG2#NL#202606");
        assert_eq!(NOTIFIED_OPEN_SK, "NOTIFIED#OPEN");
        assert_eq!(notified_close_sk(48), "NOTIFIED#CLOSE#48");
        assert_eq!(TICK_PK, "TICK");
        assert_eq!(TICK_CYCLE_SK, "CYCLE");
        assert_eq!(TICK_NOTIFY_SK, "NOTIFY");
    }

    #[test]
    fn avatar_keys() {
        let a = AvatarId::new("01HA7");
        assert_eq!(avatar_sk(&a), "AVATAR#01HA7");
    }
}
