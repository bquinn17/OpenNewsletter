//! Tunable defaults referenced across crates. Single point of edit.

pub const DEFAULT_QUESTIONS_PER_CYCLE: u32 = 5;
pub const DEFAULT_VOTES_PER_USER_PER_CYCLE: u32 = 3;
pub const DEFAULT_RESPONSE_WINDOW_DAYS: u32 = 7;
pub const DEFAULT_MEMBER_SOFT_CAP: u32 = 50;
pub const DEFAULT_TIMEZONE: &str = "America/New_York";
pub const DEFAULT_NOTIFY_OFFSETS_HOURS: &[u32] = &[96, 48, 24];

/// The notify-tick's query window is derived from this cap, so an offset above it
/// would silently never fire (`docs/design/notifications.md` §7.1).
pub const MAX_REMINDER_OFFSET_HOURS: u32 = 168;

/// Group gradient palette slugs. Mirrors the enum in `shared/openapi.yaml`
/// and the frontend palette table (`docs/design/api.md` §4.3).
pub const GRADIENT_SLUGS: &[&str] = &[
    "grape-sky",
    "ember-rose",
    "forest-mint",
    "ocean-dusk",
    "citrus-blush",
    "slate-lilac",
];

/// Avatar fallback colour slugs (`docs/design/api.md` §2.3).
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

/// Close-time auto-publish (`docs/design/lifecycle.md` §5.3): after
/// this many cycle ticks whose sweep left some draft unpublished by an error,
/// the cycle publishes anyway without those drafts. The tick runs every 5
/// minutes, so a broken draft holds its group's edition back about 15 minutes.
pub const AUTO_PUBLISH_MAX_FAILED_SWEEPS: u32 = 3;

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

/// `lambda-image-process` variant dimensions and WebP quality
/// (`docs/design/media.md` §1, §5.3, §11.1).
pub const DISPLAY_LONG_EDGE_PX: u32 = 1200;
pub const DISPLAY_QUALITY: f32 = 82.0;
pub const THUMB_LONG_EDGE_PX: u32 = 400;
pub const THUMB_QUALITY: f32 = 75.0;
pub const AVATAR_SIZE_PX: u32 = 256;
pub const AVATAR_QUALITY: f32 = 82.0;

/// `lambda-image-process` decode budget, checked against the header before any
/// pixel buffer is allocated. A 15 MB upload can declare a canvas that decodes
/// to gigabytes and kill the 1024 MB Lambda mid-run, which would leave its row
/// `pending` forever. 256 MB of decoded pixels covers a 48 MP RGB photo and
/// leaves room for the rotated copy EXIF orientation makes.
pub const MAX_IMAGE_EDGE_PX: u32 = 16_384;
pub const MAX_DECODED_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
/// GIF frames are decoded at full canvas size, once per variant; these bound the
/// memory and CPU a single animated upload can take within the 60s timeout.
pub const MAX_GIF_FRAMES: usize = 300;
pub const MAX_GIF_TOTAL_PIXELS: u64 = 100_000_000;
pub const MAX_DISPLAY_NAME_CHARS: usize = 40;
pub const MAX_IMAGE_CAPTION_CHARS: usize = 140;

pub const MAX_POLL_OPTIONS: usize = 6;
pub const MIN_POLL_OPTIONS: usize = 2;
pub const MAX_POLL_OPTION_LABEL_CHARS: usize = 80;

/// Debounce for the response-draft autosave, enforced client-side; echoed to
/// the frontend via `GET /config` (`docs/design/api.md` §2).
pub const AUTOSAVE_DEBOUNCE_MS: u32 = 1500;

pub const INVITE_CODE_LEN: usize = 16;
pub const INVITE_DEFAULT_TTL_DAYS: u32 = 7;
/// Upper bound on an admin-supplied `ttlDays`, so date arithmetic at the API
/// boundary can't be driven out of range.
pub const INVITE_MAX_TTL_DAYS: u32 = 90;
pub const INVITE_AUDIT_RETENTION_DAYS: i64 = 30;
/// RFC 5321's limit on a forward path; caps `POST /admin/invites`'s `boundEmail`.
pub const MAX_BOUND_EMAIL_CHARS: usize = 254;

pub const MEMBERSHIP_CACHE_TTL_SECONDS: u64 = 60;

pub const VOTE_COUNT_PAD_WIDTH: usize = 6;

/// Default page size for list endpoints when `?limit=` is omitted
/// (`docs/design/api.md` §1).
pub const DEFAULT_LIST_LIMIT: u32 = 20;
/// General upper bound on `?limit=` per `docs/design/api.md` §1.
pub const MAX_LIST_LIMIT: u32 = 100;
/// `GET /groups/{groupId}/newsletters` computes `myDraftCount`/`myPublishedCount`
/// with one extra query per cycle returned, so its page size is capped tighter
/// than the general list limit (`docs/design/api.md` §5.1).
pub const MAX_NEWSLETTER_LIST_LIMIT: u32 = 24;
/// Default page size for `GET .../comments` when `?limit=` is omitted;
/// capped at [`MAX_LIST_LIMIT`] (`docs/design/api.md` §8.1,
/// `docs/design/engagement.md` §8 #9).
pub const DEFAULT_COMMENT_LIST_LIMIT: u32 = 50;
/// How many times a comment edit/delete re-reads and retries after its
/// transaction lost a race to a concurrent image change on the same comment
/// (`docs/design/engagement.md` §1.4, decided M10).
pub const COMMENT_WRITE_ATTEMPTS: u32 = 3;

/// A push subscription is deleted after this many consecutive delivery
/// failures (`docs/design/notifications.md` §3.2).
pub const MAX_PUSH_FAILURES: u32 = 5;
/// A user keeps at most this many push subscriptions; adding one more evicts
/// the least-recently-successful (`docs/design/api.md` §10.1).
pub const MAX_PUSH_SUBSCRIPTIONS_PER_USER: usize = 10;
/// `TTL` header sent with every Web Push request — 24h, so a push service
/// doesn't deliver a stale notification to a reopened app (`docs/design/notifications.md` §10).
pub const PUSH_TTL_SECONDS: u64 = 86_400;
/// Per-send HTTP timeout for the Web Push request itself.
pub const PUSH_SEND_TIMEOUT_SECS: u64 = 10;
/// How many Web Push requests one fan-out keeps in flight at once
/// (`docs/design/notifications.md` §6.2, decided M11 review). A dead endpoint costs
/// one slot for up to `PUSH_SEND_TIMEOUT_SECS` instead of stalling every
/// member queued behind it.
pub const PUSH_SEND_CONCURRENCY: usize = 16;
/// How many per-answer engagement reads (`GET .../newsletters/{c}` on a
/// published edition) one request keeps in flight at once.
pub const EDITION_READ_CONCURRENCY: usize = 16;
/// How many members' preference/subscription reads a fan-out (`lambda-push`
/// and `lambda-notify-tick`) keeps in flight at once, ahead of the send step.
pub const PUSH_MEMBER_READ_CONCURRENCY: usize = 16;
/// Send cutoffs: no *new* send starts once this many seconds have passed
/// since the work began; sends already in flight run to completion. Each
/// cutoff leaves `PUSH_SEND_TIMEOUT_SECS` plus ~5s of bookkeeping headroom
/// under the hard limit it protects, so a send is never killed mid-flight.
///
/// Cycle-open / publication fan-out — `push-api` Lambda timeout is 60s.
pub const PUSH_FANOUT_SEND_CUTOFF_SECS: u64 = 45;
/// `POST /push/test` — API Gateway's integration timeout is 30s.
pub const PUSH_TEST_SEND_CUTOFF_SECS: u64 = 15;
/// Deadline-reminder sends within one notify tick — `notify-tick` Lambda
/// timeout is 120s.
pub const NOTIFY_TICK_SEND_CUTOFF_SECS: u64 = 105;
/// A notify tick claims no further cycles after this many seconds; the
/// rest stay unclaimed and are picked up by the next tick 15 minutes later
/// rather than being claimed and then cut off.
pub const NOTIFY_TICK_CYCLE_CUTOFF_SECS: u64 = 60;
/// Push payload JSON must serialize to at most this many bytes before
/// encryption (`docs/design/notifications.md` §5, M11 decision D6).
pub const MAX_PUSH_PAYLOAD_BYTES: usize = 3_000;

/// Known Web Push service hosts a subscription `endpoint` is allowed to
/// target (`docs/design/api.md` §10.1, M13 hardening). Each entry is
/// either an exact host or, when prefixed with `.`, a suffix match against
/// the parsed URL host — never a substring match against the raw string,
/// which userinfo tricks like `https://fcm.googleapis.com@evil.com/` could
/// otherwise defeat.
pub const PUSH_ENDPOINT_HOST_SUFFIXES: &[&str] = &[
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
    ".push.services.mozilla.com",
    "web.push.apple.com",
    ".notify.windows.com",
    ".push.apple.com",
];

/// Default group settings served verbatim by `GET /config`
/// (`docs/design/api.md` §2, `docs/design/infrastructure.md` §6.4).
/// Single source of truth so the CDK stack no longer hand-copies these.
pub fn default_group_settings() -> serde_json::Value {
    serde_json::json!({
        "questionsPerCycle": DEFAULT_QUESTIONS_PER_CYCLE,
        "votesPerUserPerCycle": DEFAULT_VOTES_PER_USER_PER_CYCLE,
        "responseWindowDays": DEFAULT_RESPONSE_WINDOW_DAYS,
        "timezone": DEFAULT_TIMEZONE,
        "memberSoftCap": DEFAULT_MEMBER_SOFT_CAP,
        "imagesPerAnswerMax": MAX_IMAGES_PER_RESPONSE,
        "imageMaxBytes": MAX_IMAGE_BYTES,
        "inviteTtlDays": INVITE_DEFAULT_TTL_DAYS,
        "autosaveDebounceMs": AUTOSAVE_DEBOUNCE_MS,
        "notificationOffsetsHoursBeforeClose": DEFAULT_NOTIFY_OFFSETS_HOURS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_group_settings_has_the_keys_get_config_contracts_on() {
        let settings = default_group_settings();
        let mut keys: Vec<&str> = settings
            .as_object()
            .expect("defaults are a JSON object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "autosaveDebounceMs",
                "imageMaxBytes",
                "imagesPerAnswerMax",
                "inviteTtlDays",
                "memberSoftCap",
                "notificationOffsetsHoursBeforeClose",
                "questionsPerCycle",
                "responseWindowDays",
                "timezone",
                "votesPerUserPerCycle",
            ]
        );
    }

    #[test]
    fn new_groups_default_to_a_seven_day_window() {
        assert_eq!(default_group_settings()["responseWindowDays"], 7);
    }

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
