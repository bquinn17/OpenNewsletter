//! Tunable defaults referenced across crates. Single point of edit.

pub const DEFAULT_QUESTIONS_PER_CYCLE: u32 = 5;
pub const DEFAULT_VOTES_PER_USER_PER_CYCLE: u32 = 3;
pub const DEFAULT_RESPONSE_WINDOW_DAYS: u32 = 4;
pub const DEFAULT_MEMBER_SOFT_CAP: u32 = 50;
pub const DEFAULT_TIMEZONE: &str = "America/New_York";
pub const DEFAULT_NOTIFY_OFFSETS_HOURS: &[u32] = &[96, 48, 24];

/// The notify-tick's query window is derived from this cap, so an offset above it
/// would silently never fire (`plans/07-notifications.md` §7.1).
pub const MAX_REMINDER_OFFSET_HOURS: u32 = 168;

/// Group gradient palette slugs. Mirrors the enum in `shared/openapi.yaml`
/// and the frontend palette table (`plans/03-api-contract.md` §4.3).
pub const GRADIENT_SLUGS: &[&str] = &[
    "grape-sky",
    "ember-rose",
    "forest-mint",
    "ocean-dusk",
    "citrus-blush",
    "slate-lilac",
];

/// Avatar fallback colour slugs (`plans/03-api-contract.md` §2.3).
pub const AVATAR_COLOR_SLUGS: &[&str] = &[
    "red", "orange", "amber", "green", "teal", "blue", "violet", "pink",
];

/// Pick a member's default avatar colour from their user ID.
///
/// Deterministic so the same person always renders in the same colour, and so
/// `scripts/bootstrap_admin.py` can reproduce it without calling the API.
pub fn derive_avatar_color(user_id: &str) -> &'static str {
    let sum: u32 = user_id.bytes().map(u32::from).sum();
    AVATAR_COLOR_SLUGS[sum as usize % AVATAR_COLOR_SLUGS.len()]
}

pub const MIN_QUESTIONS_PER_CYCLE: u32 = 1;
pub const MAX_QUESTIONS_PER_CYCLE: u32 = 20;
pub const MIN_RESPONSE_WINDOW_DAYS: u32 = 1;
pub const MAX_RESPONSE_WINDOW_DAYS: u32 = 28;

pub const MAX_PROMPT_CHARS: usize = 500;
pub const MIN_PROMPT_CHARS: usize = 5;
pub const MAX_RESPONSE_BODY_CHARS: usize = 20_000;
pub const MAX_IMAGES_PER_RESPONSE: usize = 10;
pub const MAX_COMMENT_BODY_CHARS: usize = 2_000;
pub const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;
pub const MAX_AVATAR_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_DISPLAY_NAME_CHARS: usize = 40;
pub const MAX_IMAGE_CAPTION_CHARS: usize = 140;

pub const MAX_POLL_OPTIONS: usize = 6;
pub const MIN_POLL_OPTIONS: usize = 2;
pub const MAX_POLL_OPTION_LABEL_CHARS: usize = 80;

pub const INVITE_CODE_LEN: usize = 16;
pub const INVITE_DEFAULT_TTL_DAYS: u32 = 7;
/// Upper bound on an admin-supplied `ttlDays`, so date arithmetic at the API
/// boundary can't be driven out of range.
pub const INVITE_MAX_TTL_DAYS: u32 = 90;
pub const INVITE_AUDIT_RETENTION_DAYS: i64 = 30;

pub const MEMBERSHIP_CACHE_TTL_SECONDS: u64 = 60;

pub const VOTE_COUNT_PAD_WIDTH: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_colour_is_stable_for_the_same_user() {
        assert_eq!(
            derive_avatar_color("01HX7Y8Z9ABCDEF"),
            derive_avatar_color("01HX7Y8Z9ABCDEF")
        );
    }

    #[test]
    fn avatar_colour_is_always_a_known_slug() {
        for id in ["", "a", "01HX7Y8Z9ABCDEF", &"z".repeat(64)] {
            assert!(AVATAR_COLOR_SLUGS.contains(&derive_avatar_color(id)));
        }
    }
}
