//! Pure helpers shared by `lambda-engagement` and `lambda-newsletters`
//! (`plans/09-engagement.md` §2.2, §2.3; `plans/03-api-contract.md` §8.5-§8.7).
//! No I/O — callers own fetching the data and mapping failures to `ApiError`.
//!
//! **Emoji predicate note:** `09-engagement.md` §2.2 specifies `Extended_Pictographic`
//! (a UTS #51 property used for emoji segmentation). No small, actively maintained
//! crate exposes that exact property as a public API (the full Unicode Character
//! Database property set lives in `icu_properties`, which is Unicode-3.0 licensed —
//! not on the MIT/Apache-2.0/BSD list `coding-standards.md` §2.10 requires — and pulls
//! in a much larger dependency tree than this one predicate needs). `unicode-properties`
//! (part of the `unicode-rs` org, MIT/Apache-2.0, already a dependency for
//! `unicode-normalization` below) exposes the related `Emoji` (`Emoji=Yes`) property
//! instead. `Extended_Pictographic` and `Emoji=Yes` agree on every codepoint except one
//! well-known, closed, 12-character set: the ASCII "keycap bases" `#`, `*`, and `0`-`9`,
//! which are `Emoji=Yes` (they're valid emoji once followed by U+20E3 COMBINING
//! ENCLOSING KEYCAP) but not `Extended_Pictographic`. [`is_valid_emoji`] excludes that
//! fixed set explicitly, which makes the two properties equivalent for every input this
//! predicate sees. See the test module for the matrix this satisfies.

use crate::api::ReactionGroupResponse;
use crate::{Reaction, UserId};
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;
use unicode_properties::emoji::UnicodeEmoji;

/// NFC-normalizes an emoji string, per `09-engagement.md` §2.2. The normalized
/// form is what gets stored and compared.
pub fn normalize_emoji(raw: &str) -> String {
    raw.nfc().collect()
}

/// `Emoji=Yes` codepoints that are not `Extended_Pictographic` — see the
/// module-level note. Fixed by the Unicode Character Database; never changes.
const EMOJI_KEYCAP_BASES: [char; 12] = ['#', '*', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// True when `normalized` is 1-12 codepoints with at least one codepoint that
/// is `Extended_Pictographic` (approximated as `Emoji=Yes` minus the ASCII
/// keycap bases — see the module-level note) or a regional-indicator
/// (U+1F1E6-U+1F1FF, for flag sequences). Takes the already NFC-normalized
/// string; callers normalize first via [`normalize_emoji`].
pub fn is_valid_emoji(normalized: &str) -> bool {
    let codepoints: Vec<char> = normalized.chars().collect();
    if codepoints.is_empty() || codepoints.len() > 12 {
        return false;
    }
    codepoints.iter().any(|c| {
        (c.is_emoji_char() && !EMOJI_KEYCAP_BASES.contains(c))
            || unicode_properties::emoji::is_regional_indicator(*c)
    })
}

/// Groups raw reactions by emoji for the `ReactionsResponse`/`PublishedAnswerResponse`
/// shape (`09-engagement.md` §2.3, decided M10): count descending, then earliest
/// reaction first, then emoji — the one shared ordering used by both callers.
pub fn group_reactions(reactions: &[Reaction], caller: &UserId) -> Vec<ReactionGroupResponse> {
    struct Group {
        count: u32,
        earliest: chrono::DateTime<chrono::Utc>,
        reacted_by_me: bool,
    }

    let mut groups: HashMap<&str, Group> = HashMap::new();
    for r in reactions {
        let entry = groups.entry(r.emoji.as_str()).or_insert(Group {
            count: 0,
            earliest: r.created_at,
            reacted_by_me: false,
        });
        entry.count += 1;
        if r.created_at < entry.earliest {
            entry.earliest = r.created_at;
        }
        if r.reactor_user_id == *caller {
            entry.reacted_by_me = true;
        }
    }

    let mut out: Vec<(&str, Group)> = groups.into_iter().collect();
    out.sort_by(|(a_emoji, a), (b_emoji, b)| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.earliest.cmp(&b.earliest))
            .then_with(|| a_emoji.cmp(b_emoji))
    });

    out.into_iter()
        .map(|(emoji, g)| ReactionGroupResponse {
            emoji: emoji.to_owned(),
            count: g.count,
            reacted_by_me: g.reacted_by_me,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CycleId, GroupId, QuestionId};
    use chrono::{TimeZone, Utc};

    #[test]
    fn single_codepoint_emoji_is_valid() {
        assert!(is_valid_emoji("🔥"));
    }

    #[test]
    fn zwj_family_sequence_is_valid() {
        // 👨‍👩‍👧‍👦 — man, ZWJ, woman, ZWJ, girl, ZWJ, boy = 7 codepoints.
        let family = "👨‍👩‍👧‍👦";
        assert_eq!(family.chars().count(), 7);
        assert!(is_valid_emoji(family));
    }

    #[test]
    fn regional_indicator_flag_is_valid() {
        // 🇳🇿 — two regional-indicator codepoints, neither of which is
        // Extended_Pictographic on its own.
        let flag = "🇳🇿";
        assert_eq!(flag.chars().count(), 2);
        assert!(is_valid_emoji(flag));
    }

    #[test]
    fn skin_tone_modifier_sequence_is_valid() {
        // 👍🏽 — thumbs up + Fitzpatrick type-4 modifier.
        let toned = "👍🏽";
        assert!(is_valid_emoji(toned));
    }

    #[test]
    fn plain_ascii_is_rejected() {
        assert!(!is_valid_emoji("a"));
        assert!(!is_valid_emoji("GG"));
    }

    #[test]
    fn keycap_less_ascii_digit_is_rejected() {
        // A bare digit/`#`/`*` is `Emoji=Yes` (it's a valid keycap *base*) but
        // not `Extended_Pictographic` — must still be rejected without the
        // combining keycap character attached (`09-engagement.md` §2.2).
        assert!(!is_valid_emoji("1"));
        assert!(!is_valid_emoji("#"));
        assert!(!is_valid_emoji("*"));
    }

    #[test]
    fn empty_string_is_rejected() {
        assert!(!is_valid_emoji(""));
    }

    #[test]
    fn over_twelve_codepoints_is_rejected() {
        let too_long: String = "🔥".repeat(13);
        assert_eq!(too_long.chars().count(), 13);
        assert!(!is_valid_emoji(&too_long));
    }

    #[test]
    fn exactly_twelve_codepoints_is_accepted() {
        let max_len: String = "🔥".repeat(12);
        assert_eq!(max_len.chars().count(), 12);
        assert!(is_valid_emoji(&max_len));
    }

    #[test]
    fn normalize_emoji_applies_nfc() {
        // é as "e" + combining acute (NFD) normalizes to the single precomposed
        // codepoint (NFC) — not an emoji either way, but exercises the function
        // the emoji path shares.
        let nfd = "e\u{0301}";
        let nfc = normalize_emoji(nfd);
        assert_eq!(nfc.chars().count(), 1);
    }

    fn reaction(emoji: &str, reactor: &str, created_at: chrono::DateTime<chrono::Utc>) -> Reaction {
        Reaction {
            group_id: GroupId::new("g1"),
            cycle_id: CycleId::new("202606"),
            question_id: QuestionId::new("q1"),
            answer_user_id: UserId::new("answerer"),
            reactor_user_id: UserId::new(reactor),
            emoji: emoji.to_owned(),
            created_at,
        }
    }

    #[test]
    fn groups_order_by_count_descending() {
        let t = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let reactions = vec![
            reaction("🔥", "u1", t),
            reaction("🔥", "u2", t),
            reaction("🤣", "u3", t),
        ];
        let groups = group_reactions(&reactions, &UserId::new("u1"));
        assert_eq!(groups[0].emoji, "🔥");
        assert_eq!(groups[0].count, 2);
        assert!(groups[0].reacted_by_me);
        assert_eq!(groups[1].emoji, "🤣");
        assert_eq!(groups[1].count, 1);
        assert!(!groups[1].reacted_by_me);
    }

    #[test]
    fn ties_break_by_earliest_reaction_then_emoji() {
        let earlier = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 6, 1, 1, 0, 0).unwrap();
        // Both emoji end up with count 1; 🤣 was reacted first.
        let reactions = vec![reaction("🔥", "u1", later), reaction("🤣", "u2", earlier)];
        let groups = group_reactions(&reactions, &UserId::new("nobody"));
        assert_eq!(
            groups.iter().map(|g| g.emoji.as_str()).collect::<Vec<_>>(),
            vec!["🤣", "🔥"]
        );
    }

    #[test]
    fn full_tie_breaks_by_emoji_string() {
        let t = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let reactions = vec![reaction("🤣", "u1", t), reaction("🔥", "u2", t)];
        let groups = group_reactions(&reactions, &UserId::new("nobody"));
        assert_eq!(
            groups.iter().map(|g| g.emoji.as_str()).collect::<Vec<_>>(),
            vec!["🔥", "🤣"]
        );
    }
}
