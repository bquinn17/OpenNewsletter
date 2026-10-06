# 03 — API Contract

This is the source-of-truth list of every HTTP endpoint OpenNewsletter exposes. Each route names its handler Lambda (matching `01-infrastructure-cdk.md` §6.2), the auth/role requirement, request/response shapes, error codes, and the access-pattern numbers from `02-data-model-dynamodb.md`.

A canonical OpenAPI 3.1 document lives at `shared/openapi.yaml`. This Markdown is the human-readable mirror; the YAML is what gets codegen'd into TS types and Rust route stubs.

> **Status**: `shared/openapi.yaml` now exists (landed in M5, "OpenAPI contract") and covers every route through M4 (§2-§4, §13). **It is now authoritative; this document is the human-readable mirror.** M5's own newsletter and candidate-question routes (§5-§6) are added to the YAML separately — check there for the current source of truth on any drift.

---

## 1. General conventions

- Base URL: `https://api.opennewsletter.example.com` (prod). Dev has no custom domains — the raw `https://{apiId}.execute-api.us-east-1.amazonaws.com` endpoint is used (see `13-dev-environments.md` §2).
- Content type: `application/json` (UTF-8) for everything except media uploads (which go directly to S3).
- Auth: All routes require a Cognito JWT in `Authorization: Bearer {idToken}` UNLESS marked **Public**. There are no Public routes — the OAuth callback is a Cognito-managed redirect, not an app endpoint. **The ID token, not the access token**: `POST /invites/redeem` creates the caller's `User` row from the `email` and `name` claims, which only the ID token carries (`05-auth-flow.md` §4.2). The JWT authorizer's audience is the frontend client ID, which matches the ID token's `aud`.
- Path tenancy: Routes that operate inside a group are prefixed `/groups/{groupId}/...`. The handler verifies caller membership before doing anything (per `02-data-model-dynamodb.md` §6).
- Correlation: Clients SHOULD send `x-correlation-id: {ulid}`. If absent the API generates one. It is echoed in responses and logged.
- Pagination: List endpoints accept `?cursor={opaque}&limit={1..100}`. Responses include `nextCursor` (null when exhausted). Cursor is base64url(JSON) encoding DynamoDB's `LastEvaluatedKey`. A cursor that decodes but doesn't address a row in the listing's own partition and key range (e.g. one taken from another answer's comment list) is a 422 on `cursor`, the same as a malformed one (decided M10, `persistence::cursor::decode_scoped`).
- Timestamps: All ISO-8601 UTC.
- IDs: All UUIDv7 strings except `cycleId` (yyyymm) and `inviteCode` (16-char Crockford base32).

### 1.1 Error format

RFC-7807 Problem Details:

```json
{
  "type": "https://api.opennewsletter.example.com/errors/{slug}",
  "title": "Human-readable summary",
  "status": 400,
  "detail": "Specific message",
  "code": "MACHINE_READABLE_CODE",
  "correlationId": "01HX...",
  "fieldErrors": [{"field": "body", "code": "INVALID", "message": "body must be at most 10000 characters"}]
}
```

`fieldErrors` is present only on `VALIDATION_FAILED` responses whose problem is one named request field. `field` is the camelCase wire name (query parameters included, e.g. `limit`, `cursor`). `code` is currently always `INVALID`; `message` repeats `detail`. It is omitted for validation failures not tied to one field, such as malformed JSON or a missing `email` claim. (Implemented 2026-09-27 via `ApiError::invalid_field`.)

Error code catalog (machine-readable codes for the top-level `code`):

| Code | HTTP | Meaning |
|---|---|---|
| `UNAUTHENTICATED` | 401 | No/invalid JWT |
| `FORBIDDEN` | 403 | Authenticated but not authorized for this resource (membership/admin check failed) |
| `NOT_FOUND` | 404 | Resource missing or not visible to caller |
| `VALIDATION_FAILED` | 422 | Body/query failed schema validation; see `fieldErrors` |
| `INVITE_INVALID` | 400 | Code unknown |
| `INVITE_EXPIRED` | 410 | Past `expiresAt` |
| `INVITE_CONSUMED` | 409 | Already used |
| `MEMBER_CAP_REACHED` | 409 | Group has hit its member soft cap |
| `VOTE_CAP_REACHED` | 409 | User exhausted `votesPerUserPerCycle` |
| `CYCLE_NOT_VOTING` | 409 | Action requires cycle in `voting` status |
| `CYCLE_NOT_OPEN` | 409 | Action requires cycle in `open` status |
| `CYCLE_NOT_PUBLISHED` | 409 | Action requires cycle in `published` status |
| `LAST_ADMIN` | 409 | Can't remove or demote the group's only admin, or a concurrent admin change won the race (reload and retry; `02` §4 #7) |
| `CANDIDATE_PROMOTED` | 409 | Candidate already locked into a cycle; can no longer be deleted |
| `NEWSLETTER_ARCHIVED` | 410 | Cycle has transitioned to `archived`; see `10-archival.md` (not reachable until archival ships) |
| `IMAGE_LIMIT_EXCEEDED` | 409 | More than 10 images attached |
| `IMAGE_TOO_LARGE` | 413 | Original > 15MB |
| `IMAGE_BAD_TYPE` | 415 | MIME type not allowed |
| `IMAGE_IN_USE` | 409 | `DELETE /uploads/{imageId}` on an image the caller's published answer still references (added M8), or that a live comment has attached (added M10) |
| `RATE_LIMITED` | 429 | Throttled |
| `INTERNAL` | 500 | Generic server error |

---

## 2. Bootstrap / metadata routes

### 2.1 `GET /config`

**Lambda**: `lambda-groups`. **Auth**: required. **Purpose**: bootstrap data the SPA needs after login.

Response 200:
```json
{
  "userId": "01HX...",
  "email": "user@example.com",
  "displayName": "Quinn",
  "vapidPublicKey": "BPV...",
  "groupDefaults": { "...": "..." },
  "memberships": [
    { "groupId": "01H...", "role": "admin", "groupName": "Trail Crew",
      "timezone": "America/New_York", "gradient": "grape-sky" }
  ]
}
```

`memberships` is a denormalized join. Each entry includes `groupName`, `timezone`, and `gradient`, so the frontend can render the group switcher (gradient swatch) and label cycles in the group's timezone (`04` §14a) without a follow-up query per group. The handler reads all the group rows in one `BatchGetItem`.

### 2.2 `GET /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Returns the bare User profile (no memberships). Used by the settings page.

Response 200: `User` shape from `02-data-model-dynamodb.md` §2.1.

### 2.3 `PATCH /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Body: `{ "displayName"?: string, "avatarColor"?: string, "avatarMediaId"?: string|null }`

Validation: `displayName` 1–40 chars, non-whitespace. `avatarColor` one of the eight allowed slugs: `red`, `orange`, `amber`, `green`, `teal`, `blue`, `violet`, `pink` (canonical home: an enum in `shared/openapi.yaml`, mirrored in `backend/crates/shared/src/config.rs` and the frontend palette table — the contract test in `11-testing-ci-cd.md` §2.3 keeps the Rust mirror honest). `avatarMediaId` (when non-null) must reference an `AvatarMedia` row owned by the caller in `status=ready`; otherwise `VALIDATION_FAILED`. Setting `avatarMediaId: null` clears the photo and the deterministic `avatarColor` takes over for rendering.

Response 200: updated `User`.

---

## 3. Invite & membership routes

### 3.1 `POST /admin/invites`

**Lambda**: `lambda-invites`. **Auth**: required. **Role**: must be admin of the target group.

Body:
```json
{
  "groupId": "01H...",
  "ttlDays": 7,                    // optional, defaults to group setting (7)
  "roleOnRedeem": "member"         // "member" | "admin"
}
```

Response 201:
```json
{
  "code": "ABCD-EFGH-JKMN-PQRS",
  "groupId": "01H...",
  "expiresAt": "...",
  "roleOnRedeem": "member"
}
```

Errors: `FORBIDDEN`, `VALIDATION_FAILED`.

### 3.2 `GET /admin/groups/{groupId}/invites`

**Lambda**: `lambda-invites`. **Role**: admin.

List of invites for the group with status. Uses AP6.

Response 200: `{ "items": [Invite], "nextCursor": null }`

### 3.3 `POST /admin/invites/{code}/revoke`

**Lambda**: `lambda-invites`. **Role**: admin of the invite's group.

Marks status `revoked`. Idempotent.

Response 204.

### 3.4 `POST /invites/redeem`

**Lambda**: `lambda-invites`. **Auth**: required. **Role**: any authenticated user.

The single, canonical place where invites are consumed. It covers **both** cases: first signup (the handler lazily creates the `CognitoSubLookup` + `User` rows when the caller's `sub` has no `userId` yet) and an existing user joining an additional group. The `PreSignUp` trigger is a pass-through and never touches invites — see `05-auth-flow.md` §3–§4 for the full algorithm.

Body: `{ "code": "ABCD-EFGH-JKMN-PQRS" }`

Response 200:
```json
{
  "groupId": "01H...",
  "groupName": "Trail Crew",
  "role": "member"
}
```

Errors: `INVITE_INVALID`, `INVITE_EXPIRED`, `INVITE_CONSUMED`, `MEMBER_CAP_REACHED`.

### 3.5 `DELETE /groups/{groupId}/members/{userId}` (leave or kick)

**Lambda**: `lambda-groups`. **Role**: self OR admin (admin can remove others; users can remove themselves).

Last admin can't leave — returns 409 `LAST_ADMIN`.

Response 204.

### 3.6 `PATCH /groups/{groupId}/members/{userId}` (role change)

**Lambda**: `lambda-groups`. **Role**: admin.

Body: `{ "role": "admin"|"member" }`

Last-admin guard: demoting the only admin returns 409 `LAST_ADMIN`. Per-group nicknames are not a v1 feature; member display always reflects the user's account `displayName`.

---

## 4. Group routes

### 4.1 `GET /groups`

**Lambda**: `lambda-groups`. Returns the same `memberships` array as `/config`. Useful when the SPA refreshes group list without re-fetching everything.

### 4.2 `GET /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: member.

Response 200: full `Group` (incl. settings + `gradient` slug) plus `members: [{userId, displayName, role, avatarColor, avatarUrl, joinedAt, editionsAnswered}]`. `avatarUrl` is the absolute CloudFront URL for the member's avatar (`https://cdn.{domain}/avatar/{avatarId}/display.webp`) when they have one set, else `null`; `avatarColor` is the deterministic / chosen fallback color slug rendered as a colored initial when `avatarUrl` is null.

### 4.3 `PATCH /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: admin.

Body (all optional):
```json
{
  "name": "...",
  "timezone": "America/New_York",
  "gradient": "grape-sky",
  "cycleSettings": {
    "questionsPerCycle": 5,
    "votesPerUserPerCycle": 3,
    "responseWindowDays": 4
  },
  "notificationSettings": {
    "offsetsHoursBeforeClose": [96, 48, 24],
    "onCycleOpen": true
  },
  "memberSoftCap": 50
}
```

`gradient` must be one of the six preset palette slugs: `grape-sky`, `ember-rose`, `forest-mint`, `ocean-dusk`, `citrus-blush`, `slate-lilac` (canonical home: an enum in `shared/openapi.yaml`, mirrored in `backend/crates/shared/src/config.rs` and the frontend palette table).

Validation: `responseWindowDays` between 1 and 28; `questionsPerCycle` between 1 and 20; `votesPerUserPerCycle` between 1 and `questionsPerCycle` (each `(voter, question)` pair is unique, so the user can never cast more upvotes than there are candidate questions to vote on); `timezone` must be a valid IANA TZ; `offsetsHoursBeforeClose` non-empty, descending, integers ≥1 and ≤168 (`MAX_REMINDER_OFFSET_HOURS` in `shared/src/config.rs` — the notify-tick's query window is derived from this cap, so an uncapped offset would silently never fire; see `07-notifications.md` §7.1).

---

## 5. Newsletter routes

### 5.1 `GET /groups/{groupId}/newsletters`

**Lambda**: `lambda-newsletters`. **Role**: member.

Query: `?status=voting|open|published&limit=&cursor=`

Returns past + current + upcoming newsletters for the group, newest first. Uses AP7.

Response 200:
```json
{
  "items": [
    {
      "cycleId": "202605",
      "status": "open",
      "responseOpenAt": "...",
      "responseCloseAt": "...",
      "publishedAt": null,
      "questionCount": 5,
      "myDraftCount": 2,
      "myPublishedCount": 0
    }
  ],
  "nextCursor": null
}
```

Implementation note: `myDraftCount`/`myPublishedCount` are computed via AP15 (one Query per cycle returned). Cap `limit` at 24 to keep this bounded.

The API does NOT return month-name or year labels. Clients derive display strings (e.g. `"June"` / `"2026"`) from `responseOpenAt` using the group's `timezone`. The published timestamp (`publishedAt`) and `responseOpenAt` together let the UI compute every label it needs.

### 5.2 `GET /groups/{groupId}/newsletters/{cycleId}`

**Lambda**: `lambda-newsletters`. **Role**: member.

Returns the full edition. Behavior depends on status:

- `voting` → `{ status: "voting", candidates: [...] }` — see §6 candidate question shape (admin-only fields stripped).
- `open` → questions list + the caller's own drafts (only). Other users' answers omitted.
- `published` → questions list with all published answers, comments, and reactions hydrated; poll questions include vote tallies.
- `archived` → 410 Gone with link header to archive blob (per `10-archival.md` §4).

Response 200 (published):
```json
{
  "cycleId": "202605",
  "status": "published",
  "responseCloseAt": "...",
  "publishedAt": "...",
  "questions": [
    {
      "questionId": "01H...",
      "kind": "text",
      "prompt": "What's your favorite hike of the year?",
      "displayOrder": 0,
      "askedBy": { "userId": "01H...", "displayName": "Kari" },
      "isAnonymous": false,
      "answers": [
        {
          "responseId": "01H...",
          "userId": "01H...",
          "displayName": "Quinn",
          "body": "...markdown...",
          "images": [{ "imageId": "01H...", "displayUrl": "https://cdn.../img/...", "thumbUrl": "...", "width": 1200, "height": 800 }],
          "publishedAt": "...",
          "comments": [{ "commentId": "01H...", "authorUserId": "01H...", "displayName": "...", "body": "...", "image": null, "createdAt": "...", "editedAt": null, "deletedAt": null }],
          "reactionGroups": [{ "emoji": "🔥", "count": 3, "reactedByMe": true }]
        }
      ]
    },
    {
      "questionId": "01H...",
      "kind": "poll",
      "prompt": "Which trail had the best views?",
      "askedBy": null,
      "isAnonymous": true,
      "options": [
        { "optionId": "01H...", "label": "Mt Si", "voteCount": 4 },
        { "optionId": "01H...", "label": "Lake 22", "voteCount": 7 }
      ],
      "myVoteOptionId": "01H..."
      // Poll questions do not carry top-level comments. Comments are always
      // attached to a specific text answer (see §8). Poll widgets are
      // comment-free in v1.
    }
  ]
}
```

Image URLs are absolute CloudFront URLs. The frontend MUST first call `/media-cookie` (§9.5) to receive signed cookies before requesting any image.

**M5 implementation note — `open` response shape** (not otherwise pinned down above): each locked question carries `questionId`, `kind`, `prompt`, `displayOrder`, `askedBy` (redacted per the same anonymity rule as candidates), `isAnonymous`, `pollOptions` (poll questions only), and `myResponse` — `null` if the caller has no draft yet, else `{ responseId, status, body, imageMediaIds, pollOptionId, updatedAt, publishedAt }`. See `domain::api::OpenQuestionResponse` for the exact shape; `shared/openapi.yaml` is authoritative.

**M5 implementation note — admin candidate delete and cycle resolution** (§6.5): the route carries no `cycleId`, but a `CandidateQuestion` row is stored under its cycle's key. `lambda-questions` resolves this by checking, in order: (1) the group's current `voting` cycle's candidate pool — if found there, it's still deletable; (2) the group's current `open` cycle's locked questions — a match there means the candidate was promoted between when the admin loaded the list and when the delete arrived, so it returns `CANDIDATE_PROMOTED`; (3) the `open` cycle's own (unpromoted, leftover) candidate rows, still deletable; otherwise `NOT_FOUND`. A group has at most one cycle in each of those two statuses at a time, so this fully covers the promotion race without needing a `cycleId` on the route.

---

## 6. Candidate question routes (voting pool)

### 6.1 `GET /groups/{groupId}/candidate-questions`

**Lambda**: `lambda-questions`. **Role**: member.

Returns the candidate pool for the *next* cycle (the one that will open next). Server determines which cycle is "next" by querying the group's newsletters.

Query: `?sort=top|recent&limit=&cursor=`

Response 200:
```json
{
  "nextCycleId": "202606",
  "votesPerUserPerCycle": 3,
  "myVoteCount": 2,
  "items": [
    {
      "questionId": "01H...",
      "kind": "text",
      "prompt": "...",
      "askedBy": { "userId": "01H...", "displayName": "Kari" },
      "isAnonymous": false,
      "voteCount": 7,
      "votedByMe": true,
      "submittedAt": "..."
    },
    {
      "questionId": "01H...",
      "kind": "poll",
      "prompt": "...",
      "pollOptions": [{ "optionId": "01H...", "label": "..." }],
      "askedBy": null,
      "isAnonymous": true,
      "voteCount": 5,
      "votedByMe": false,
      "submittedAt": "..."
    }
  ],
  "nextCursor": null
}
```

`askedBy`, when populated, has shape `{ userId, displayName, avatarColor, avatarUrl }` — mirroring the member-listing avatar fields so the UI can render a small avatar swatch next to the asker. It's populated when `isAnonymous=false`, OR whenever the caller is a group admin (admins always see authorship for moderation). When `isAnonymous=true` and the caller is a non-admin, `askedBy` is `null`. The internal `submittedBy` userId attribute is never returned directly — it's only surfaced through the redacted `askedBy` object.

`sort=top` uses AP11 (GSI1, ScanIndexForward=false). `sort=recent` uses AP10 with sort by `submittedAt`. Default (when `sort` is omitted) is `top`. **M5 implementation note:** since a group's candidate pool is bounded by its member count (soft-capped, typically ≤50), `lambda-questions` fetches the full pool via AP10 and sorts/pages it in memory rather than threading AP11's GSI1 cursor — both sort orders and the `cursor`/`limit` contract behave identically to the client either way.

### 6.2 `POST /groups/{groupId}/candidate-questions`

**Lambda**: `lambda-questions`. **Role**: member.

Body:
```json
{
  "kind": "text",
  "prompt": "...",
  "pollOptions": null,
  "isAnonymous": false
}
```

Or for polls:
```json
{
  "kind": "poll",
  "prompt": "...",
  "pollOptions": [
    { "label": "Mt Si" },
    { "label": "Lake 22" }
  ],
  "isAnonymous": false
}
```

Validation: prompt 5–500 chars. Poll: 2–6 options, each label 1–80 chars, no duplicate labels. `isAnonymous` is optional and defaults to `false` (the question will be attributed to the submitter); set to `true` to hide authorship from non-admin members. The submitter's `userId` is always recorded server-side regardless of `isAnonymous`. The flag is fixed at creation and cannot be changed later.

Response 201: full candidate item, with `askedBy` populated (the submitter is always allowed to see their own attribution, even when `isAnonymous=true`).

Errors: `VALIDATION_FAILED`, `CYCLE_NOT_VOTING` (if no eligible "next" cycle exists — e.g., the upcoming cycle has already locked).

### 6.3 `POST /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member.

Casts an upvote. Idempotent (re-POSTing returns 200 with current state).

Implementation: `TransactWriteItems` per `02-data-model-dynamodb.md` §4 transaction #1. The cap is enforced in the transaction by the voter's `VoterTally` counter (`02` §2.7a), so concurrent votes can't exceed `votesPerUserPerCycle`. A pre-read of the caller's votes supplies the error detail. If the cap is exceeded, returns `VOTE_CAP_REACHED` and lists the questions the user has already voted on. **M5 implementation note:** that list is carried in the problem body's free-text `detail` field (comma-separated question IDs), not a new structured field — `03-api-contract.md` §1.1's Problem Details shape only has `fieldErrors` for per-field validation issues, which doesn't fit a cap-conflict error.

Response 200:
```json
{ "questionId": "...", "voteCount": 8, "votedByMe": true, "myVoteCount": 3 }
```

### 6.4 `DELETE /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member. Withdraws the caller's upvote (transaction #2).

Response 200: same shape as 6.3.

### 6.5 Admin moderation routes

There is **no admin curate / promote / override** surface in v1. The cycle tick is the source of truth for which candidates promote, and locked questions are immutable. See `06-newsletter-lifecycle.md` §7. The only admin-side mutation on candidates:

- `DELETE /admin/groups/{groupId}/candidate-questions/{questionId}` — delete a candidate while it's still in `voting` (e.g., abusive). Removes votes too via batch delete. Returns 409 `CANDIDATE_PROMOTED` if the candidate has already been locked into a cycle.

**M5 implementation note — vote cascade:** `02-data-model-dynamodb.md`'s access-pattern list has no "list voters of question X" pattern (`CandidateVote` is keyed by voter, not by question), so there is no efficient query for "every vote on this candidate." Scanning the whole table for one admin action would be worse. `lambda-questions` instead iterates the group's membership list (AP3, bounded by `memberSoftCap`) and issues one best-effort delete per member against that member's `(cycle, questionId)` vote key — deleting an absent item is a no-op, so no existence check is needed first. This is O(members) requests on a rare, admin-only action, not a hot path.

---

## 7. Response routes (drafts + publish)

### 7.1 `GET /groups/{groupId}/newsletters/{cycleId}/my-responses`

**Lambda**: `lambda-responses`. **Role**: member. Lists caller's drafts/publishes for the cycle. AP15.

Response 200: `{ "items": [Response] }`

### 7.2 `GET /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response`

**Lambda**: `lambda-responses`. AP16. Returns 404 if caller has no draft yet.

### 7.3 `PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response`

**Lambda**: `lambda-responses`. The autosave/save-draft endpoint.

Body (text):
```json
{
  "kind": "text",
  "body": "...markdown...",
  "imageMediaIds": ["01H...", "01H..."],
  "publish": false
}
```

Image captions are stored on the `ImageMedia` row itself (set via `PATCH /uploads/{imageId}`, see §9.6) — they are not part of the response save payload. This keeps a caption attached to its image across edits and means an image's caption is the same wherever the image appears.

Body (poll):
```json
{
  "kind": "poll",
  "pollOptionId": "01H...",
  "publish": true
}
```

Behavior:
- Cycle must be in status `open` (else `CYCLE_NOT_OPEN`). Once the cycle has transitioned to `published`, this endpoint refuses all writes — including poll vote changes — and the SPA hides edit affordances accordingly.
- "Open" means `status = open` **and** `now < responseCloseAt`. The second clause covers the ≤5-minute gap before the cycle tick flips the status, so a save that lands just past the deadline is refused even though the row still says `open`.
- If `publish=true`, status moves to `published` and `publishedAt` is set. With `publish=false`, a new response is saved as `draft`.
- **Publishing is sticky.** Once a response is `published`, later saves with `publish=false` still update its content but leave `status=published` and the original `publishedAt` untouched. Autosaves of an edited published answer must not unpublish it, and this keeps the `editionsAnswered` count from being bumped twice (`02` §4 #4). There is no unpublish in v1.
- `responseId` is assigned on the first save and stays the same across later saves.
- Every save is one transaction: the response write, a condition check on the Newsletter row (`status = open AND responseCloseAt > now`), and the `editionsAnswered` bump on the first publish (`02` §4 #4). If the tick wins that race, the save returns 409 `CYCLE_NOT_OPEN`.
- The kind in the body must match the question's kind (else `VALIDATION_ERROR`). Image IDs are de-duplicated before the ≤10 check. Failing that check returns `IMAGE_LIMIT_EXCEEDED`. Any image that fails the ownership/`ready`/cycle/purpose rules returns `VALIDATION_ERROR` naming the image. The save does not modify `ImageMedia` rows; linking an image to a question arrives with the media pipeline in M8.
- **Concurrency: last-write-wins.** Cross-device editing is rare in this app and the autosave debounce (≥1500 ms after the last keystroke) makes interleaved saves unlikely. We skip optimistic-concurrency tokens and conflict-resolution UI in v1; whichever save lands last is the canonical state. Revisit if real-world telemetry shows clobbering.
- Validation: text body 0–20000 chars, ≤10 image IDs, all referenced ImageMedia must (a) belong to caller, (b) be in `ready` status, (c) reference this `groupId`+`cycleId`, (d) have `purpose: "response"`.
- For polls: `pollOptionId` must be in question's options. Last-write-wins.

Response 200: full updated `Response`.

### 7.4 Auto-republish after edit

After the cycle has closed (status `published`), responses are immutable. Attempts to `PUT` return 409 `CYCLE_NOT_OPEN`. The frontend hides edit affordances accordingly.

---

## 8. Engagement routes (comments + reactions)

All require the cycle to be `published`. Pre-publish, this surface is hidden in the UI.

**Addressing (decided M10).** Every engagement route is nested under the answer's question: `/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/...`. Comment and reaction rows are keyed on `(questionId, answerUserId)` (`02` §2.11–§2.12), and a bare `responseId` has no index, so it couldn't be resolved without reading every question's answers. With `questionId` in the path, the handler queries that question's answers (`pk=GROUP#g#NL#c#Q#q`, `begins_with(sk, A#)`, at most one row per member) and takes the row whose `response_id` matches. That row must be a published **text** answer; otherwise the route returns 404. Poll answers are never engagement targets (`09` §3).

**Check order on all seven routes (GETs included).** Each check runs only if the previous one passed:
1. Membership: 403 if the caller isn't a member.
2. The cycle exists: 404 if not.
3. The cycle isn't `archived`: 410 `NEWSLETTER_ARCHIVED` if it is.
4. The cycle is `published`: 409 `CYCLE_NOT_PUBLISHED` if not.
5. The answer resolves: 404 if not.
6. Route-specific checks.

**Comment shape.** `CommentResponse` (`shared/openapi.yaml`) is `{commentId, authorUserId, displayName, body, image, createdAt, editedAt, deletedAt}`, and it is used both here and inline in §5.2. `displayName` is the author's account display name. `image` is `{imageId, displayUrl, thumbUrl, width, height, caption}` while the attached image is `ready`, and null otherwise. Soft-deleted comments are **returned** as placeholders, with `deletedAt` set, `body` empty and `image` null, so the UI can render `[deleted]` in place.

### 8.1 `GET .../questions/{questionId}/responses/{responseId}/comments`

**Lambda**: `lambda-engagement`. AP18. Paginated (`limit` default 50, max 100), oldest first. Returns `CommentListResponse {items, nextCursor}`.

### 8.2 `POST .../questions/{questionId}/responses/{responseId}/comments`

Body: `{ "body"?: string, "imageMediaId"?: string|null }`. `body` 0–2000 chars. `imageMediaId` must reference an image that is all of the following:
- `ready`
- owned by the caller
- uploaded for this group, cycle and question with `purpose: "comment"` (§9.1)
- not attached to another comment

A whitespace-only or missing `body` with no image is a 422. All failures are `VALIDATION_FAILED` with `fieldErrors` on `body` or `imageMediaId`. See `09-engagement.md` §1.3.

Response 201: the created `CommentResponse`.

**Image attachment (decided M10).** `ImageMedia` gains `attached_comment_id` (`02` §2.10). Writing the comment and claiming the image happen in one `TransactWriteItems`, conditioned on the image being unclaimed (or already claimed by this comment). The same applies to changing or removing an image on PATCH and to soft delete, which also releases the old image. §9.4 uses the claim for its in-use check.

### 8.3 `PATCH .../comments/{commentId}`

Body: `{ "body"?: string, "imageMediaId"?: string|null }`. Author only: admins can't edit other people's comments, and get 403. An absent field is left unchanged. `"imageMediaId": null` removes the attachment. The result must still have a non-whitespace body or an image. Sets `editedAt`. A missing or soft-deleted comment returns 404. Response 200: the updated `CommentResponse`.

### 8.4 `DELETE .../comments/{commentId}`

Soft delete: sets `deletedAt`, blanks `body` and drops the image attachment, releasing the image's claim. The row remains, so the parent thread's ordering is preserved and moderation history survives. Allowed for the author or a group admin. Idempotent: a `DELETE` of an already-soft-deleted row returns 204, and a `DELETE` of a missing row returns 404.

### 8.5 `GET .../questions/{questionId}/responses/{responseId}/reactions`

**Lambda**: `lambda-engagement`. AP19. Returns `ReactionsResponse`: the grouped form (`reactionGroups`) plus the caller's own emoji (`myReactions`). **Group order (decided M10):** count descending, then earliest reaction first, then emoji. The inline `reactionGroups` in §5.2 use the same order (one shared function).

### 8.6 `PUT .../reactions/{emoji}`

Idempotent toggle-on with an empty body. `emoji` is URL-encoded. The server percent-decodes it if it still contains `%`, because API Gateway's decoding of path parameters isn't relied on. It then validates it against the canonical emoji predicate in `09-engagement.md` §2.2: NFC-normalized, 1–12 codepoints, and at least one `Extended_Pictographic` or regional-indicator codepoint. Anything else is rejected with `VALIDATION_FAILED`. Encoding the emoji in the path simplifies the `R#{user}#{emoji}` sort key. Response 200: the answer's `ReactionsResponse` after the change.

### 8.7 `DELETE .../reactions/{emoji}`

Removes the caller's reaction with that emoji. Idempotent, with the same validation as PUT. Response 200: the answer's `ReactionsResponse` after the change.

---

## 9. Media routes

### 9.1 `POST /uploads`

**Lambda**: `lambda-media`. Returns a pre-signed S3 PUT URL the client uses to upload directly.

Body:
```json
{
  "groupId": "01H...",
  "cycleId": "202605",
  "questionId": "01H...",
  "purpose": "response",     // "response" (default) | "comment" — see 08-media-uploads.md §3.1
  "mimeType": "image/jpeg",
  "byteSize": 4823920,
  "sha256": "n4bQgYhM..."    // optional; standard base64 of the SHA-256 digest (44 chars)
}
```

`sha256`, when sent, is signed into the presign as `x-amz-checksum-sha256`, so S3 rejects a body whose digest doesn't match. (Decided in M8: the earlier draft said "base64url" and implied `x-amz-content-sha256`, but presigned PUTs can't use that header for payload integrity; S3's flexible-checksum header needs standard base64.)

Validation: `mimeType` ∈ {`image/jpeg`, `image/png`, `image/webp`, `image/gif`} (else 415 `IMAGE_BAD_TYPE` — an unknown type such as SVG is a 415, not a 422), 1 ≤ `byteSize` ≤ 15728640 (over → 413 `IMAGE_TOO_LARGE`), caller is member of group, `questionId` is a locked question of the cycle (else 422). Cycle-status check depends on `purpose`: `"response"` requires the cycle to be `open`; `"comment"` requires it to be `published` (comments only exist post-publish — this is the upload path for comment attachments, `09-engagement.md` §1.3). The 10-image cap applies to `purpose=response` uploads only.

Server creates an `ImageMedia` row with `status=pending` and an S3 key:
```
uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}
```

Response 201:
```json
{
  "imageId": "01H...",
  "uploadUrl": "https://...s3.amazonaws.com/...",
  "headers": { "content-type": "image/jpeg", "x-amz-checksum-sha256": "..." },
  "expiresInSeconds": 600
}
```

The frontend then `PUT`s the binary directly to `uploadUrl` with **every** listed header, verbatim. `headers` is whatever the presigned request was signed with (lower-case names).

### 9.0 Addressing an existing upload (decided M8)

An `ImageMedia` row is keyed by `(groupId, cycleId, imageId)` (`02` §2.10), so every `/uploads/{imageId}` route (§9.2–§9.4, §9.6) takes **required `groupId` and `cycleId` query parameters**. Without them the lookup would need an unbounded scan of the caller's GSI1 uploads. The caller must be a member of `groupId` (403 otherwise). `GET` and `/complete` are open to any member; `PATCH` and `DELETE` require the caller to own the image (403 otherwise).

Every one of these routes that returns a body returns `ImageMediaResponse` (`shared/openapi.yaml`): the row's fields in camelCase, minus the S3 keys, plus absolute `displayUrl`/`thumbUrl` (null until `ready`) and `errorMessage` (`IMAGE_TOO_LARGE`, `IMAGE_DECODE_FAILED` or `DELETED` when `failed`).

### 9.2 `POST /uploads/{imageId}/complete`

**Lambda**: `lambda-media`. Optional convenience endpoint — the S3 trigger does the same work. Returns the current `ImageMedia` status. Used by the client to short-circuit polling on slow networks.

### 9.3 `GET /uploads/{imageId}`

**Lambda**: `lambda-media`. Returns the `ImageMedia` row (status, dimensions, processed URLs). Frontend polls this every ~1.5s after upload until `status=ready`.

### 9.4 `DELETE /uploads/{imageId}`

**Lambda**: `lambda-media`. Detach + mark for cleanup. Allowed only when image is not referenced by any published response: if the caller's answer to the image's question is published and lists the image, 409 `IMAGE_IN_USE`. Otherwise soft-marks the row `failed` with `errorMessage: "DELETED"` and returns 204; the S3 lifecycle handles real deletion. Idempotent. A draft that still lists a deleted image fails its next save's image validation, so the client drops the ID when it deletes. A `purpose: "comment"` image that a live comment has claimed (`attached_comment_id` set, §8.2) is also 409 `IMAGE_IN_USE` (added M10).

### 9.5 `GET /media-cookie`

**Lambda**: `lambda-media`. Returns CloudFront signed cookies scoped to a single group. Cookies last 1 hour and cover paths matching `/img/{groupId}/*`.

Response 200 sets cookies via `Set-Cookie` headers (`CloudFront-Policy`, `CloudFront-Signature`, `CloudFront-Key-Pair-Id`) AND returns a JSON body with the same values for clients that can't read response cookies cross-origin:

```json
{
  "policy": "...",
  "signature": "...",
  "keyPairId": "...",
  "expiresAt": "2026-06-02T11:00:00Z"
}
```

`expiresAt` is an RFC 3339 timestamp like every other API timestamp (the epoch seconds live inside the policy). The `Set-Cookie` headers are emitted only when the Lambda has a cookie domain configured (`MEDIA_COOKIE_DOMAIN`, set in prod to `.{domain}`); dev returns the body alone. Cookies use `Path=/img/{groupId}/` and `Max-Age=3600`, so cookies for several groups coexist in the browser.

For the SPA in **prod**, the cookies are sufficient (the CloudFront domain is on the same registrable domain as the API via the `cdn.` subdomain — see DNS plan). In **dev** there is no shared registrable domain (raw AWS endpoints), so cookies cannot flow — the SPA instead appends the JSON-body values as CloudFront signed-URL query params to each image URL. See `08-media-uploads.md` §4.5.

### 9.6 `PATCH /uploads/{imageId}`

**Lambda**: `lambda-media`. **Role**: caller must own the image.

Body: `{ "caption": "Cocoa thermos at mile two." | null }` (≤140 chars after trimming; `null` or an all-whitespace string clears it).

Updates the `ImageMedia.caption` field. Allowed regardless of whether the image is referenced by a draft or published response — the caption is a property of the image, not of a particular usage.

Response 200: updated `ImageMedia`.

### 9.7 Avatar routes

Avatars use a separate pipeline (see `08-media-uploads.md` §11) so they can cross group boundaries.

- `POST /avatars` — request a pre-signed PUT URL. Body: `{ "mimeType": "image/jpeg|png|webp", "byteSize": ..., "sha256"?: ... }`. Validation: `byteSize` ≤ 5 MB (else 413), MIME as listed (GIF → 415). Server creates an `AvatarMedia` row (`status=pending`) and returns `{ "avatarId", "uploadUrl", "headers", "expiresInSeconds": 600 }`.
- `GET /avatars/{avatarId}` — returns `AvatarMediaResponse` (status, `avatarUrl` once ready, `errorMessage`). Looked up under the caller's own user partition, so another user's avatar is a 404. Frontend polls until `status=ready`, then issues `PATCH /me` with the new `avatarMediaId`.
- `DELETE /avatars/{avatarId}` — caller must own (404 otherwise). Marks the row `failed`/`DELETED`, idempotently; if the avatar is the caller's current `avatarMediaId`, the User row is also cleared back to `null` (the deterministic `avatarColor` takes over for rendering).

Avatar URLs are absolute CloudFront URLs under `https://cdn.{domain}/avatar/{avatarId}/display.webp`. No signed cookies required — see `02-data-model-dynamodb.md` §2.16.

---

## 10. Push routes

### 10.1 `POST /push/subscribe`

**Lambda**: `lambda-push`.

Body: full PushSubscription JSON from the browser (`endpoint`, `keys: { p256dh, auth }`, `userAgent`).

Behavior: hash endpoint, upsert by `(userId, endpointHash)`. Resets `failureCount=0`.

Response 201: `{ "subscriptionId": "..." }`.

### 10.2 `POST /push/unsubscribe`

Body: `{ "endpoint": "..." }`. Deletes the matching row.

Response 204.

### 10.3 `GET /push/subscriptions`

Returns the caller's active subs (for a settings page where they can prune devices).

### 10.4 `POST /push/test`

Sends a test push to all of the caller's subscriptions. Useful from the Settings page. Response 200 with delivery results.

### 10.5 `PUT /push/preferences/{groupId}`

Body: `{ "cycleOpen": true, "deadlineReminders": true }`. Upserts a `NotificationPref` row. Publication notifications are always-on with no per-user toggle; the API rejects any `publication` field in the body with `VALIDATION_FAILED`.

---

## 11. Health

### 11.1 `GET /healthz`

**Lambda**: `lambda-groups`. **Auth**: required (so it stays inside the protected stage; idle backend principle).

Response 200: `{ "status": "ok", "version": "git-sha", "buildAt": "..." }`. `version` and `buildAt` are baked in at compile time from `BUILD_SHA`/`BUILD_AT`, which `make build-lambdas` sets. A local build reports `"unknown"` and `null`.

---

## 11a. Dev-only endpoints

These routes are wired into the API only when `ENV=dev` and refuse with `404 NOT_FOUND` in any other environment. They exist so manual testing and Playwright E2E can fast-forward state without waiting for EventBridge. See `13-dev-environments.md` §9.

### 11a.1 `POST /admin/dev/tick/cycle`

**Lambda**: `lambda-cycle-tick`. **Auth**: required. **Role**: any group admin.

Synchronously invokes the cycle-tick handler. Optional body `{ "groupId"?: "01H...", "advanceCycleClosesBy"?: "5m"|"1h"|... }`. When `advanceCycleClosesBy` is present, `groupId` is **required** and the caller must be an admin of that group: the group's active cycle has its `nextTransitionAt` (and the matching `responseOpenAt`/`responseCloseAt` field) rewound by the given duration before the tick runs, making the transition due immediately. The duration is an integer plus `m`/`h`/`d`, between 0 and 366 days; anything else is `VALIDATION_FAILED` with a `fieldErrors` entry for `advanceCycleClosesBy`. With an empty body the tick just runs globally; the role check is "caller is an admin of at least one group."

"The group's active cycle" means: the `open` cycle if one exists (its `responseCloseAt` is rewound, fast-forwarding toward publication — the common E2E case), else the `voting` cycle (its `responseOpenAt`/`voteWindowCloseAt` is rewound, fast-forwarding toward promotion). A group always has at most one of each, so this is unambiguous.

Response 200: `{ "transitions": [{ "groupId", "cycleId", "from", "to" }] }`. `from`/`to` are one of `voting`/`open`/`published`. A freshly created next-cycle row (§5.4) appears in the same array with `"from": null, "to": "voting"` rather than in a separate list — one array covers every state change a tick made, created-from-nothing included.

### 11a.2 `POST /admin/dev/tick/notify`

**Lambda**: `lambda-notify-tick`. **Auth**: required. **Role**: any group admin.

Synchronously invokes the notify-tick handler. Returns `{ "fanouts": [{ "groupId", "cycleId", "kind", "delivered", "failed" }] }`.

Both routes are also called by Playwright E2E (`11-testing-ci-cd.md` §4.3) — implement once, two consumers.

---

## 12. OpenAPI generation

`shared/openapi.yaml` is hand-maintained as the canonical contract; codegen targets:

- TypeScript types → `frontend/src/types/api.ts` via `openapi-typescript` (driven by `scripts/codegen_types.sh`)
- Rust request/response types → `backend/crates/domain/src/api.rs`, hand-written serde structs that mirror the YAML. Prefer this over `progenitor`: the structs are small, and generated Rust would fight the newtype IDs in `domain/ids.rs`. `11-testing-ci-cd.md` §2.3 is the contract test that keeps them honest.

A schema validation test in CI rejects any drift between handwritten Rust models and the YAML.

All of this now exists (landed in M5, `12-build-order.md` M5 "OpenAPI contract"). M4's wire types, formerly in per-crate `dto.rs` files, now live in `backend/crates/domain/src/api.rs`; `lambda-groups` and `lambda-invites` import them from there. A route added in a later milestone updates the YAML in the same PR (`coding-standards.md` §8).

---

## 13. Per-route auth/role summary table

(Keep this synced with code; see `12-build-order.md` Milestone "API skeleton" for the test that enforces this.)

| Method + Path | Role |
|---|---|
| `GET /config`, `GET /me`, `PATCH /me` | authenticated |
| `POST /admin/invites` | admin of {groupId in body} |
| `GET /admin/groups/{g}/invites`, `POST /admin/invites/{code}/revoke` | admin |
| `POST /invites/redeem` | authenticated |
| `GET /groups`, `GET /groups/{g}` | member |
| `PATCH /groups/{g}` | admin |
| `DELETE /groups/{g}/members/{u}` | self OR admin |
| `PATCH /groups/{g}/members/{u}` | admin (role only) |
| `GET /groups/{g}/newsletters*` | member |
| `*` candidate-questions GET/POST/votes | member |
| `DELETE /admin/groups/{g}/candidate-questions/{q}` | admin |
| `*` my-response routes | member (and self) |
| `*` comments/reactions GET/POST, reactions PUT/DELETE | member |
| `PATCH` comment | author only |
| `DELETE` comment | author OR admin |
| `POST /uploads*`, `GET /uploads/{id}`, `PATCH /uploads/{id}`, `DELETE /uploads/{id}`, `GET /media-cookie` | member (owner for PATCH/DELETE) |
| `POST /avatars`, `GET /avatars/{id}`, `DELETE /avatars/{id}` | self (owner) |
| `*` push routes | self |
| `GET /healthz` | authenticated |
| `POST /admin/dev/tick/*` | admin (dev environment only; 404 in prod) |
