# 02 — Data Model (DynamoDB Single-Table)

This document defines every entity stored in the `OpenNewsletter-{env}` table. Read this in conjunction with `03-api-contract.md` (which references the access patterns by name) and `06-newsletter-lifecycle.md` (which references the cycle state transitions).

---

## 1. Table shape recap

- Hash key: `pk` (S)
- Range key: `sk` (S)
- TTL attribute: `ttl` (N, epoch seconds; only set on items that should auto-expire)
- Stream: NEW_AND_OLD_IMAGES (consumed by archival Lambda — see `10-archival.md`)

Two GSIs:

- **GSI1** — `gsi1pk` / `gsi1sk` — generic inversion ("show me all X for parent Y")
- **GSI2** — `gsi2pk` / `gsi2sk` — by-status / by-time ("show me cycles whose nextTransitionAt is within the next 5 minutes")

All keys are strings. All ID values are UUIDv7 unless noted.

---

## 2. Entity catalog

For each entity below: `pk`, `sk`, optional GSI keys, the application attributes, and a brief explanation. `entity` attribute is set on every item for stream consumers and debugging.

### 2.1 User

A Cognito-authenticated person.

| Attr | Value |
|---|---|
| `pk` | `USER#{userId}` |
| `sk` | `PROFILE` |
| `entity` | `User` |
| `userId` | UUIDv7 (= Cognito `sub`) |
| `email` | string |
| `displayName` | string |
| `avatarMediaId` | string \| null |
| `createdAt` | ISO-8601 |
| `lastLoginAt` | ISO-8601 |

`userId` mirrors the Cognito sub so we never need to look it up.

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
| `notificationSettings` | map: `{ offsetsHoursBeforeClose: [96, 48, 24], onCycleOpen: true }` |
| `memberCount` | number (denormalized; updated transactionally on join/leave) |
| `memberSoftCap` | number, default 50 |
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
| `submittedBy` | userId (server-side only; never returned to non-admins) |
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
| `submittedBy` | userId (server-side only) |
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
| `version` | number (monotonic; incremented on each save; used for optimistic concurrency) |
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
| `mimeType` | `image/jpeg` \| `image/png` \| `image/webp` \| `image/gif` |
| `originalKey` | S3 key in originals bucket |
| `displayKey` | S3 key in processed bucket (1200px WebP) — null until processed |
| `thumbKey` | S3 key in processed bucket (400px WebP) — null until processed |
| `status` | `pending` \| `ready` \| `failed` |
| `bytes` | number |
| `width` / `height` | numbers (from processor) |
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
| `createdAt` | ISO-8601 |
| `editedAt` | ISO-8601 \| null |
| `deletedAt` | ISO-8601 \| null (soft delete; body replaced with empty string when set) |

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

### 2.15 Counters and idempotency markers

A few small entities for atomic bookkeeping:

- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#OPEN` — written when cycle-open notification fans out, prevents duplicates.
- `pk=GROUP#{g}#NL#{c}` `sk=NOTIFIED#CLOSE#{offsetHours}` — likewise per offset.
- `pk=TICK` `sk=CYCLE` — single record updated on each `lambda-cycle-tick` run with `lastRanAt`.
- `pk=TICK` `sk=NOTIFY` — likewise for `lambda-notify-tick`.

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
4. **Publish response** — Update response `status` from `draft` to `published`, increment `version`, set `publishedAt`. Must check `Newsletter.status = open`.
5. **Join group via invite** — Update `Invite` from `pending` to `consumed` AND Put `GroupMembership` AND Update `Group.memberCount += 1` (with member-cap check).
6. **Leave group** — Delete `GroupMembership` AND Update `Group.memberCount -= 1`.

For all transactions, use a `ClientRequestToken` derived from the request's `x-correlation-id` so retries are idempotent within DynamoDB's 10-minute window.

---

## 5. Optimistic concurrency for drafts

Response items carry a `version` (number). Autosave PUTs use:

```
ConditionExpression: attribute_not_exists(version) OR version = :expectedVersion
UpdateExpression:    SET ..., version = :expectedVersion + 1
```

If the condition fails, the API returns `409 Conflict` with the current server version + body. The frontend reconciles (typically by overwriting if local state is newer; see `04-frontend-architecture.md` §7.4).

---

## 6. Tenant isolation rule

Every Lambda handler that operates on a `groupId`-scoped entity MUST first verify the caller has a `GroupMembership` for that `groupId`. The check is:

```
GetItem pk=USER#{callerSub} sk=GROUP#{groupId}
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
