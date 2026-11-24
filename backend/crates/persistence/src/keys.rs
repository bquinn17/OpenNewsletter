//! Pure key-builder functions for the single-table layout.
//!
//! Every `pk`/`sk`/`gsi1pk`/`gsi1sk`/`gsi2pk`/`gsi2sk` string the application
//! ever writes is constructed here. If a key format changes, change it here
//! and the type system finds every dependent call site.
//!
//! Layouts mirror `docs/design/data-model.md` §2.

use domain::{
    AvatarId, CognitoSub, CycleId, GroupId, ImageId, InviteCode, NewsletterStatus, QuestionId,
    ResponseId, Timestamp, UserId,
};
use shared::config::VOTE_COUNT_PAD_WIDTH;

// ---------- Canonical key-timestamp formatting ----------

/// The fixed-width sentinel used for a terminal-state `gsi2sk` (no next
/// transition due). Sorts after any real timestamp `key_timestamp` can
/// produce, so a `<=` range query never matches it.
pub const SENTINEL_FAR_FUTURE_TIMESTAMP: &str = "9999-12-31T00:00:00Z";

/// The canonical format for any timestamp embedded in a DynamoDB key
/// attribute (`pk`/`sk`/`gsi1*`/`gsi2*`) or compared against one in a
/// key-condition expression: fixed-width `YYYY-MM-DDTHH:MM:SSZ`, whole
/// seconds, `Z` suffix, no fractional part.
///
/// `Timestamp`'s `Display`/serde form (`x.to_string()`) is *not* safe for
/// this: it always carries a `.sss` fractional part, so two timestamps that
/// fall in the same second can sort inconsistently against each other
/// lexicographically. Route every timestamp that ends up in a key attribute —
/// or as a range-query bound compared against one — through this function
/// instead. Plain (non-key) data attributes are unaffected and may keep
/// `x.to_string()`.
pub fn key_timestamp(dt: Timestamp) -> String {
    dt.to_key_string()
}

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

/// `gsi2pk` for a group's META row while it has no cycle yet — written by the
/// bootstrap/seed scripts on group creation, and cleared by
/// `lambda-cycle-tick`'s first-cycle pass once it creates (or finds) the
/// group's first `voting`/`open` cycle (`docs/design/lifecycle.md` §4.2).
/// Sparse: only a group actually awaiting its first cycle carries this GSI2
/// entry at all.
pub const GROUP_AWAITING_FIRST_CYCLE_GSI2PK: &str = "GROUP_STATUS#awaiting_first_cycle";

/// `gsi2sk` for [`GROUP_AWAITING_FIRST_CYCLE_GSI2PK`]: `{createdAt}#{groupId}`,
/// with `createdAt` in the canonical fixed-width [`key_timestamp`] format.
pub fn group_awaiting_first_cycle_gsi2sk(created_at: Timestamp, group_id: &GroupId) -> String {
    format!("{}#{group_id}", key_timestamp(created_at))
}

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

/// `{nextTransitionAt}#{groupId}#{cycleId}`, with `nextTransitionAt` in the
/// canonical fixed-width `key_timestamp` format. For a newsletter with no
/// next transition due (a terminal state), use
/// [`newsletter_gsi2sk_sentinel`] instead.
pub fn newsletter_gsi2sk(
    next_transition_at: Timestamp,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> String {
    format!(
        "{}#{group_id}#{cycle_id}",
        key_timestamp(next_transition_at)
    )
}

/// `gsi2sk` for a newsletter in a terminal state (no next transition due).
/// Uses [`SENTINEL_FAR_FUTURE_TIMESTAMP`], which sorts after every real
/// `key_timestamp` value, so `list_cycles_due`'s `<=` bound never matches it.
pub fn newsletter_gsi2sk_sentinel(group_id: &GroupId, cycle_id: &CycleId) -> String {
    format!("{SENTINEL_FAR_FUTURE_TIMESTAMP}#{group_id}#{cycle_id}")
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

/// Every `CandidateVote` sk starts with this; queries of the voter partition
/// filter on it so the [`VOTER_TALLY_SK`] row isn't read back as a vote.
pub const CANDIDATE_VOTE_SK_PREFIX: &str = "QC#";

/// The voter's per-cycle `VoterTally` row (§2.7a), in the same partition as
/// their votes. Its `votes_cast` counter enforces `votesPerUserPerCycle`
/// inside the vote transactions (§4 #1, #2).
pub const VOTER_TALLY_SK: &str = "TALLY";

// ---------- Locked question (§2.8) ----------

pub fn locked_pk(group_id: &GroupId, cycle_id: &CycleId) -> String {
    format!("GROUP#{}#NL#{}", group_id, cycle_id)
}

pub fn locked_sk(question_id: &QuestionId) -> String {
    format!("Q#{}", question_id)
}

// ---------- Response (§2.9) ----------

pub fn response_pk(group_id: &GroupId, cycle_id: &CycleId, question_id: &QuestionId) -> String {
    format!("GROUP#{}#NL#{}#Q#{}", group_id, cycle_id, question_id)
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

/// `gsi2pk` for a published text answer's most-reacted ranking (§2.9). Set
/// the first time the answer gets a counted reaction. Distinct from the
/// cycle-tick's `gsi2pk = NL_STATUS#…` values, so these rows never show up in
/// its due-cycle queries (§9).
pub fn reacted_gsi2pk(group_id: &GroupId) -> String {
    format!("GROUP#{}#REACTED", group_id)
}

/// `{reaction_count zero-padded}#{response_id}`, padded like
/// [`candidate_gsi1sk`] so lexicographic sort matches numeric. Rewritten with
/// every `reaction_count` change (§4 #8).
pub fn reacted_gsi2sk(reaction_count: u32, response_id: &ResponseId) -> String {
    format!(
        "{:0>width$}#{}",
        reaction_count,
        response_id,
        width = VOTE_COUNT_PAD_WIDTH
    )
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

pub fn image_gsi1sk(uploaded_at: Timestamp, image_id: &ImageId) -> String {
    format!("{}#{image_id}", key_timestamp(uploaded_at))
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

/// Every `Comment` sk starts with this; queries of an engagement partition
/// filter on it to skip reaction rows.
pub const COMMENT_SK_PREFIX: &str = "C#";

pub fn comment_sk(created_at: Timestamp, comment_id: &domain::CommentId) -> String {
    format!(
        "{COMMENT_SK_PREFIX}{}#{}",
        key_timestamp(created_at),
        comment_id
    )
}

/// Every `Reaction` sk starts with this; mirrors [`COMMENT_SK_PREFIX`] for
/// callers that split a whole engagement partition by row type.
pub const REACTION_SK_PREFIX: &str = "R#";

pub fn reaction_sk(reactor_user_id: &UserId, emoji: &str) -> String {
    format!("{REACTION_SK_PREFIX}{}#{}", reactor_user_id, emoji)
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
pub const NOTIFIED_PUBLISH_SK: &str = "NOTIFIED#PUBLISH";

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

// ---------- MemberStats (§2.17) ----------

pub fn member_stats_pk(group_id: &GroupId) -> String {
    format!("GROUP#{}#STATS", group_id)
}

pub fn member_stats_sk(user_id: &UserId) -> String {
    format!("MEMBER#{}", user_id)
}

/// Every `MemberStats` sk starts with this; AP23's query filters on it.
pub const MEMBER_STATS_SK_PREFIX: &str = "MEMBER#";

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

    fn ts(s: &str) -> Timestamp {
        s.parse().expect("valid test timestamp")
    }

    fn uid() -> UserId {
        UserId::new("01HX1")
    }
    fn gid() -> GroupId {
        GroupId::new("01HG2")
    }
    fn cid() -> CycleId {
        CycleId::new("202606")
    }
    fn qid() -> QuestionId {
        QuestionId::new("01HQ3")
    }
    fn iid() -> ImageId {
        ImageId::new("01HI4")
    }

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
    fn group_awaiting_first_cycle_keys() {
        assert_eq!(
            GROUP_AWAITING_FIRST_CYCLE_GSI2PK,
            "GROUP_STATUS#awaiting_first_cycle"
        );
        let t = ts("2026-01-01T00:00:00Z");
        assert_eq!(
            group_awaiting_first_cycle_gsi2sk(t, &gid()),
            "2026-01-01T00:00:00Z#01HG2"
        );
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
        assert_eq!(
            newsletter_gsi2pk(NewsletterStatus::Voting),
            "NL_STATUS#voting"
        );
        assert_eq!(newsletter_gsi2pk(NewsletterStatus::Open), "NL_STATUS#open");
        assert_eq!(
            newsletter_gsi2pk(NewsletterStatus::Published),
            "NL_STATUS#published"
        );
        let t = ts("2026-06-01T00:00:00Z");
        assert_eq!(
            newsletter_gsi2sk(t, &gid(), &cid()),
            "2026-06-01T00:00:00Z#01HG2#202606"
        );
        assert_eq!(
            newsletter_gsi2sk_sentinel(&gid(), &cid()),
            "9999-12-31T00:00:00Z#01HG2#202606"
        );
    }

    #[test]
    fn candidate_keys_and_padding() {
        assert_eq!(candidate_pk(&gid(), &cid()), "GROUP#01HG2#CYCLE#202606");
        assert_eq!(candidate_sk(&qid()), "QC#01HQ3");
        assert_eq!(
            candidate_gsi1pk(&gid(), &cid()),
            "GROUP#01HG2#CYCLE#202606#VOTES"
        );
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
    fn reacted_gsi2_keys_and_padding() {
        let rid = ResponseId::new("01HR9");
        assert_eq!(reacted_gsi2pk(&gid()), "GROUP#01HG2#REACTED");
        assert_eq!(reacted_gsi2sk(7, &rid), "000007#01HR9");
        assert_eq!(reacted_gsi2sk(0, &rid), "000000#01HR9");
        assert!(reacted_gsi2sk(9, &rid) < reacted_gsi2sk(10, &rid));
        assert!(reacted_gsi2sk(99, &rid) < reacted_gsi2sk(100, &rid));
    }

    #[test]
    fn image_keys() {
        assert_eq!(image_pk(&gid(), &cid()), "GROUP#01HG2#NL#202606");
        assert_eq!(image_sk(&iid()), "IMG#01HI4");
        assert_eq!(image_gsi1pk(&uid()), "USER#01HX1#IMG");
        let t = ts("2026-06-01T00:00:00Z");
        assert_eq!(image_gsi1sk(t, &iid()), "2026-06-01T00:00:00Z#01HI4");
    }

    #[test]
    fn engagement_keys() {
        let answer_user = UserId::new("01HU5");
        assert_eq!(
            engagement_pk(&gid(), &cid(), &qid(), &answer_user),
            "GROUP#01HG2#NL#202606#Q#01HQ3#A#01HU5"
        );
        let cmt = CommentId::new("01HC6");
        let t = ts("2026-06-04T12:00:00Z");
        assert_eq!(comment_sk(t, &cmt), "C#2026-06-04T12:00:00Z#01HC6");
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
        assert_eq!(NOTIFIED_PUBLISH_SK, "NOTIFIED#PUBLISH");
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

    #[test]
    fn member_stats_keys() {
        assert_eq!(member_stats_pk(&gid()), "GROUP#01HG2#STATS");
        assert_eq!(member_stats_sk(&uid()), "MEMBER#01HX1");
        assert_eq!(MEMBER_STATS_SK_PREFIX, "MEMBER#");
    }

    #[test]
    fn key_timestamp_is_fixed_width_with_no_fraction() {
        // Whole-second input: no fractional part, `Z` suffix, exactly 20 bytes.
        let whole = ts("2026-10-01T04:00:00Z");
        let formatted = key_timestamp(whole);
        assert_eq!(formatted, "2026-10-01T04:00:00Z");
        assert_eq!(formatted.len(), 20);

        // Sub-second input: nanos are truncated, not rounded, and the format
        // stays fixed-width — this is the exact input shape `Timestamp::now()`
        // produces that `x.to_string()` would otherwise render with a
        // fractional part.
        let with_nanos = ts("2026-10-01T04:00:00.123456789Z");
        let formatted_nanos = key_timestamp(with_nanos);
        assert_eq!(formatted_nanos, "2026-10-01T04:00:00Z");
        assert_eq!(formatted_nanos.len(), 20);

        // Sanity: `x.to_string()` is the thing we're avoiding — it would have
        // produced a longer, fractional-suffixed string here.
        assert_ne!(with_nanos.to_string(), formatted_nanos);
    }

    #[test]
    fn key_timestamp_with_nanos_sorts_correctly_against_a_same_second_bound() {
        // A `now` with a non-zero fraction, and a query upper bound computed
        // from the same whole second (as `list_cycles_due` does by truncating
        // `now` before formatting). The two must compare equal once both go
        // through `key_timestamp` — with the raw `x.to_string()` form, the
        // fractional part would make `now` sort *after* the truncated bound
        // and the due-row would be missed.
        let now = ts("2026-10-01T04:00:00.5Z");
        let bound = key_timestamp(now);
        let gsi2sk = newsletter_gsi2sk(now, &gid(), &cid());
        assert!(gsi2sk.as_str() <= format!("{bound}#~~~").as_str());

        // And the naive `x.to_string()` form of the same instant does NOT
        // sort consistently at the same second — demonstrating the bug this
        // fix closes.
        let naive_bound = now.to_string();
        assert_ne!(naive_bound, bound);
    }
}
