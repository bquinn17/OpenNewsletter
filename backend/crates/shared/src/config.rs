//! Tunable defaults referenced across crates. Single point of edit.

pub const DEFAULT_QUESTIONS_PER_CYCLE: u32 = 5;
pub const DEFAULT_VOTES_PER_USER_PER_CYCLE: u32 = 3;
pub const DEFAULT_RESPONSE_WINDOW_DAYS: u32 = 4;
pub const DEFAULT_MEMBER_SOFT_CAP: u32 = 50;
pub const DEFAULT_TIMEZONE: &str = "America/New_York";
pub const DEFAULT_NOTIFY_OFFSETS_HOURS: &[u32] = &[96, 48, 24];

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
pub const INVITE_AUDIT_RETENTION_DAYS: i64 = 30;

pub const MEMBERSHIP_CACHE_TTL_SECONDS: u64 = 60;

pub const VOTE_COUNT_PAD_WIDTH: usize = 6;
