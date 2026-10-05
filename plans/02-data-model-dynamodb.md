# 02 — Data Model (DynamoDB Single-Table)

This document defines every entity stored in the `OpenNewsletter-{env}` table. Read this in conjunction with `03-api-contract.md` (which references the access patterns by name) and `06-newsletter-lifecycle.md` (which references the cycle state transitions).

**Naming convention.** Every attribute name in this document — table headers, key formulas, transaction descriptions — is the literal DynamoDB item attribute name, and those are **snake_case** (`member_count`, `response_close_at`, `editions_answered`, …), matching the Rust entity structs in `backend/crates/domain/src/entities.rs`, which `persistence` serializes via `serde_dynamo::{to_item, from_item}` with no `rename_all` on struct fields. The HTTP/JSON API contract (`03-api-contract.md`, `shared/openapi.yaml`) is **camelCase**, as specified there. Lambda handlers map entities to DTOs explicitly at the boundary — there is no automatic camelCase ⇄ snake_case conversion, so a field renamed on one side does not silently follow on the other. (Reconciled 2026-09-27 — this doc previously documented item attributes in camelCase, a gap called out since M4.)

---

## 1. Table shape recap

- Hash key: `pk` (S)
- Range key: `sk` (S)
- TTL attribute: `ttl` (N, epoch seconds; only set on items that should auto-expire)
- Stream: **disabled in v1.** The only planned consumer is the archival Lambda (`10-archival.md`), which is deferred. Streams add cost with no v1 reader. Re-enable (`NEW_AND_OLD_IMAGES`) when archival lands; the table reconfigure is non-disruptive.

Two GSIs:

- **GSI1** — `gsi1pk` / `gsi1sk` — generic inversion ("show me all X for parent Y")
- **GSI2** — `gsi2pk` / `gsi2sk` — by-status / by-time ("show me cycles whose `next_transition_at` is within the next 5 minutes")

All keys are strings. All ID values are UUIDv7 unless noted.

**Key-timestamp format.** Any timestamp embedded in a key attribute (`pk`/`sk`/`gsi1*`/`gsi2*`) — or compared against one in a key-condition expression, such as the tick's `gsi2sk <= {nowISO}#~~~` bound — MUST use the fixed-width form `YYYY-MM-DDTHH:MM:SSZ` (whole seconds, `Z` suffix, no fractional part), produced by `persistence::keys::key_timestamp` (Rust: `DateTime::to_rfc3339_opts(SecondsFormat::Secs, true)`; Python: `dt.strftime("%Y-%m-%dT%H:%M:%SZ")`). Plain `DateTime::to_rfc3339()` is unsafe here: it emits `+00:00` instead of `Z` and a variable-width fractional part whenever the instant has non-zero nanoseconds, so two timestamps in the same second can sort inconsistently against each other. Elsewhere in this document, "ISO-8601" for a non-key application attribute (serialized via `serde_dynamo`) is unaffected by this rule and may keep the default `to_rfc3339()` form.

---

## 2. Entity catalog

For each entity below: `pk`, `sk`, optional GSI keys, the application attributes, and a brief explanation. `entity` attribute is set on every item for debugging (and for the future archival stream consumer — streams themselves are off in v1). All `Attr` values below are DynamoDB item attribute names (snake_case) — not to be confused with the camelCase JSON field names of the same entity in the HTTP contract (`03-api-contract.md`).

### 2.1 User

A Cognito-authenticated person.

| Attr | Value |
|---|---|
| `pk` | `USER#{user_id}` |
| `sk` | `PROFILE` |
| `entity` | `User` |
| `user_id` | UUIDv7 (generated server-side on first invite redemption) |
| `cognito_sub` | string (Cognito `sub` — UUIDv4-shaped) |
| `email` | string |
| `display_name` | string |
| `avatar_color` | string (slug, e.g. `grape`/`coral`/`mint`; deterministic fallback derived from `user_id` hash if unset) |
| `avatar_media_id` | string \| null (references an `AvatarMedia` row — see §2.16) |
| `created_at` | ISO-8601 |
| `last_login_at` | ISO-8601 |

`user_id` is our own UUIDv7, not the Cognito sub. This preserves the global "all IDs are UUIDv7, sortable by creation time" invariant. Map sub → user_id via the lookup row below.

### 2.1a Cognito-sub lookup

| Attr | Value |
|---|---|
| `pk` | `COGNITO_SUB#{sub}` |
| `sk` | `USER_ID` |
| `entity` | `CognitoSubLookup` |
| `user_id` | UUIDv7 |

Created in the same `TransactWriteItems` as the User row (during invite redemption). Read on every authenticated request to resolve `claims.sub` → our `user_id`. Cached per cold-start for ≤60s alongside the membership cache.

> **Note (reconciled 2026-09-27):** `persistence::invites::join_via_invite_tx` builds this item via `to_item(CognitoSubLookup { cognito_sub, user_id })` (`backend/crates/persistence/src/invites.rs`), so the item also carries a `cognito_sub` attribute (redundant with the value already encoded in `pk`) that this doc does not document. Flagging for the lead to decide whether to keep it (harmless denormalization, occasionally handy for a scan/debug) or trim the struct to `user_id` only.

### 2.2 GroupMembership (also serves as Group → user index via GSI1)

A user's membership in one group.

| Attr | Value |
|---|---|
| `pk` | `USER#{user_id}` |
| `sk` | `GROUP#{group_id}` |
| `gsi1pk` | `GROUP#{group_id}` |
| `gsi1sk` | `MEMBER#{user_id}` |
| `entity` | `GroupMembership` |
| `user_id` | UUIDv7 |
| `group_id` | UUIDv7 |
| `role` | `admin` \| `member` |
| `joined_at` | ISO-8601 |
| `editions_answered` | number (denormalized; incremented in the publish-response transaction the first time a user publishes any answer in a given cycle) |

Access patterns served:
- "What groups am I in?" → `Query pk = USER#{user_id} AND begins_with(sk, GROUP#)`
- "Who is in this group?" → `Query GSI1 gsi1pk = GROUP#{group_id} AND begins_with(gsi1sk, MEMBER#)`

### 2.3 Group

The tenant.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}` |
| `sk` | `META` |
| `entity` | `Group` |
| `group_id` | UUIDv7 |
| `name` | string |
| `timezone` | IANA TZ string, default `America/New_York` |
| `cycle_settings` | map: `{ questions_per_cycle, votes_per_user_per_cycle, response_window_days, auto_publish: true }` |
| `notification_settings` | map: `{ offsets_hours_before_close: [96, 48, 24], on_cycle_open: true }` (publication push is always-on with no toggle) |
| `member_count` | number (denormalized; updated transactionally on join/leave) |
| `member_soft_cap` | number, default 50 |
| `gradient` | string (slug from a small preset palette, e.g. `grape-sky`, `coral-sun`; chosen at group creation, admin-editable in Settings) |
| `created_at` | ISO-8601 |
| `created_by` | user_id |

### 2.4 Invite

| Attr | Value |
|---|---|
| `pk` | `INVITE#{code}` |
| `sk` | `META` |
| `gsi1pk` | `GROUP#{group_id}` |
| `gsi1sk` | `INVITE#{code}` |
| `entity` | `Invite` |
| `code` | string (16-char base32 crockford) |
| `group_id` | UUIDv7 |
| `created_by` | user_id (admin) |
| `created_at` | ISO-8601 |
| `expires_at` | ISO-8601 |
| `ttl` | epoch seconds (= `expires_at` + 30 days, so the audit log lingers a month after expiry then auto-purges) |
| `status` | `pending` \| `consumed` \| `revoked` |
| `consumed_by` | user_id \| null |
| `consumed_at` | ISO-8601 \| null |
| `role_on_redeem` | `admin` \| `member` (defaults to `member`; admins can issue admin invites) |

Access patterns:
- "Validate invite by code" → `GetItem pk=INVITE#{code} sk=META`
- "List invites for group" → `Query GSI1 gsi1pk=GROUP#{group_id} AND begins_with(gsi1sk, INVITE#)`

### 2.5 Newsletter (Cycle)

The unit of an edition for one group.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}` |
| `sk` | `NL#{yyyymm}` |
| `gsi2pk` | `NL_STATUS#{status}` |
| `gsi2sk` | `{next_transition_at}#{group_id}#{yyyymm}` — `next_transition_at` in the canonical key-timestamp format (§1), or the sentinel `9999-12-31T00:00:00Z` in a terminal state |
| `entity` | `Newsletter` |
| `group_id` | UUIDv7 |
| `cycle_id` | string `yyyymm` (e.g. `202605`) |
| `status` | `voting` \| `open` \| `published` \| `archived` |
| `vote_window_open_at` | ISO-8601 (typically the publication of the previous cycle) |
| `vote_window_close_at` | ISO-8601 (= `response_open_at`) |
| `response_open_at` | ISO-8601 |
| `response_close_at` | ISO-8601 |
| `published_at` | ISO-8601 \| null |
| `next_transition_at` | ISO-8601 — the next instant any state change is due. Drives `lambda-cycle-tick`. |
| `locked_question_ids` | array<string> (set when transitioning `voting -> open`) |
| `notified_offsets_hours` | array<number> (records which reminder fan-outs have completed) |
| `notified_on_open` | bool |

State machine fully specified in `06-newsletter-lifecycle.md`.

Access patterns:
- "List newsletters for a group" → `Query pk=GROUP#{group_id} AND begins_with(sk, NL#)` (DESC by sk = most recent first)
- "Find a specific edition" → `GetItem pk=GROUP#{group_id} sk=NL#{yyyymm}`
- "Find all cycles whose next transition is due" → `Query GSI2 gsi2pk=NL_STATUS#{status} AND gsi2sk <= {nowKeyTimestamp}#~~~` for each non-terminal status, where `nowKeyTimestamp` is `now` formatted per the key-timestamp rule in §1 (truncated to whole seconds — a cycle due at exactly that second still matches, since the comparison is `<=`)

### 2.6 Candidate question (in voting pool)

Suggested by users for the next not-yet-opened cycle.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#CYCLE#{next_cycle_id}` |
| `sk` | `QC#{question_id}` |
| `gsi1pk` | `GROUP#{group_id}#CYCLE#{next_cycle_id}#VOTES` |
| `gsi1sk` | `{padded_vote_count}#{question_id}` (pad to 6 digits, e.g. `000007#01HX...`) |
| `entity` | `CandidateQuestion` |
| `question_id` | UUIDv7 |
| `group_id` | UUIDv7 |
| `next_cycle_id` | string `yyyymm` |
| `kind` | `text` \| `poll` |
| `prompt` | string (≤500 chars) |
| `poll_options` | array<{ option_id: UUIDv7, label: string }> \| null (only when `kind=poll`) |
| `vote_count` | number (denormalized; updated transactionally on vote add/remove) |
| `submitted_by` | user_id (always recorded; returned to non-admins ONLY when `is_anonymous=false`) |
| `is_anonymous` | boolean (submitter's choice at creation; immutable thereafter) |
| `submitted_at` | ISO-8601 |

GSI1 enables "leaderboard for current cycle" reads in O(votes) without a scan.

Access patterns:
- "List candidates for a cycle" → `Query pk=GROUP#{g}#CYCLE#{c} AND begins_with(sk, QC#)`
- "Top N candidates by votes" → `Query GSI1 gsi1pk=GROUP#{g}#CYCLE#{c}#VOTES` ScanIndexForward=false Limit=N

### 2.7 Candidate vote

Records that a user upvoted a candidate question.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#CYCLE#{next_cycle_id}#VOTER#{user_id}` |
| `sk` | `QC#{question_id}` |
| `entity` | `CandidateVote` |
| `user_id` | UUIDv7 |
| `question_id` | UUIDv7 |
| `voted_at` | ISO-8601 |

The `votes_per_user_per_cycle` cap is enforced inside the vote transactions by the voter's `VoterTally` row (§2.7a, §4 #1). The handler also pre-reads the voter partition, but only to build a helpful `VOTE_CAP_REACHED` detail; it isn't the guard.

> **Note (reconciled 2026-09-27):** the `CandidateVote` struct in `backend/crates/domain/src/entities.rs` also carries `group_id` and `cycle_id` fields, and `persistence::questions::cast_vote_tx`/`withdraw_vote_tx` serialize the whole struct via `to_item`, so the item also has `group_id` and `cycle_id` attributes (redundant with the values already encoded in `pk`) that this doc's table above did not list. Flagging for the lead — likely fine to keep (useful if this item is ever read outside its own partition) but worth a conscious decision either way.

Access patterns:
- "List my votes this cycle" → `Query pk=GROUP#{g}#CYCLE#{c}#VOTER#{u} AND begins_with(sk, QC#)` (the prefix skips the tally row)
- "Has user voted for this question?" → `GetItem pk=GROUP#{g}#CYCLE#{c}#VOTER#{u} sk=QC#{q}`

### 2.7a Voter tally

One per voter per cycle, in the same partition as their votes. Its counter is the transactional guard for `votes_per_user_per_cycle`. Added 2026-09-27: before that the cap was only a pre-read, so concurrent votes on different candidates could exceed it.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#CYCLE#{next_cycle_id}#VOTER#{user_id}` |
| `sk` | `TALLY` |
| `entity` | `VoterTally` |
| `votes_cast` | number; created by the first vote's `ADD` |

It is written only inside §4 #1/#2 and the admin candidate-delete cascade, so it always equals the number of `QC#` rows in its partition. A tally on a closed voting cycle is inert.

### 2.8 Locked question

A candidate that has been promoted into a specific newsletter.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#NL#{cycle_id}` |
| `sk` | `Q#{question_id}` |
| `entity` | `LockedQuestion` |
| `question_id` | UUIDv7 |
| `group_id` | UUIDv7 |
| `cycle_id` | string |
| `kind` | `text` \| `poll` |
| `prompt` | string |
| `poll_options` | array \| null |
| `display_order` | number (admin-controllable; default = vote rank when promoted) |
| `submitted_by` | user_id (copied from the candidate at promotion; returned to non-admins ONLY when `is_anonymous=false`) |
| `is_anonymous` | boolean (copied from the candidate at promotion; immutable) |
| `locked_at` | ISO-8601 |

Access patterns:
- "Show me this newsletter's questions in order" → `Query pk=GROUP#{g}#NL#{c} AND begins_with(sk, Q#)`, then sort by `display_order` client-side (or use a sort key that embeds display_order; simpler to sort in memory since N≤20)

### 2.9 Response (Answer)

A user's reply to one locked question.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#NL#{cycle_id}#Q#{question_id}` |
| `sk` | `A#{user_id}` |
| `gsi1pk` | `USER#{user_id}#NL#{cycle_id}` |
| `gsi1sk` | `Q#{question_id}` |
| `entity` | `Response` |
| `response_id` | UUIDv7 (used as a stable id for engagement targeting) |
| `user_id` | UUIDv7 |
| `question_id` | UUIDv7 |
| `group_id` | UUIDv7 |
| `cycle_id` | string |
| `kind` | `text` \| `poll` (mirrors question kind) |
| `status` | `draft` \| `published` |
| `body` | string (markdown-safe; ≤20k chars) — only when `kind=text` |
| `poll_option_id` | UUIDv7 \| null — only when `kind=poll` (last-write-wins) |
| `image_media_ids` | array<string> (≤10) — only when `kind=text` |
| `updated_at` | ISO-8601 |
| `published_at` | ISO-8601 \| null |

Access patterns:
- "List answers to a question (after publish)" → `Query pk=GROUP#{g}#NL#{c}#Q#{q} AND begins_with(sk, A#)`
- "List my drafts in this cycle" → `Query GSI1 gsi1pk=USER#{u}#NL#{c}`
- "Get my draft for a question" → `GetItem pk=GROUP#{g}#NL#{c}#Q#{q} sk=A#{u}`

### 2.10 ImageMedia

Each uploaded image. Originals lifecycle is owned by S3; the table tracks metadata + status.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#NL#{cycle_id}` |
| `sk` | `IMG#{image_id}` |
| `gsi1pk` | `USER#{user_id}#IMG` |
| `gsi1sk` | `{uploaded_at}#{image_id}` — `uploaded_at` in the canonical key-timestamp format (§1) |
| `entity` | `ImageMedia` |
| `image_id` | UUIDv7 |
| `user_id` | user_id (uploader) |
| `group_id` | UUIDv7 |
| `cycle_id` | string |
| `question_id` | string \| null (set when attached to a response; null on initial upload) |
| `purpose` | `response` \| `comment` — what the image is destined to attach to. Set at `POST /uploads` time (`03-api-contract.md` §9.1). The 10-images-per-answer cap counts `purpose=response` rows only; comment images are capped at one per comment at comment-create time. |
| `mime_type` | `image/jpeg` \| `image/png` \| `image/webp` \| `image/gif` |
| `original_key` | S3 key in originals bucket |
| `display_key` | S3 key in processed bucket (1200px WebP) — null until processed |
| `thumb_key` | S3 key in processed bucket (400px WebP) — null until processed |
| `status` | `pending` \| `ready` \| `failed` |
| `bytes` | number |
| `width` / `height` | numbers (from processor) |
| `caption` | string \| null (author-supplied, ≤140 chars; rendered beneath the image in the published view) |
| `uploaded_at` | ISO-8601 |
| `processed_at` | ISO-8601 \| null |
| `error_message` | string \| null — why `status=failed`: `IMAGE_TOO_LARGE`, `IMAGE_DECODE_FAILED` or `DELETED` (added M8; `08` §3.2, `03` §9.4) |

`question_id` is set at `POST /uploads` time (the route requires `questionId`), so in practice it is never null for rows created since M8.

### 2.11 Comment

Flat reply to a published response.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#NL#{cycle_id}#Q#{question_id}#A#{answer_user_id}` |
| `sk` | `C#{created_at}#{comment_id}` — `created_at` in the canonical key-timestamp format (§1) |
| `entity` | `Comment` |
| `comment_id` | UUIDv7 |
| `author_user_id` | UUIDv7 |
| `body` | string (≤2000 chars; markdown-safe) |
| `image_media_id` | string \| null (optional single image attached to the comment; references an `ImageMedia` row owned by the comment author) |
| `created_at` | ISO-8601 |
| `edited_at` | ISO-8601 \| null |
| `deleted_at` | ISO-8601 \| null (soft delete; `body` and `image_media_id` cleared when set) |

> **Note (reconciled 2026-09-27):** the `Comment` struct (`backend/crates/domain/src/entities.rs`) also carries `group_id`, `cycle_id`, `question_id`, and `answer_user_id` fields, and `persistence::engagement::put_comment` serializes the whole struct via `to_item`, so the item also has those four attributes (redundant with the values already encoded in `pk`) that this doc's table above did not list. Same pattern as `CandidateVote` above — flagging for the lead rather than silently documenting it as intentional.

Access patterns:
- "Comments on an answer, oldest first" → `Query pk=GROUP#{g}#NL#{c}#Q#{q}#A#{u} AND begins_with(sk, C#)`

### 2.12 Reaction

Emoji reaction. Last-write-wins per `(answer, reactor, emoji)`.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{group_id}#NL#{cycle_id}#Q#{question_id}#A#{answer_user_id}` |
| `sk` | `R#{reactor_user_id}#{emoji}` |
| `entity` | `Reaction` |
| `reactor_user_id` | UUIDv7 |
| `emoji` | string (1–12 codepoints) |
| `created_at` | ISO-8601 |

> **Note (reconciled 2026-09-27):** same pattern as `Comment` above — the `Reaction` struct also carries `group_id`, `cycle_id`, `question_id`, and `answer_user_id`, and `persistence::engagement::put_reaction` writes them as item attributes via `to_item`, undocumented in the table above.

To remove a reaction, delete the item.

Access patterns:
- "Reactions on an answer" → `Query pk=...#A#{u} AND begins_with(sk, R#)`

### 2.13 PushSubscription

| Attr | Value |
|---|---|
| `pk` | `USER#{user_id}` |
| `sk` | `PUSH#{endpoint_hash}` |
| `entity` | `PushSubscription` |
| `user_id` | UUIDv7 |
| `endpoint_hash` | sha256(endpoint URL) base64url |
| `endpoint` | string |
| `p256dh` | string |
| `auth` | string |
| `created_at` | ISO-8601 |
| `last_success_at` | ISO-8601 \| null |
| `failure_count` | number |
| `user_agent` | string |

Access patterns:
- "Send to all subs for a user" → `Query pk=USER#{u} AND begins_with(sk, PUSH#)`

### 2.14 NotificationPref

Per-user, per-group notification on/off.

| Attr | Value |
|---|---|
| `pk` | `USER#{user_id}` |
| `sk` | `NPREF#{group_id}` |
| `entity` | `NotificationPref` |
| `user_id` | UUIDv7 |
| `group_id` | UUIDv7 |
| `cycle_open` | bool, default true |
| `deadline_reminders` | bool, default true |

Publication notifications are always-on with no user toggle; there is no `publication` field. The only kill-switch is the OS-level / browser-level push permission.

### 2.15 Counters and idempotency markers

A few small entities for atomic bookkeeping:

- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#OPEN` — written when cycle-open notification fans out, prevents duplicates.
- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#CLOSE#{offset_hours}` — likewise per offset.
- `pk=TICK` `sk=CYCLE` — single record updated on each `lambda-cycle-tick` run with `last_ran_at`.
- `pk=TICK` `sk=NOTIFY` — likewise for `lambda-notify-tick`.

> **Note (reconciled 2026-09-27):** only the `TICK`/`CYCLE` sentinel is actually written today — `persistence::newsletters::upsert_tick_sentinel` puts `pk=TICK sk=CYCLE` with a `last_ran_at` attribute, matching this doc. The `NOTIFIED#OPEN`, `NOTIFIED#CLOSE#{offset_hours}`, and `TICK#NOTIFY` rows have key builders defined (`persistence::keys::{NOTIFIED_OPEN_SK, notified_close_sk, TICK_NOTIFY_SK}`) but nothing in the codebase currently writes or reads them — notification fan-out is still a named no-op pending M11 (`backend/crates/lambda-cycle-tick/src/notify.rs`). Not a naming mismatch, but worth the lead knowing these rows don't exist yet in any environment.

### 2.16 AvatarMedia

User avatars are uploaded via a dedicated pipeline (separate from the per-question image pipeline) so they can cross group boundaries. See `08-media-uploads.md` §11.

| Attr | Value |
|---|---|
| `pk` | `USER#{user_id}` |
| `sk` | `AVATAR#{avatar_id}` |
| `entity` | `AvatarMedia` |
| `avatar_id` | UUIDv7 |
| `user_id` | UUIDv7 (owner) |
| `mime_type` | `image/jpeg` \| `image/png` \| `image/webp` |
| `original_key` | S3 key in originals bucket (avatar prefix) |
| `display_key` | S3 key in processed bucket (256×256 WebP) — null until processed |
| `status` | `pending` \| `ready` \| `failed` |
| `bytes` | number |
| `uploaded_at` | ISO-8601 |
| `processed_at` | ISO-8601 \| null |
| `error_message` | string \| null — same values as `ImageMedia.error_message` (added M8) |

The processed avatar bucket is served via a separate CloudFront behavior (`/avatar/*`) that does NOT require signed cookies — avatars are not tenant-scoped, URLs use the UUIDv7 `avatar_id` (unguessable), and an avatar leak is acceptable. Cache-control: `public, max-age=86400, immutable` so changing avatars produces a new `avatar_id` and never collides with the old one in caches.

---

## 3. Access patterns ⇄ index map

| # | Pattern | Operation | Index |
|---|---|---|---|
| AP1 | Get user profile | GetItem | base |
| AP2 | List groups for user | Query | base |
| AP3 | List members of group | Query | GSI1 |
| AP4 | Get group meta | GetItem | base |
| AP5 | Get invite by code | GetItem | base |
| AP6 | List invites for group | Query | GSI1 |
| AP7 | List newsletters in group | Query | base |
| AP8 | Get newsletter | GetItem | base |
| AP9 | Cycles by status with next transition due | Query | GSI2 |
| AP10 | List candidate questions for next cycle | Query | base |
| AP11 | Top-N candidates by votes | Query | GSI1 (Limit, ScanIndexForward=false) |
| AP12 | List my votes this cycle | Query | base |
| AP13 | List locked questions for newsletter | Query | base |
| AP14 | List answers to a question (post-publish) | Query | base |
| AP15 | List my drafts in cycle | Query | GSI1 |
| AP16 | Get my response to a question | GetItem | base |
| AP17 | List my image uploads (admin/audit) | Query | GSI1 |
| AP18 | Comments on an answer | Query | base |
| AP19 | Reactions on an answer | Query | base |
| AP20 | Push subs for a user | Query | base |
| AP21 | Notification prefs for user × group | GetItem | base |

---

## 4. Transactional invariants

The following operations MUST use `TransactWriteItems`:

1. **Cast a vote** — three items:
   - Put `CandidateVote` with `attribute_not_exists(sk)`.
   - Update `CandidateQuestion` with `vote_count += 1` (conditioned on the prior count) and `gsi1sk` rewritten with the new padded count.
   - Update the voter's `VoterTally` (§2.7a) with `ADD votes_cast 1`, conditioned on `attribute_not_exists(votes_cast) OR votes_cast < cap`.

   The cancellation reason says which condition failed. A vote row means a concurrent duplicate request (idempotent success); a candidate count means a lost race (retry once); the tally means `VOTE_CAP_REACHED`.
2. **Withdraw a vote** — Delete `CandidateVote` with `attribute_exists(sk)` AND Update `CandidateQuestion` with `vote_count -= 1` AND new `gsi1sk` AND `ADD votes_cast -1` on the tally (`votes_cast > 0`). The admin candidate delete removes each voter's vote together with the same tally decrement.
3. **Promote candidates → locked questions** — for each promoted candidate, Put `LockedQuestion` AND Update `Newsletter` to set `locked_question_ids` and transition status from `voting` to `open`. Single transaction (cap 100 items).
4. **Save / publish response** — every response save (draft or publish) is one transaction: Update the response row (content, `updated_at`, `response_id` via `if_not_exists`), plus a ConditionCheck on the Newsletter row. On publish, the Update also sets `status` to `published` and `published_at` via `if_not_exists`; publishing is sticky (`03-api-contract.md` §7.3). (No `version` attribute exists — drafts are last-write-wins, §5.) Must check `Newsletter.status = open AND response_close_at > now`. The transaction also increments `GroupMembership.editions_answered` by 1 IFF this is the user's first published response in this cycle (precondition: query AP15 for the user × cycle returns zero `published` rows before this write). Republishing or publishing additional answers in the same cycle does not double-count.
5. **Join group via invite** — Update `Invite` from `pending` to `consumed` AND Put `GroupMembership` AND Update `Group.member_count += 1` (with member-cap check).
6. **Leave group** — Delete `GroupMembership` AND Update `Group.member_count -= 1`.
7. **Last-admin guard** (leave, kick, or demote a member who is currently an admin) — the write also carries a `ConditionCheck` that a *witness*, a different member, still has `role = admin`.
   - **Choosing the witness:** an admin acting on someone else is their own witness. An admin acting on themselves names any other admin, or gets `LAST_ADMIN` if there is none.
   - **Why it holds:** transactions are serializable, so every committed removal leaves its witness an admin, and no interleaving of concurrent requests can reach zero admins. A pre-read of the admin count alone could (two admins demoting each other).
   - **Losing the race:** a failed witness check returns `LAST_ADMIN`.

**No `ClientRequestToken`.** An earlier draft asked for one derived from `x-correlation-id`. It is deliberately not used: a single request can issue several transactions, and the vote retry re-sends with a different expected count. A reused token with different parameters fails with `IdempotentParameterMismatch`. Retry safety comes from the condition expressions and from handlers treating "already done" outcomes as success. (Reconciled 2026-09-27 against the code.)

---

## 5. Concurrency for drafts

Last-write-wins (see `03-api-contract.md` §7.3). Autosave PUTs overwrite `body`, `image_media_ids`, `poll_option_id`, and `updated_at` with no condition on the response row itself. There is no `version` attribute and no `409 Conflict` reconciliation. The only condition is the Newsletter-row check from §4 #4, which requires the cycle to still be open. The debounced autosave (≥1500 ms) makes interleaved writes from the same user rare enough that the simplicity is worth the trade. Revisit if telemetry shows actual clobbering.

---

## 6. Tenant isolation rule

Every Lambda handler that operates on a `group_id`-scoped entity MUST first verify the caller has a `GroupMembership` for that `group_id`. The check is (note: memberships are keyed by our internal `user_id`, NOT the Cognito `sub` — the handler first resolves `sub → user_id` via the `CognitoSubLookup` row, `05-auth-flow.md` §6):

```
user_id = GetItem pk=COGNITO_SUB#{jwt.sub} sk=USER_ID   (cached per cold-start ≤60s)
GetItem pk=USER#{user_id} sk=GROUP#{group_id}
```

Cached per cold-start for ≤60s, keyed by `(user_id, group_id)`. On miss → 403.

For admin-only operations: the cached membership must have `role=admin`.

---

## 7. Soft delete and archival hooks

- Comments use soft-delete (`deleted_at` set, `body` blanked). Hard delete reserved for compliance/admin.
- Responses are never deleted. Drafts that were never published simply remain `status=draft` forever and are excluded from published views.
- When a Newsletter transitions to `archived` (manual or future job), see `10-archival.md` §3 for the export-and-prune procedure.

---

## 8. Capacity and limits considerations

- Largest partition: `GROUP#{g}#NL#{c}#Q#{q}#A#{u}` (one answer's comments + reactions). Active groups won't exceed a few thousand rows here. Well within DDB's 10GB partition cap.
- Hottest write key: `CandidateQuestion` `vote_count` updates during voting. With ≤50 members and 3 votes/user/cycle = ~150 ops/cycle/group. Trivial.
- TTL fires on `Invite` items 30 days after expiry (via `ttl`). No application logic depends on these post-expiry; safe to auto-clean.

---

## 9. Why GSI2 ("status + time")

`lambda-cycle-tick` runs every 5 minutes and needs to find: any `voting` cycle whose `vote_window_close_at <= now`, any `open` cycle whose `response_close_at <= now`, any `published` cycle eligible for archival.

Without GSI2 we'd scan the table. With GSI2:

```
Query GSI2 where gsi2pk = "NL_STATUS#voting" AND gsi2sk <= "{nowKeyTimestamp}#~~~"
Query GSI2 where gsi2pk = "NL_STATUS#open"   AND gsi2sk <= "{nowKeyTimestamp}#~~~"
```

Two cheap queries per tick; processes only items whose transition is actually due. `{nowKeyTimestamp}` and every `next_transition_at` written into `gsi2sk` go through the same formatter (`persistence::keys::key_timestamp`) so the comparison sorts consistently — see the key-timestamp rule in §1.

When a cycle changes status, the writer recomputes `gsi2pk` (new status) and `gsi2sk` (new `next_transition_at`, via `persistence::keys::newsletter_gsi2sk`/`newsletter_gsi2sk_sentinel`). All such writes go through helper `persistence::newsletters::write_status_transition` to ensure GSI keys stay in sync.

---

## 10. Helper code structure (Rust)

In `backend/crates/persistence/src/`:

- `repo.rs` — `Repo` struct holding the `aws_sdk_dynamodb::Client` and table name
- `keys.rs` — pure functions building all the `pk`/`sk`/`gsi1pk`/`gsi1sk`/`gsi2pk`/`gsi2sk` strings, plus the `attr`/`index` modules centralizing literal attribute and index names. Single source of truth for key formats. Unit-tested.
- Entity structs live in `backend/crates/domain/src/entities.rs` (`serde::{Serialize, Deserialize}`, no `rename_all` on struct fields — Rust field name *is* the DynamoDB attribute name); `persistence`'s per-family modules call `serde_dynamo::{to_item, from_item}` directly rather than through a dedicated `model.rs`.
- `users.rs`, `groups.rs`, `invites.rs`, `newsletters.rs`, `questions.rs`, `responses.rs`, `engagement.rs`, `media.rs`, `push.rs` — one file per entity family with focused query/mutation functions named after the access pattern (e.g. `list_top_candidates_by_votes`, `cast_vote_tx`, `save_response`).
- `auth.rs` — caller resolution and tenant-isolation gate (`require_user_id`, `require_membership`), with the per-cold-start membership cache described in §6.
- `expr.rs` — shared `UpdateExpression` builder (`set_fields`) that aliases every patched attribute behind a numbered placeholder, so reserved words never need special-casing.
- `tests/` — integration tests against a local DynamoDB container (see `11-testing-ci-cd.md`).

> **Note (reconciled 2026-09-27):** this section previously named a `model.rs` with `from_item`/`to_item` helpers; no such file exists. Each entity-family module (`groups.rs`, `invites.rs`, etc.) calls `serde_dynamo::{to_item, from_item}` inline against the structs in `domain::entities`, and manually inserts the key/`entity` attributes afterward (see e.g. `persistence::groups::put_group`). `mod.rs` is also renamed above to `repo.rs`, its actual filename.

The `keys.rs` functions are the contract. If you change a key shape, you change one file and the type system finds every dependent call.
