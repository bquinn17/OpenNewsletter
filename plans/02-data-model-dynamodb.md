# 02 — Data Model (DynamoDB Single-Table)

This document defines every entity stored in the `OpenNewsletter-{env}` table. Read this in conjunction with `03-api-contract.md` (which references the access patterns by name) and `06-newsletter-lifecycle.md` (which references the cycle state transitions).

---

## 1. Table shape recap

- Hash key: `pk` (S)
- Range key: `sk` (S)
- TTL attribute: `ttl` (N, epoch seconds; only set on items that should auto-expire)
- Stream: **disabled in v1.** The only planned consumer is the archival Lambda (`10-archival.md`), which is deferred. Streams add cost with no v1 reader. Re-enable (`NEW_AND_OLD_IMAGES`) when archival lands; the table reconfigure is non-disruptive.

Two GSIs:

- **GSI1** — `gsi1pk` / `gsi1sk` — generic inversion ("show me all X for parent Y")
- **GSI2** — `gsi2pk` / `gsi2sk` — by-status / by-time ("show me cycles whose nextTransitionAt is within the next 5 minutes")

All keys are strings. All ID values are UUIDv7 unless noted.

---

## 2. Entity catalog

For each entity below: `pk`, `sk`, optional GSI keys, the application attributes, and a brief explanation. `entity` attribute is set on every item for debugging (and for the future archival stream consumer — streams themselves are off in v1).

### 2.1 User

A Cognito-authenticated person.

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `PROFILE` |
| `entity` | `User` |
| `userId` | UUIDv7 (generated server-side on first invite redemption) |
| `cognitoSub` | string (Cognito `sub` — UUIDv4-shaped) |
| `email` | string |
| `displayName` | string |
| `avatarColor` | string (slug, e.g. `grape`/`coral`/`mint`; deterministic fallback derived from `userId` hash if unset) |
| `avatarMediaId` | string \| null (references an `AvatarMedia` row — see §2.16) |
| `createdAt` | ISO-8601 |
| `lastLoginAt` | ISO-8601 |

`userId` is our own UUIDv7, not the Cognito sub. This preserves the global "all IDs are UUIDv7, sortable by creation time" invariant. Map sub → userId via the lookup row below.

### 2.1a Cognito-sub lookup

| Attr | Value |
|---|---|
| `pk` | `COGNITO_SUB#{sub}` |
| `sk` | `USER_ID` |
| `entity` | `CognitoSubLookup` |
| `userId` | UUIDv7 |

Created in the same `TransactWriteItems` as the User row (during invite redemption). Read on every authenticated request to resolve `claims.sub` → our `userId`. Cached per cold-start for ≤60s alongside the membership cache.

### 2.2 GroupMembership (also serves as Group → user index via GSI1)

A user's membership in one group.

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `GROUP#{groupId}` |
| `gsi1pk` | `GROUP#{groupId}` |
| `gsi1sk` | `MEMBER#{userId}` |
| `entity` | `GroupMembership` |
| `userId` | UUIDv7 |
| `groupId` | UUIDv7 |
| `role` | `admin` \| `member` |
| `joinedAt` | ISO-8601 |
| `editionsAnswered` | number (denormalized; incremented in the publish-response transaction the first time a user publishes any answer in a given cycle) |

Access patterns served:
- "What groups am I in?" → `Query pk = USER#{userId} AND begins_with(sk, GROUP#)`
- "Who is in this group?" → `Query GSI1 gsi1pk = GROUP#{groupId} AND begins_with(gsi1sk, MEMBER#)`

### 2.3 Group

The tenant.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}` |
| `sk` | `META` |
| `entity` | `Group` |
| `groupId` | UUIDv7 |
| `name` | string |
| `timezone` | IANA TZ string, default `America/New_York` |
| `cycleSettings` | map: `{ questionsPerCycle, votesPerUserPerCycle, responseWindowDays, autoPublish: true }` |
| `notificationSettings` | map: `{ offsetsHoursBeforeClose: [96, 48, 24], onCycleOpen: true }` (publication push is always-on with no toggle) |
| `memberCount` | number (denormalized; updated transactionally on join/leave) |
| `memberSoftCap` | number, default 50 |
| `gradient` | string (slug from a small preset palette, e.g. `grape-sky`, `coral-sun`; chosen at group creation, admin-editable in Settings) |
| `createdAt` | ISO-8601 |
| `createdBy` | userId |

### 2.4 Invite

| Attr | Value |
|---|---|
| `pk` | `INVITE#{code}` |
| `sk` | `META` |
| `gsi1pk` | `GROUP#{groupId}` |
| `gsi1sk` | `INVITE#{code}` |
| `entity` | `Invite` |
| `code` | string (16-char base32 crockford) |
| `groupId` | UUIDv7 |
| `createdBy` | userId (admin) |
| `createdAt` | ISO-8601 |
| `expiresAt` | ISO-8601 |
| `ttl` | epoch seconds (= `expiresAt` + 30 days, so the audit log lingers a month after expiry then auto-purges) |
| `status` | `pending` \| `consumed` \| `revoked` |
| `consumedBy` | userId \| null |
| `consumedAt` | ISO-8601 \| null |
| `roleOnRedeem` | `admin` \| `member` (defaults to `member`; admins can issue admin invites) |

Access patterns:
- "Validate invite by code" → `GetItem pk=INVITE#{code} sk=META`
- "List invites for group" → `Query GSI1 gsi1pk=GROUP#{groupId} AND begins_with(gsi1sk, INVITE#)`

### 2.5 Newsletter (Cycle)

The unit of an edition for one group.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}` |
| `sk` | `NL#{yyyymm}` |
| `gsi2pk` | `NL_STATUS#{status}` |
| `gsi2sk` | `{nextTransitionAt}#{groupId}#{yyyymm}` |
| `entity` | `Newsletter` |
| `groupId` | UUIDv7 |
| `cycleId` | string `yyyymm` (e.g. `202605`) |
| `status` | `voting` \| `open` \| `published` \| `archived` |
| `voteWindowOpenAt` | ISO-8601 (typically the publication of the previous cycle) |
| `voteWindowCloseAt` | ISO-8601 (= `responseOpenAt`) |
| `responseOpenAt` | ISO-8601 |
| `responseCloseAt` | ISO-8601 |
| `publishedAt` | ISO-8601 \| null |
| `nextTransitionAt` | ISO-8601 — the next instant any state change is due. Drives `lambda-cycle-tick`. |
| `lockedQuestionIds` | array<string> (set when transitioning `voting -> open`) |
| `notifiedOffsetsHours` | array<number> (records which reminder fan-outs have completed) |
| `notifiedOnOpen` | bool |

State machine fully specified in `06-newsletter-lifecycle.md`.

Access patterns:
- "List newsletters for a group" → `Query pk=GROUP#{groupId} AND begins_with(sk, NL#)` (DESC by sk = most recent first)
- "Find a specific edition" → `GetItem pk=GROUP#{groupId} sk=NL#{yyyymm}`
- "Find all cycles whose next transition is due" → `Query GSI2 gsi2pk=NL_STATUS#{status} AND gsi2sk <= {nowISO}#~~~` for each non-terminal status

### 2.6 Candidate question (in voting pool)

Suggested by users for the next not-yet-opened cycle.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#CYCLE#{nextCycleId}` |
| `sk` | `QC#{questionId}` |
| `gsi1pk` | `GROUP#{groupId}#CYCLE#{nextCycleId}#VOTES` |
| `gsi1sk` | `{paddedVoteCount}#{questionId}` (pad to 6 digits, e.g. `000007#01HX...`) |
| `entity` | `CandidateQuestion` |
| `questionId` | UUIDv7 |
| `groupId` | UUIDv7 |
| `nextCycleId` | string `yyyymm` |
| `kind` | `text` \| `poll` |
| `prompt` | string (≤500 chars) |
| `pollOptions` | array<{ optionId: UUIDv7, label: string }> \| null (only when `kind=poll`) |
| `voteCount` | number (denormalized; updated transactionally on vote add/remove) |
| `submittedBy` | userId (always recorded; returned to non-admins ONLY when `isAnonymous=false`) |
| `isAnonymous` | boolean (submitter's choice at creation; immutable thereafter) |
| `submittedAt` | ISO-8601 |

GSI1 enables "leaderboard for current cycle" reads in O(votes) without a scan.

Access patterns:
- "List candidates for a cycle" → `Query pk=GROUP#{g}#CYCLE#{c} AND begins_with(sk, QC#)`
- "Top N candidates by votes" → `Query GSI1 gsi1pk=GROUP#{g}#CYCLE#{c}#VOTES` ScanIndexForward=false Limit=N

### 2.7 Candidate vote

Records that a user upvoted a candidate question.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#CYCLE#{nextCycleId}#VOTER#{userId}` |
| `sk` | `QC#{questionId}` |
| `entity` | `CandidateVote` |
| `userId` | UUIDv7 |
| `questionId` | UUIDv7 |
| `votedAt` | ISO-8601 |

Cap of `votesPerUserPerCycle` enforced via `Query` of voter partition + count compare in `TransactWriteItems` (see `03-api-contract.md` §6.4).

Access patterns:
- "List my votes this cycle" → `Query pk=GROUP#{g}#CYCLE#{c}#VOTER#{u}`
- "Has user voted for this question?" → `GetItem pk=GROUP#{g}#CYCLE#{c}#VOTER#{u} sk=QC#{q}`

### 2.8 Locked question

A candidate that has been promoted into a specific newsletter.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#NL#{cycleId}` |
| `sk` | `Q#{questionId}` |
| `entity` | `LockedQuestion` |
| `questionId` | UUIDv7 |
| `groupId` | UUIDv7 |
| `cycleId` | string |
| `kind` | `text` \| `poll` |
| `prompt` | string |
| `pollOptions` | array \| null |
| `displayOrder` | number (admin-controllable; default = vote rank when promoted) |
| `submittedBy` | userId (copied from the candidate at promotion; returned to non-admins ONLY when `isAnonymous=false`) |
| `isAnonymous` | boolean (copied from the candidate at promotion; immutable) |
| `lockedAt` | ISO-8601 |

Access patterns:
- "Show me this newsletter's questions in order" → `Query pk=GROUP#{g}#NL#{c} AND begins_with(sk, Q#)`, then sort by `displayOrder` client-side (or use a sort key that embeds displayOrder; simpler to sort in memory since N≤20)

### 2.9 Response (Answer)

A user's reply to one locked question.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#NL#{cycleId}#Q#{questionId}` |
| `sk` | `A#{userId}` |
| `gsi1pk` | `USER#{userId}#NL#{cycleId}` |
| `gsi1sk` | `Q#{questionId}` |
| `entity` | `Response` |
| `responseId` | UUIDv7 (used as a stable id for engagement targeting) |
| `userId` | UUIDv7 |
| `questionId` | UUIDv7 |
| `groupId` | UUIDv7 |
| `cycleId` | string |
| `kind` | `text` \| `poll` (mirrors question kind) |
| `status` | `draft` \| `published` |
| `body` | string (markdown-safe; ≤20k chars) — only when `kind=text` |
| `pollOptionId` | UUIDv7 \| null — only when `kind=poll` (last-write-wins) |
| `imageMediaIds` | array<string> (≤10) — only when `kind=text` |
| `updatedAt` | ISO-8601 |
| `publishedAt` | ISO-8601 \| null |

Access patterns:
- "List answers to a question (after publish)" → `Query pk=GROUP#{g}#NL#{c}#Q#{q} AND begins_with(sk, A#)`
- "List my drafts in this cycle" → `Query GSI1 gsi1pk=USER#{u}#NL#{c}`
- "Get my draft for a question" → `GetItem pk=GROUP#{g}#NL#{c}#Q#{q} sk=A#{u}`

### 2.10 ImageMedia

Each uploaded image. Originals lifecycle is owned by S3; the table tracks metadata + status.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#NL#{cycleId}` |
| `sk` | `IMG#{imageId}` |
| `gsi1pk` | `USER#{userId}#IMG` |
| `gsi1sk` | `{uploadedAt}#{imageId}` |
| `entity` | `ImageMedia` |
| `imageId` | UUIDv7 |
| `userId` | userId (uploader) |
| `groupId` | UUIDv7 |
| `cycleId` | string |
| `questionId` | string \| null (set when attached to a response; null on initial upload) |
| `purpose` | `response` \| `comment` — what the image is destined to attach to. Set at `POST /uploads` time (`03-api-contract.md` §9.1). The 10-images-per-answer cap counts `purpose=response` rows only; comment images are capped at one per comment at comment-create time. |
| `mimeType` | `image/jpeg` \| `image/png` \| `image/webp` \| `image/gif` |
| `originalKey` | S3 key in originals bucket |
| `displayKey` | S3 key in processed bucket (1200px WebP) — null until processed |
| `thumbKey` | S3 key in processed bucket (400px WebP) — null until processed |
| `status` | `pending` \| `ready` \| `failed` |
| `bytes` | number |
| `width` / `height` | numbers (from processor) |
| `caption` | string \| null (author-supplied, ≤140 chars; rendered beneath the image in the published view) |
| `uploadedAt` | ISO-8601 |
| `processedAt` | ISO-8601 \| null |

### 2.11 Comment

Flat reply to a published response.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#NL#{cycleId}#Q#{questionId}#A#{answerUserId}` |
| `sk` | `C#{createdAt}#{commentId}` |
| `entity` | `Comment` |
| `commentId` | UUIDv7 |
| `authorUserId` | UUIDv7 |
| `body` | string (≤2000 chars; markdown-safe) |
| `imageMediaId` | string \| null (optional single image attached to the comment; references an `ImageMedia` row owned by the comment author) |
| `createdAt` | ISO-8601 |
| `editedAt` | ISO-8601 \| null |
| `deletedAt` | ISO-8601 \| null (soft delete; `body` and `imageMediaId` cleared when set) |

Access patterns:
- "Comments on an answer, oldest first" → `Query pk=GROUP#{g}#NL#{c}#Q#{q}#A#{u} AND begins_with(sk, C#)`

### 2.12 Reaction

Emoji reaction. Last-write-wins per `(answer, reactor, emoji)`.

| Attr | Value |
|---|---|
| `pk` | `GROUP#{groupId}#NL#{cycleId}#Q#{questionId}#A#{answerUserId}` |
| `sk` | `R#{reactorUserId}#{emoji}` |
| `entity` | `Reaction` |
| `reactorUserId` | UUIDv7 |
| `emoji` | string (1–12 codepoints) |
| `createdAt` | ISO-8601 |

To remove a reaction, delete the item.

Access patterns:
- "Reactions on an answer" → `Query pk=...#A#{u} AND begins_with(sk, R#)`

### 2.13 PushSubscription

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `PUSH#{endpointHash}` |
| `entity` | `PushSubscription` |
| `userId` | UUIDv7 |
| `endpointHash` | sha256(endpoint URL) base64url |
| `endpoint` | string |
| `p256dh` | string |
| `auth` | string |
| `createdAt` | ISO-8601 |
| `lastSuccessAt` | ISO-8601 \| null |
| `failureCount` | number |
| `userAgent` | string |

Access patterns:
- "Send to all subs for a user" → `Query pk=USER#{u} AND begins_with(sk, PUSH#)`

### 2.14 NotificationPref

Per-user, per-group notification on/off.

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `NPREF#{groupId}` |
| `entity` | `NotificationPref` |
| `userId` | UUIDv7 |
| `groupId` | UUIDv7 |
| `cycleOpen` | bool, default true |
| `deadlineReminders` | bool, default true |

Publication notifications are always-on with no user toggle; there is no `publication` field. The only kill-switch is the OS-level / browser-level push permission.

### 2.15 Counters and idempotency markers

A few small entities for atomic bookkeeping:

- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#OPEN` — written when cycle-open notification fans out, prevents duplicates.
- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#CLOSE#{offsetHours}` — likewise per offset.
- `pk=TICK` `sk=CYCLE` — single record updated on each `lambda-cycle-tick` run with `lastRanAt`.
- `pk=TICK` `sk=NOTIFY` — likewise for `lambda-notify-tick`.

### 2.16 AvatarMedia

User avatars are uploaded via a dedicated pipeline (separate from the per-question image pipeline) so they can cross group boundaries. See `08-media-uploads.md` §11.

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `AVATAR#{avatarId}` |
| `entity` | `AvatarMedia` |
| `avatarId` | UUIDv7 |
| `userId` | UUIDv7 (owner) |
| `mimeType` | `image/jpeg` \| `image/png` \| `image/webp` |
| `originalKey` | S3 key in originals bucket (avatar prefix) |
| `displayKey` | S3 key in processed bucket (256×256 WebP) — null until processed |
| `status` | `pending` \| `ready` \| `failed` |
| `bytes` | number |
| `uploadedAt` | ISO-8601 |
| `processedAt` | ISO-8601 \| null |

The processed avatar bucket is served via a separate CloudFront behavior (`/avatar/*`) that does NOT require signed cookies — avatars are not tenant-scoped, URLs use the UUIDv7 `avatarId` (unguessable), and an avatar leak is acceptable. Cache-control: `public, max-age=86400, immutable` so changing avatars produces a new `avatarId` and never collides with the old one in caches.

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

1. **Cast a vote** — Put `CandidateVote` with `attribute_not_exists(sk)` AND Update `CandidateQuestion` with `voteCount += 1` AND `gsi1sk` rewritten with new padded count. Plus a "vote-count check" on the voter partition (see `03-api-contract.md` §6.4).
2. **Withdraw a vote** — Delete `CandidateVote` with `attribute_exists(sk)` AND Update `CandidateQuestion` with `voteCount -= 1` AND new `gsi1sk`.
3. **Promote candidates → locked questions** — for each promoted candidate, Put `LockedQuestion` AND Update `Newsletter` to set `lockedQuestionIds` and transition status from `voting` to `open`. Single transaction (cap 100 items).
4. **Publish response** — Update response `status` from `draft` to `published`, set `publishedAt`. (No `version` attribute exists — drafts are last-write-wins, §5.) Must check `Newsletter.status = open`. The transaction also increments `GroupMembership.editionsAnswered` by 1 IFF this is the user's first published response in this cycle (precondition: query AP15 for the user × cycle returns zero `published` rows before this write). Republishing or publishing additional answers in the same cycle does not double-count.
5. **Join group via invite** — Update `Invite` from `pending` to `consumed` AND Put `GroupMembership` AND Update `Group.memberCount += 1` (with member-cap check).
6. **Leave group** — Delete `GroupMembership` AND Update `Group.memberCount -= 1`.

For all transactions, use a `ClientRequestToken` derived from the request's `x-correlation-id` so retries are idempotent within DynamoDB's 10-minute window.

---

## 5. Concurrency for drafts

Last-write-wins (see `03-api-contract.md` §7.3). Autosave PUTs are unconditional `UpdateItem` calls that overwrite `body`, `imageMediaIds`, `pollOptionId`, and `updatedAt`. No `version` attribute, no `ConditionExpression`, no `409 Conflict` reconciliation. The debounced autosave (≥1500 ms) makes interleaved writes from the same user rare enough that the simplicity is worth the trade. Revisit if telemetry shows actual clobbering.

---

## 6. Tenant isolation rule

Every Lambda handler that operates on a `groupId`-scoped entity MUST first verify the caller has a `GroupMembership` for that `groupId`. The check is (note: memberships are keyed by our internal `userId`, NOT the Cognito `sub` — the handler first resolves `sub → userId` via the `CognitoSubLookup` row, `05-auth-flow.md` §6):

```
userId = GetItem pk=COGNITO_SUB#{jwt.sub} sk=USER_ID   (cached per cold-start ≤60s)
GetItem pk=USER#{userId} sk=GROUP#{groupId}
```

Cached per cold-start for ≤60s, keyed by `(userId, groupId)`. On miss → 403.

For admin-only operations: the cached membership must have `role=admin`.

---

## 7. Soft delete and archival hooks

- Comments use soft-delete (`deletedAt` set, `body` blanked). Hard delete reserved for compliance/admin.
- Responses are never deleted. Drafts that were never published simply remain `status=draft` forever and are excluded from published views.
- When a Newsletter transitions to `archived` (manual or future job), see `10-archival.md` §3 for the export-and-prune procedure.

---

## 8. Capacity and limits considerations

- Largest partition: `GROUP#{g}#NL#{c}#Q#{q}#A#{u}` (one answer's comments + reactions). Active groups won't exceed a few thousand rows here. Well within DDB's 10GB partition cap.
- Hottest write key: `CandidateQuestion` `voteCount` updates during voting. With ≤50 members and 3 votes/user/cycle = ~150 ops/cycle/group. Trivial.
- TTL fires on `Invite` items 30 days after expiry (via `ttl`). No application logic depends on these post-expiry; safe to auto-clean.

---

## 9. Why GSI2 ("status + time")

`lambda-cycle-tick` runs every 5 minutes and needs to find: any `voting` cycle whose `voteWindowCloseAt <= now`, any `open` cycle whose `responseCloseAt <= now`, any `published` cycle eligible for archival.

Without GSI2 we'd scan the table. With GSI2:

```
Query GSI2 where gsi2pk = "NL_STATUS#voting" AND gsi2sk <= "{nowISO}#~~~"
Query GSI2 where gsi2pk = "NL_STATUS#open"   AND gsi2sk <= "{nowISO}#~~~"
```

Two cheap queries per tick; processes only items whose transition is actually due.

When a cycle changes status, the writer recomputes `gsi2pk` (new status) and `gsi2sk` (new `nextTransitionAt`). All such writes go through helper `persistence::cycle::write_status_transition` to ensure GSI keys stay in sync.

---

## 10. Helper code structure (Rust)

In `backend/crates/persistence/src/`:

- `mod.rs` — `Repo` struct holding the `aws_sdk_dynamodb::Client` and table name
- `keys.rs` — pure functions building all the `pk`/`sk`/`gsi1pk`/`gsi1sk`/`gsi2pk`/`gsi2sk` strings. Single source of truth for key formats. Unit-tested.
- `model.rs` — `serde::{Serialize, Deserialize}` structs for every entity, with `from_item` / `to_item` helpers using `serde_dynamo`.
- `users.rs`, `groups.rs`, `invites.rs`, `newsletters.rs`, `questions.rs`, `responses.rs`, `engagement.rs`, `media.rs`, `push.rs` — one file per entity family with focused query/mutation functions named after the access pattern (e.g. `list_top_candidates_by_votes`, `cast_vote_tx`, `publish_response`).
- `tests/` — integration tests against a local DynamoDB container (see `11-testing-ci-cd.md`).

The `keys.rs` functions are the contract. If you change a key shape, you change one file and the type system finds every dependent call.
