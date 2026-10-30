# API

This is the full list of every HTTP endpoint OpenNewsletter exposes: handler Lambda (matching [`infrastructure.md` §6.2](infrastructure.md#62-lambda-functions-request-handlers)), auth/role requirement, request/response shapes, status-dependent behavior, error codes, and the access-pattern numbers from [`data-model.md`](data-model.md). It's for anyone adding or changing a route, on either the backend or the frontend. **`shared/openapi.yaml` is the source of truth for wire shapes** — it covers every route below and is what gets codegen'd into TS types and Rust route stubs (§12); on any drift between the two, the YAML wins. This page is the source of truth for semantics, auth and error behavior: validation rules, status-dependent behavior, and what each error code means.

---

## 1. General conventions

- Base URL: `https://api.opennewsletter.example.com` (prod). Dev has no custom domains — the raw `https://{apiId}.execute-api.us-east-1.amazonaws.com` endpoint is used (see [`environments.md` §2](../development/environments.md#2-environments)).
- Content type: `application/json` (UTF-8) for everything except media uploads (which go directly to S3).
- Auth: All routes require a Cognito JWT in `Authorization: Bearer {idToken}` UNLESS marked **Public**. There are no Public routes — the OAuth callback is a Cognito-managed redirect, not an app endpoint. **The ID token, not the access token**: `POST /invites/redeem` creates the caller's `User` row from the `email` and `name` claims, which only the ID token carries ([`auth.md` §4.2](auth.md#42-algorithm)). The JWT authorizer's audience is the frontend client ID, which matches the ID token's `aud`.
- Path tenancy: Routes that operate inside a group are prefixed `/groups/{groupId}/...`. The handler verifies caller membership before doing anything (per [`data-model.md` §6](data-model.md#6-tenant-isolation-rule)).
- Correlation: Clients SHOULD send `x-correlation-id: {ulid}`. If absent the API generates one. It is echoed in responses and logged.
- Pagination: List endpoints accept `?cursor={opaque}&limit={1..100}`. Responses include `nextCursor` (null when exhausted). Cursor is base64url(JSON) encoding DynamoDB's `LastEvaluatedKey`. A cursor that decodes but doesn't address a row in the listing's own partition and key range (e.g. one taken from another answer's comment list) is a 422 on `cursor`, the same as a malformed one (`persistence::cursor::decode_scoped`).
- Timestamps: All ISO-8601 UTC.
- IDs: All UUIDv7 strings except `cycleId` (yyyymm) and `inviteCode` (16-char Crockford base32).

### 1.1 Error format

RFC-7807 Problem Details: `ProblemDetails` in `shared/openapi.yaml` (`type`, `title`, `status`, `detail`, `code`, `correlationId`, plus an optional `fieldErrors: FieldError[]`).

`fieldErrors` is present only on `VALIDATION_FAILED` responses whose problem is one named request field. `field` is the camelCase wire name (query parameters included, e.g. `limit`, `cursor`). Nested request fields use their bare name too (`responseWindowDays`, not `cycleSettings.responseWindowDays`). `code` is a `FieldErrorCode` (`shared/openapi.yaml`, mirrored by `domain::FieldErrorCode`). It is `INVALID` except where a client must react to the specific reason. The one such code so far is `CLOSES_IN_PAST`, for a `responseWindowDays` change that would close the open cycle in the past (§4.3). Clients treat an unknown code as `INVALID`. `message` repeats `detail`. It is omitted for validation failures not tied to one field, such as malformed JSON or a missing `email` claim.

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
| `LAST_ADMIN` | 409 | Can't remove or demote the group's only admin, or a concurrent admin change won the race (reload and retry; [`data-model.md` §4](data-model.md#4-transactional-invariants) #7) |
| `CANDIDATE_PROMOTED` | 409 | Candidate already locked into a cycle; can no longer be deleted |
| `NEWSLETTER_ARCHIVED` | 410 | Cycle has transitioned to `archived`; see [`archival.md`](archival.md) (not reachable until archival ships) |
| `IMAGE_LIMIT_EXCEEDED` | 409 | More than 10 images attached |
| `IMAGE_TOO_LARGE` | 413 | Original > 15MB |
| `IMAGE_BAD_TYPE` | 415 | MIME type not allowed |
| `IMAGE_IN_USE` | 409 | `DELETE /uploads/{imageId}` on an image the caller's published answer still references, or that a live comment has attached |
| `INVITE_EMAIL_MISMATCH` | 403 | `POST /invites/redeem` on an invite with `boundEmail` set, where the ID token's `email` (lowercased) doesn't match it or `email_verified` is false ([`data-model.md` §2.4](data-model.md#24-invite)) |
| `RATE_LIMITED` | 429 | Throttled |
| `INTERNAL` | 500 | Generic server error |

---

## 2. Bootstrap / metadata routes

### 2.1 `GET /config`

**Lambda**: `lambda-groups`. **Auth**: required. **Purpose**: bootstrap data the SPA needs after login.

Response 200: `ConfigResponse` in `shared/openapi.yaml`.

`memberships` is a denormalized join. Each entry includes `groupName`, `timezone`, and `gradient`, so the frontend can render the group switcher (gradient swatch) and label cycles in the group's timezone ([`frontend.md` §14a](frontend.md#14a-client-derived-display-data-not-api-fields)) without a follow-up query per group. The handler reads all the group rows in one `BatchGetItem`.

### 2.2 `GET /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Returns the bare user profile (no memberships). Used by the settings page.

Response 200: `UserResponse` in `shared/openapi.yaml`.

### 2.3 `PATCH /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Body: `PatchMeRequest` in `shared/openapi.yaml`.

Validation: `displayName` 1–40 chars, non-whitespace. `avatarColor` one of the eight allowed slugs (canonical home: the `AvatarColorSlug` enum in `shared/openapi.yaml`, mirrored in `backend/crates/shared/src/config.rs` and the frontend palette table — the contract test in [`testing-and-ci.md` §2.3](../development/testing-and-ci.md#23-contract-test) keeps the Rust mirror honest). `avatarMediaId` (when non-null) must reference an `AvatarMedia` row owned by the caller in `status=ready`; otherwise `VALIDATION_FAILED`. Setting `avatarMediaId: null` clears the photo and the deterministic `avatarColor` takes over for rendering. Clearing permanently deletes the old avatar, the same way replacing it does (§9.7) — otherwise a client that cleared without calling `DELETE /avatars` would leave the old objects behind.

Response 200: `UserResponse`.

---

## 3. Invite & membership routes

### 3.1 `POST /admin/invites`

**Lambda**: `lambda-invites`. **Auth**: required. **Role**: must be admin of the target group.

Body: `CreateInviteRequest` in `shared/openapi.yaml`.

`boundEmail` restricts redemption to a caller whose ID-token `email` matches it (case-insensitively) and whose `email_verified` is true — see [`data-model.md` §2.4](data-model.md#24-invite) and §3.4 below. It's an admin-side option in general; the UI need not expose it (it's used by `scripts/bootstrap_admin.py`'s prod path, [`auth.md` §9](auth.md#9-bootstrap-admin--first-group)).

Response 201: `CreateInviteResponse`.

Errors: `FORBIDDEN`, `VALIDATION_FAILED`.

### 3.2 `GET /admin/groups/{groupId}/invites`

**Lambda**: `lambda-invites`. **Role**: admin.

List of invites for the group with status. Uses AP6.

Response 200: `InviteListResponse` in `shared/openapi.yaml`.

### 3.3 `POST /admin/invites/{code}/revoke`

**Lambda**: `lambda-invites`. **Role**: admin of the invite's group.

Marks status `revoked`. Idempotent.

Response 204.

### 3.4 `POST /invites/redeem`

**Lambda**: `lambda-invites`. **Auth**: required. **Role**: any authenticated user.

The single, canonical place where invites are consumed. It covers **both** cases: first signup (the handler lazily creates the `CognitoSubLookup` + `User` rows when the caller's `sub` has no `userId` yet) and an existing user joining an additional group. The `PreSignUp` trigger is a pass-through and never touches invites — see [`auth.md` §3](auth.md#3-presignup-lambda)–§4 for the full algorithm.

Body: `RedeemRequest` in `shared/openapi.yaml`.

Response 200: `RedeemResponse`.

Errors: `INVITE_INVALID`, `INVITE_EXPIRED`, `INVITE_CONSUMED`, `MEMBER_CAP_REACHED`, `INVITE_EMAIL_MISMATCH` (when the invite has `boundEmail` set and the caller's ID-token `email`/`email_verified` don't satisfy it; [`auth.md` §4.2](auth.md#42-algorithm)).

### 3.5 `DELETE /groups/{groupId}/members/{userId}` (leave or kick)

**Lambda**: `lambda-groups`. **Role**: self OR admin (admin can remove others; users can remove themselves).

Last admin can't leave — returns 409 `LAST_ADMIN`.

Response 204.

### 3.6 `PATCH /groups/{groupId}/members/{userId}` (role change)

**Lambda**: `lambda-groups`. **Role**: admin.

Body: `PatchMemberRequest` in `shared/openapi.yaml`.

Last-admin guard: demoting the only admin returns 409 `LAST_ADMIN`. Per-group nicknames are not a v1 feature; member display always reflects the user's account `displayName`.

---

## 4. Group routes

### 4.1 The caller's groups

There is no route that lists only the caller's groups. The list is the `memberships` array of `GET /config` (§2.1), and the SPA refetches `/config` when it changes, such as after redeeming an invite.

### 4.2 `GET /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: member.

Response 200: `GroupResponse` in `shared/openapi.yaml`, including `members: MemberResponse[]`. `avatarUrl` is a presigned S3 GET URL for the member's avatar (`avatar/{avatarId}/display.webp` in the processed bucket, signed by `media-urls` — [`media.md` §4](media.md#4-serving-presigned-s3-gets)) when they have one set, else `null`; `avatarColor` is the deterministic / chosen fallback color slug rendered as a colored initial when `avatarUrl` is null.

### 4.3 `PATCH /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: admin.

Body (all optional): `PatchGroupRequest` in `shared/openapi.yaml`.

`gradient` must be one of the six preset palette slugs (canonical home: the `GradientSlug` enum in `shared/openapi.yaml`, mirrored in `backend/crates/shared/src/config.rs` and the frontend palette table).

Validation: `responseWindowDays` between 1 and 28; `questionsPerCycle` between 1 and 20; `votesPerUserPerCycle` between 1 and `questionsPerCycle` (each `(voter, question)` pair is unique, so the user can never cast more upvotes than there are candidate questions to vote on); `timezone` must be a valid IANA TZ; `offsetsHoursBeforeClose` non-empty, descending, integers ≥1 and ≤168 (`MAX_REMINDER_OFFSET_HOURS` in `shared/src/config.rs` — the notify-tick's query window is derived from this cap, so an uncapped offset would silently never fire; see [`notifications.md` §7.1](notifications.md#71-algorithm)).

**`responseWindowDays` on a currently-`open` cycle**: the handler recomputes that cycle's `responseCloseAt = responseOpenAt + days` (and its GSI2 keys) only if the new close is still in the future. If the recomputed close would be in the past, the request is rejected with 422 `VALIDATION_FAILED` and a `fieldErrors` entry on `responseWindowDays` with code `CLOSES_IN_PAST`, rather than silently applying to a later cycle. Reminder markers already claimed for that cycle stay claimed — an extended window doesn't re-send a reminder already sent. The current `voting` cycle's `responseWindowDays` (and `questionsPerCycle`, `votesPerUserPerCycle`, `timezone`) also apply immediately, per [`lifecycle.md` §6](lifecycle.md#6-settings-changes-mid-cycle), which has the full per-field rules.

---

## 5. Newsletter routes

### 5.1 `GET /groups/{groupId}/newsletters`

**Lambda**: `lambda-newsletters`. **Role**: member.

Query: `?status=voting|open|published&limit=&cursor=`

Returns past + current + upcoming newsletters for the group, newest first. Uses AP7.

Response 200: `NewsletterListResponse` in `shared/openapi.yaml`.

Implementation note: `myDraftCount`/`myPublishedCount` are computed via AP15 (one Query per cycle returned). Cap `limit` at 24 to keep this bounded.

The API does NOT return month-name or year labels. Clients derive display strings (e.g. `"June"` / `"2026"`) from `responseOpenAt` using the group's `timezone`. The published timestamp (`publishedAt`) and `responseOpenAt` together let the UI compute every label it needs.

### 5.2 `GET /groups/{groupId}/newsletters/{cycleId}`

**Lambda**: `lambda-newsletters`. **Role**: member.

Returns the full edition. Behavior depends on status:

- `voting` → `{ status: "voting", candidates: [...] }` — see §6 candidate question shape (admin-only fields stripped).
- `open` → questions list + the caller's own drafts (only). Other users' answers omitted.
- `published` → questions list with all published answers, comments, and reactions hydrated; poll questions include vote tallies.
- `archived` → 410 Gone with link header to archive blob (per [`archival.md` §4](archival.md#4-read-path)).

**Read plan for `published`.** The handler reads in a fixed number of concurrent waves rather than per answer, so latency doesn't grow with group size. (An earlier version made about six sequential DynamoDB calls per answer; a 50-member group with image-heavy answers would have exceeded the 10 s Lambda timeout.) The waves are:
1. AP13, the locked questions.
2. AP14 for every question at once.
3. One engagement-partition read per published text answer ([`data-model.md` §3](data-model.md#3-access-patterns--index-map), AP18 + AP19 combined), 16 at a time (`EDITION_READ_CONCURRENCY`).
4. One `BatchGetItem` for every user profile and one for every image the edition references, run together.

The output is unchanged: question, answer, image and comment order, the skip rules for images that aren't `ready`, `[deleted]` placeholders and poll tallies are all as before.

Response 200 (published): `NewsletterDetailResponse` in `shared/openapi.yaml` (the `published`-status branch), nesting `PublishedQuestionResponse` → `PublishedAnswerResponse` → `CommentResponse`/`ReactionGroupResponse` for text questions, or `PublishedPollOptionResponse` + `myVoteOptionId` for polls. Poll questions carry no top-level comments — comments are always attached to a specific text answer (§8); poll widgets are comment-free in v1.

Image URLs are presigned S3 GET URLs, minted inline by the handler (§9.5); each `displayUrl`/`thumbUrl` is usable as soon as it's returned. See [`media.md` §4](media.md#4-serving-presigned-s3-gets).

**The `open` response shape** is `OpenQuestionResponse` (`shared/openapi.yaml`): each locked question carries `askedBy` redacted per the same anonymity rule as candidates, plus `myResponse` — `null` if the caller has no draft yet, else a `MyResponseSummary`.

**Admin candidate delete and cycle resolution** (§6.5): the route carries no `cycleId`, but a `CandidateQuestion` row is stored under its cycle's key. `lambda-questions` resolves this by checking, in order: (1) the group's current `voting` cycle's candidate pool — if found there, it's still deletable; (2) the group's current `open` cycle's locked questions — a match there means the candidate was promoted between when the admin loaded the list and when the delete arrived, so it returns `CANDIDATE_PROMOTED`; (3) the `open` cycle's own (unpromoted, leftover) candidate rows, still deletable; otherwise `NOT_FOUND`. A group has at most one cycle in each of those two statuses at a time, so this fully covers the promotion race without needing a `cycleId` on the route.

---

## 6. Candidate question routes (voting pool)

### 6.1 `GET /groups/{groupId}/candidate-questions`

**Lambda**: `lambda-questions`. **Role**: member.

Returns the candidate pool for the *next* cycle (the one that will open next). Server determines which cycle is "next" by querying the group's newsletters.

Query: `?sort=top|recent&limit=&cursor=`

Response 200: `CandidateListResponse` in `shared/openapi.yaml`.

`askedBy`, when populated, mirrors the member-listing avatar fields (`AskedBy` schema) so the UI can render a small avatar swatch next to the asker. It's populated when `isAnonymous=false`, OR whenever the caller is a group admin (admins always see authorship for moderation). When `isAnonymous=true` and the caller is a non-admin, `askedBy` is `null`. The internal `submittedBy` userId attribute is never returned directly — it's only surfaced through the redacted `askedBy` object.

`sort=top` orders by `voteCount` DESC, then `submittedAt` ASC (oldest first), then `questionId` ASC — the same tie-break promotion uses ([`lifecycle.md` §5.2](lifecycle.md#52-promote_and_opennl), [`data-model.md` §2.6](data-model.md#26-candidate-question-in-voting-pool)). `sort=recent` uses AP10 with sort by `submittedAt`. Default (when `sort` is omitted) is `top`. Since a group's candidate pool is bounded by its member count (soft-capped, typically ≤50), `lambda-questions` fetches the full pool via AP10 and sorts/pages it in memory rather than using AP11's GSI1 — whose tie order (newest-first on vote-count ties) doesn't match promotion's and would make the UI's "top" list disagree with which candidates actually get promoted.

### 6.2 `POST /groups/{groupId}/candidate-questions`

**Lambda**: `lambda-questions`. **Role**: member.

Body: `CreateCandidateRequest` in `shared/openapi.yaml` (`pollOptions`, a `CreatePollOptionRequest[]`, is required only for `kind: poll`).

Validation: prompt 5–500 chars. Poll: 2–6 options, each label 1–80 chars after trimming, no duplicate labels (compared case-insensitively, [`engagement.md` §3.3](engagement.md#33-validation)). `isAnonymous` is optional and defaults to `false` (the question will be attributed to the submitter); set to `true` to hide authorship from non-admin members. The submitter's `userId` is always recorded server-side regardless of `isAnonymous`. The flag is fixed at creation and cannot be changed later.

Response 201: `CandidateItemResponse`, with `askedBy` populated (the submitter is always allowed to see their own attribution, even when `isAnonymous=true`).

Errors: `VALIDATION_FAILED`, `CYCLE_NOT_VOTING` (if no eligible "next" cycle exists — e.g., the upcoming cycle has already locked).

### 6.3 `POST /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member.

Casts an upvote. Idempotent (re-POSTing returns 200 with current state).

Implementation: `TransactWriteItems` per [`data-model.md` §4](data-model.md#4-transactional-invariants) transaction #1. The cap is enforced in the transaction by the voter's `VoterTally` counter ([`data-model.md` §2.7a](data-model.md#27a-voter-tally)), so concurrent votes can't exceed `votesPerUserPerCycle`. A pre-read of the caller's votes supplies the error detail. If the cap is exceeded, returns `VOTE_CAP_REACHED` and lists the questions the user has already voted on. That list is carried in the problem body's free-text `detail` field (comma-separated question IDs), not a new structured field — §1.1's Problem Details shape only has `fieldErrors` for per-field validation issues, which doesn't fit a cap-conflict error.

Response 200: `CandidateVoteResponse` in `shared/openapi.yaml`.

### 6.4 `DELETE /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member. Withdraws the caller's upvote (transaction #2).

Response 200: same shape as 6.3.

### 6.5 Admin moderation routes

There is **no admin curate / promote / override** surface in v1. The cycle tick is the source of truth for which candidates promote, and locked questions are immutable. See [`lifecycle.md` §7](lifecycle.md#7-manual-admin-overrides). The only admin-side mutation on candidates:

- `DELETE /admin/groups/{groupId}/candidate-questions/{questionId}` — delete a candidate while it's still in `voting` (e.g., abusive). Removes votes too via batch delete. Returns 409 `CANDIDATE_PROMOTED` if the candidate has already been locked into a cycle.

**Vote cascade:** [`data-model.md`](data-model.md)'s access-pattern list has no "list voters of question X" pattern (`CandidateVote` is keyed by voter, not by question), so there is no efficient query for "every vote on this candidate." Scanning the whole table for one admin action would be worse. `lambda-questions` instead iterates the group's membership list (AP3, bounded by `memberSoftCap`) and issues one best-effort delete per member against that member's `(cycle, questionId)` vote key — deleting an absent item is a no-op, so no existence check is needed first. This is O(members) requests on a rare, admin-only action, not a hot path. **Order matters:** read the candidate's `vote_count`, sweep the votes, then delete the candidate conditioned on `vote_count` being unchanged (the sweep itself doesn't touch it). The sweep runs while the candidate still exists, so a failure partway leaves a retryable DELETE; deleting the candidate first would make a retry 404 and strand the remaining votes. A vote cast between the read and the delete moves the count and fails the delete, so the handler re-reads and sweeps again (up to 3 rounds, then 500 with the candidate intact and the DELETE retryable). After the delete, `cast_vote_tx`'s `vote_count` condition fails, so no vote can land later. (Until the 2026-10 review this was sweep, delete, sweep again unconditionally, which doubled every delete's cost and returned 500 for a delete that had succeeded when the second sweep failed.)

---

## 7. Response routes (drafts + publish)

### 7.1 Reading the caller's own responses

The only response route is the save (§7.3). The caller reads their own drafts and published answers through the newsletter routes: while the cycle is `open`, each question in `GET /groups/{groupId}/newsletters/{cycleId}` carries `myResponse` (§5.2), and `GET /groups/{groupId}/newsletters` reports `myDraftCount`/`myPublishedCount` per cycle (§5.1). Each save returns the full `ResponseDto`, so the editor never has to re-read what it just wrote.

### 7.2 Images on a draft

`myResponse.imageMediaIds` lists a draft's images by ID only. To show them, the respond page loads each one with `GET /uploads/{imageId}` (§9.3), which returns its presigned URLs and caption. An image that can't be loaded (for example, one that was deleted) is left out rather than blocking the editor ([`frontend.md` §8](frontend.md#8-drafts--detailed-flow)).

### 7.3 `PUT /groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/my-response`

**Lambda**: `lambda-responses`. The autosave/save-draft endpoint.

Body: `SaveResponseRequest` in `shared/openapi.yaml` (tagged by `kind`: text carries `body`+`imageMediaIds`, poll carries `pollOptionId`).

Image captions are stored on the `ImageMedia` row itself (set via `PATCH /uploads/{imageId}`, see §9.6) — they are not part of the response save payload. This keeps a caption attached to its image across edits and means an image's caption is the same wherever the image appears.

Behavior:
- Cycle must be in status `open` (else `CYCLE_NOT_OPEN`). Once the cycle has transitioned to `published`, this endpoint refuses all writes — including poll vote changes — and the SPA hides edit affordances accordingly.
- "Open" means `status = open` **and** `now < responseCloseAt`. The second clause covers the ≤5-minute gap before the cycle tick flips the status, so a save that lands just past the deadline is refused even though the row still says `open`.
- If `publish=true`, status moves to `published` and `publishedAt` is set. With `publish=false`, a new response is saved as `draft`.
- **Publishing is sticky.** Once a response is `published`, later saves with `publish=false` still update its content but leave `status=published` and the original `publishedAt` untouched. Autosaves of an edited published answer must not unpublish it, and this keeps the `editionsAnswered` count from being bumped twice ([`data-model.md` §4](data-model.md#4-transactional-invariants) #4). There is no unpublish in v1.
- `responseId` is assigned on the first save and stays the same across later saves.
- Every save is one transaction: the response write, a condition check on the Newsletter row (`status = open AND responseCloseAt > now`), and the `editionsAnswered` bump on the first publish ([`data-model.md` §4](data-model.md#4-transactional-invariants) #4). If the tick wins that race, the save returns 409 `CYCLE_NOT_OPEN`.
- The kind in the body must match the question's kind (else `VALIDATION_FAILED`). Image IDs are de-duplicated before the ≤10 check. Failing that check returns `IMAGE_LIMIT_EXCEEDED`. Any image that fails the ownership/`ready`/cycle/purpose rules returns `VALIDATION_FAILED` naming the image. The save does not modify `ImageMedia` rows; the image's question is fixed at `POST /uploads` time (§9.1).
- **Concurrency: last-write-wins.** Cross-device editing is rare in this app and the autosave debounce (≥1500 ms after the last keystroke) makes interleaved saves unlikely. We skip optimistic-concurrency tokens and conflict-resolution UI in v1; whichever save lands last is the canonical state. Revisit if real-world telemetry shows clobbering.
- Validation: text body 0–20000 chars, ≤10 image IDs, all referenced ImageMedia must (a) belong to caller, (b) be in `ready` status, (c) reference this `groupId`+`cycleId`, (d) have `purpose: "response"`.
- For polls: `pollOptionId` must be in question's options. Last-write-wins. Every poll pick saves immediately as a draft (`publish: false`), matching text answers — so an unpublished pick is auto-published too, same as a non-empty text draft (§7.4).

Response 200: `ResponseDto` in `shared/openapi.yaml`.

### 7.4 After the cycle closes

After the cycle has closed (status `published`), responses are immutable. Attempts to `PUT` return 409 `CYCLE_NOT_OPEN`. The frontend hides edit affordances accordingly.

**Auto-publish of non-empty drafts ([`lifecycle.md` §5.3](lifecycle.md#53-publishnl), [`data-model.md` §4](data-model.md#4-transactional-invariants) #4a).** Before flipping the Newsletter to `published`, `lambda-cycle-tick` auto-publishes every non-empty draft answer: text with a non-whitespace `body` or a non-empty `imageMediaIds`, or a poll with `pollOptionId` set. So a caller who never clicked "Publish" can still see their draft show up as a published answer once the cycle closes, as long as it wasn't empty — only an empty draft (or no draft at all) is excluded. There is no race with the save endpoint: saves are already refused once `now >= responseCloseAt` (the "open" clause above), even while the row still reads `status = open` in the up-to-5-minute gap before the tick runs, so the last successful save is exactly what auto-publish sees. The explicit Publish affordance in the UI still exists — it signals "I'm done" (status pill, progress count) — but it no longer determines whether an answer appears in the published edition once the deadline passes.

---

## 8. Engagement routes (comments + reactions)

All require the cycle to be `published`. Pre-publish, this surface is hidden in the UI.

**Addressing.** Every engagement route is nested under the answer's question: `/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/...`. Comment and reaction rows are keyed on `(questionId, answerUserId)` ([`data-model.md` §2.11](data-model.md#211-comment)–§2.12), and a bare `responseId` has no index, so it couldn't be resolved without reading every question's answers. With `questionId` in the path, the handler queries that question's answers (`pk=GROUP#g#NL#c#Q#q`, `begins_with(sk, A#)`, at most one row per member) and takes the row whose `response_id` matches. That row must be a published **text** answer; otherwise the route returns 404. Poll answers are never engagement targets ([`engagement.md` §3](engagement.md#3-polls)).

**Check order on all seven routes (GETs included).** Each check runs only if the previous one passed:
1. Membership: 403 if the caller isn't a member.
2. The cycle exists: 404 if not.
3. The cycle isn't `archived`: 410 `NEWSLETTER_ARCHIVED` if it is.
4. The cycle is `published`: 409 `CYCLE_NOT_PUBLISHED` if not.
5. The answer resolves: 404 if not.
6. Route-specific checks.

**Comment shape.** `CommentResponse` in `shared/openapi.yaml`, used both here and inline in §5.2. `displayName` is the author's account display name. `image` (a `CommentImageResponse`) is populated while the attached image is `ready`, and null otherwise. Soft-deleted comments are **returned** as placeholders, with `deletedAt` set, `body` empty and `image` null, so the UI can render `[deleted]` in place.

### 8.1 `GET .../questions/{questionId}/responses/{responseId}/comments`

**Lambda**: `lambda-engagement`. AP18. Paginated (`limit` default 50, max 100), oldest first. Returns `CommentListResponse {items, nextCursor}`.

### 8.2 `POST .../questions/{questionId}/responses/{responseId}/comments`

Body: `CreateCommentRequest` in `shared/openapi.yaml`. `body` 0–2000 chars. `imageMediaId` must reference an image that is all of the following:
- `ready`
- owned by the caller
- uploaded for this group, cycle and question with `purpose: "comment"` (§9.1)
- not attached to another comment

A whitespace-only or missing `body` with no image is a 422. All failures are `VALIDATION_FAILED` with `fieldErrors` on `body` or `imageMediaId`. See [`engagement.md` §1.3](engagement.md#13-validation).

Response 201: the created `CommentResponse`.

**Image attachment.** `ImageMedia` gains `attached_comment_id` ([`data-model.md` §2.10](data-model.md#210-imagemedia)). Writing the comment and claiming the image happen in one `TransactWriteItems`, conditioned on the image being unclaimed (or already claimed by this comment). The same applies to changing or removing an image on PATCH and to soft delete, which also releases the old image. §9.4 uses the claim for its in-use check.

### 8.3 `PATCH .../comments/{commentId}`

Body: `PatchCommentRequest` in `shared/openapi.yaml`. Author only: admins can't edit other people's comments, and get 403. An absent field is left unchanged. `"imageMediaId": null` removes the attachment. The result must still have a non-whitespace body or an image. Sets `editedAt`. A missing or soft-deleted comment returns 404. Response 200: the updated `CommentResponse`.

### 8.4 `DELETE .../comments/{commentId}`

Soft delete: sets `deletedAt`, blanks `body` and drops the image attachment, releasing the image's claim. The row remains, so the parent thread's ordering is preserved and moderation history survives. Allowed for the author or a group admin. Idempotent: a `DELETE` of an already-soft-deleted row returns 204, and a `DELETE` of a missing row returns 404.

### 8.5 `GET .../questions/{questionId}/responses/{responseId}/reactions`

**Lambda**: `lambda-engagement`. AP19. Returns `ReactionsResponse` in `shared/openapi.yaml`: the grouped form (`reactionGroups`) plus the caller's own emoji (`myReactions`). **Group order:** count descending, then earliest reaction first, then emoji. The inline `reactionGroups` in §5.2 use the same order (one shared function).

### 8.6 `PUT .../reactions/{emoji}`

Idempotent toggle-on with an empty body. `emoji` is URL-encoded. The server percent-decodes it if it still contains `%`, because API Gateway's decoding of path parameters isn't relied on. It then validates it against the canonical emoji predicate in [`engagement.md` §2.2](engagement.md#22-validation): NFC-normalized, 1–12 codepoints, and at least one `Extended_Pictographic` or regional-indicator codepoint. Anything else is rejected with `VALIDATION_FAILED`. Encoding the emoji in the path simplifies the `R#{user}#{emoji}` sort key. Response 200: the answer's `ReactionsResponse` after the change.

### 8.7 `DELETE .../reactions/{emoji}`

Removes the caller's reaction with that emoji. Idempotent, with the same validation as PUT. Response 200: the answer's `ReactionsResponse` after the change.

---

## 9. Media routes

### 9.1 `POST /uploads`

**Lambda**: `lambda-media`. Returns a pre-signed S3 PUT URL the client uses to upload directly.

Body: `CreateUploadRequest` in `shared/openapi.yaml` (`purpose` defaults to `response`; see [`media.md` §3.1](media.md#31-post-uploads-validation) for `comment`).

`sha256`, when sent, is signed into the presign as `x-amz-checksum-sha256`, so S3 rejects a body whose digest doesn't match. It must be **standard base64**, not base64url: presigned PUTs can't use `x-amz-content-sha256` for payload integrity, and S3's flexible-checksum header needs standard base64.

Validation: `mimeType` ∈ {`image/jpeg`, `image/png`, `image/webp`, `image/gif`} (else 415 `IMAGE_BAD_TYPE` — an unknown type such as SVG is a 415, not a 422), 1 ≤ `byteSize` ≤ 15728640 (over → 413 `IMAGE_TOO_LARGE`), caller is member of group, `questionId` is a locked question of the cycle (else 422). Cycle-status check depends on `purpose`: `"response"` requires the cycle to be `open`; `"comment"` requires it to be `published` (comments only exist post-publish — this is the upload path for comment attachments, [`engagement.md` §1.3](engagement.md#13-validation)). The 10-image cap applies to `purpose=response` uploads only.

Server creates an `ImageMedia` row with `status=pending` and an S3 key:
```
uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}
```

Response 201: `CreateUploadResponse` in `shared/openapi.yaml`.

The frontend then `PUT`s the binary directly to `uploadUrl` with **every** listed header, verbatim. `headers` is whatever the presigned request was signed with (lower-case names).

### 9.0 Addressing an existing upload

An `ImageMedia` row is keyed by `(groupId, cycleId, imageId)` ([`data-model.md` §2.10](data-model.md#210-imagemedia)), so every `/uploads/{imageId}` route (§9.2–§9.4, §9.6) takes **required `groupId` and `cycleId` query parameters**. Without them the lookup would need an unbounded scan of the caller's GSI1 uploads. The caller must be a member of `groupId` (403 otherwise). `GET` and `/complete` are open to any member; `PATCH` and `DELETE` require the caller to own the image (403 otherwise).

Every one of these routes that returns a body returns `ImageMediaResponse` (`shared/openapi.yaml`): the row's fields in camelCase, minus the S3 keys, plus absolute `displayUrl`/`thumbUrl` (null until `ready`) and `errorMessage` (`IMAGE_TOO_LARGE` or `IMAGE_DECODE_FAILED` when `failed`). `DELETED` is not a possible value here — `DELETE /uploads/{imageId}` deletes the row instead of soft-marking it (§9.4).

### 9.2 `POST /uploads/{imageId}/complete`

**Lambda**: `lambda-media`. Optional convenience endpoint — the S3 trigger does the same work. Returns the current `ImageMedia` status. Used by the client to short-circuit polling on slow networks.

### 9.3 `GET /uploads/{imageId}`

**Lambda**: `lambda-media`. Returns the `ImageMedia` row (status, dimensions, processed URLs). Frontend polls this every ~1.5s after upload until `status=ready`.

### 9.4 `DELETE /uploads/{imageId}`

**Lambda**: `lambda-media`. **Real delete** — a soft-deleted row would leave the processed S3 variants still servable by presigned URL, so this removes the objects too. Allowed only when image is not referenced by any published response: if the caller's answer to the image's question is published and lists the image, 409 `IMAGE_IN_USE`. A `purpose: "comment"` image that a live comment has claimed (`attached_comment_id` set, §8.2) is also 409 `IMAGE_IN_USE`.

Otherwise: delete the original S3 object, both processed variants (display + thumb), then delete the `ImageMedia` row — via the shared delete helper ([`infrastructure.md` §6.2](infrastructure.md#62-lambda-functions-request-handlers)). Once a variant is deleted, its outstanding presigned URLs 403 immediately ([`media.md` §8.0](media.md#80-explicit-deletion)). S3 deletes happen first; if any fails, return 500 and leave the row in place so a retry can finish the job. Returns 204 on success. Idempotent: a repeat `DELETE` whose row is already gone also returns 204 (nothing left to leak). If the caller's answer is still a *draft* that lists the image, the handler first drops the ID from the draft (2026-10 review): one `UpdateItem` on the response row, `SET image_media_ids = <the rest>`, conditioned on `status = draft AND image_media_ids = <the list it read>`. A single-item conditional write is already atomic, so no multi-item transaction is needed; it can't share one with the S3 deletes anyway. It runs before any S3 delete, so a draft that is saved or published concurrently fails the condition and the handler rereads (up to 3 rounds, then 500): a now-published answer that lists the image gets `IMAGE_IN_USE` with nothing deleted. The response editor's "remove image" action still saves the draft without the ID before calling this route, so for it the strip is a no-op; it covers stale tabs and older PWAs. A save that validated the image just before the delete can still re-list it, so readers keep tolerating dangling IDs (`persistence::responses::has_live_content` at auto-publish and in reminders; the published read skips missing images).

Comment images follow the same path: when a comment is soft-deleted, or its image is removed or replaced (§8.3, §8.4), the released image is deleted this way after the comment transaction commits (best effort; a failure is logged at ERROR, not surfaced to the caller). The UI confirms every one of these permanent deletes with a `ConfirmDialog` ("This permanently deletes the image. It can't be recovered."): removing an image in the response editor, removing or replacing a comment image, and deleting a comment that has an image.

### 9.5 Image and avatar URLs

Every image and avatar URL in a response body is an S3 presigned GET URL for one object in the processed bucket, minted by the handler that returns it after its usual membership or ownership check:

- `displayUrl` / `thumbUrl` on `PublishedImageResponse`, `CommentImageResponse` and `ImageMediaResponse` (`null` until the image is `ready`)
- `avatarUrl` on `UserResponse`, `MemberResponse`, `AskedBy` and `AvatarMediaResponse` (`null` when there's no avatar)

There's no media route to call first: a URL is usable as soon as it's returned.

- A URL is valid for at most `MEDIA_URL_TTL_SECONDS` (1 h), and can stop working sooner if the signing Lambda's credentials expire first. A client that gets a 403 refetches the resource for fresh URLs rather than retrying the same one.
- Within one 5-minute signing window, a container returns the same URL for the same object, so a refetch doesn't change an `<img src>`.
- Clients treat the URLs as opaque. The query string changes between windows, so anything that caches the bytes keys on the path, not the full URL.

See [`media.md` §4](media.md#4-serving-presigned-s3-gets) and [`decisions.md`](../decisions.md#presigned-s3-media).

### 9.6 `PATCH /uploads/{imageId}`

**Lambda**: `lambda-media`. **Role**: caller must own the image.

Body: `PatchUploadRequest` in `shared/openapi.yaml` (`caption` ≤140 chars after trimming; `null` or an all-whitespace string clears it).

Updates the `ImageMedia.caption` field. Allowed regardless of whether the image is referenced by a draft or published response — the caption is a property of the image, not of a particular usage.

Response 200: `ImageMediaResponse`.

### 9.7 Avatar routes

Avatars use a separate pipeline (see [`media.md` §11](media.md#11-avatars-separate-pipeline)) so they can cross group boundaries.

- `POST /avatars` — request a pre-signed PUT URL. Body: `CreateAvatarRequest` in `shared/openapi.yaml`. Validation: `byteSize` ≤ 5 MB (else 413), MIME as listed (GIF → 415). Server creates an `AvatarMedia` row (`status=pending`) and returns `CreateAvatarResponse`.
- `GET /avatars/{avatarId}` — returns `AvatarMediaResponse` (status, `avatarUrl` once ready, `errorMessage`). Looked up under the caller's own user partition, so another user's avatar is a 404. Frontend polls until `status=ready`, then issues `PATCH /me` with the new `avatarMediaId`.
- `DELETE /avatars/{avatarId}` — caller must own (404 otherwise). **Real delete**: deletes the original + processed S3 objects, then deletes the `AvatarMedia` row, via the same shared delete helper as §9.4. The deleted object's presigned URLs 403 immediately. Idempotent — a repeat `DELETE` of an already-gone row returns 204. If the avatar is the caller's current `avatarMediaId`, the User row is also cleared back to `null` (the deterministic `avatarColor` takes over for rendering). Because the lookup is under the caller's own user partition, "404 if not owned" and "a repeat DELETE returns 204" can't be told apart: any missing row returns 204, and another user's avatar is never touched. The User row is cleared *before* the objects are deleted, so a failed delete never leaves the profile pointing at half-deleted objects.
- **Avatar replacement and clearing:** when `PATCH /me` (§2.3) sets a new `avatarMediaId` over an existing one, or sets it to `null`, the handler deletes the old avatar's original + processed objects the same way, then deletes the old `AvatarMedia` row, after the `User` row update commits. The frontend confirms both this replace-over-existing case and an explicit `DELETE /avatars/{id}` with a `ConfirmDialog` ("This permanently deletes the image. It can't be recovered.").

Avatar URLs are presigned S3 GET URLs for `avatar/{avatarId}/display.webp` in the processed bucket, signed by `media-urls` the same way as every other image field ([`media.md` §11.3](media.md#113-serving)) — see [`data-model.md` §2.16](data-model.md#216-avatarmedia).

---

## 10. Push routes

**Lambda** for every route here: `lambda-push` (binary `push-api`). **Role**: self. Exact shapes are in `shared/openapi.yaml` (tag `push`).

### 10.1 `POST /push/subscribe`

Body: `PushSubscribeRequest` in `shared/openapi.yaml` — the browser's `PushSubscription.toJSON()` plus `userAgent`.

Behavior: hash endpoint (base64url-unpadded sha256), upsert by `(userId, endpointHash)`. Resets `failureCount=0`; keeps the row's original `createdAt` and `lastSuccessAt` — the SPA re-posts its subscription on every load, which must not wipe the device's history. Validation: `endpoint` is `https://`, ≤2048 chars; `keys.p256dh` decodes to a 65-byte uncompressed P-256 point, `keys.auth` to 16 bytes (base64url; trailing `=` padding is accepted and stripped before storing); `userAgent` ≤512 chars. **Cap:** at most `MAX_PUSH_SUBSCRIPTIONS_PER_USER` (10) rows per user; a new 11th endpoint evicts the least recently successful one (never-succeeded counts as oldest, ties by `createdAt`).

Response 201: `PushSubscribeResponse` (`subscriptionId` = the endpoint hash).

### 10.2 `POST /push/unsubscribe`

Body: `PushUnsubscribeRequest` in `shared/openapi.yaml`. Deletes the matching row. Idempotent.

Response 204. The Settings "My devices" remove button uses this too, with the `endpoint` from §10.3 (no separate DELETE route).

### 10.3 `GET /push/subscriptions`

Returns the caller's active subs (for a settings page where they can prune devices): `PushSubscriptionListResponse` in `shared/openapi.yaml`, newest first. Keys are never returned; `endpoint` is, so the client can mark "This device".

### 10.4 `POST /push/test`

Sends a test push to all of the caller's subscriptions. Useful from the Settings page. Response 200: `PushTestResponse`. `expired` (push service 404/410) means the row was deleted; `failed` increments `failureCount` (deleted at 5).

### 10.5 `PUT /push/preferences/{groupId}`

Body: `PutPushPreferenceRequest` in `shared/openapi.yaml`, both fields required. Upserts a `NotificationPref` row and returns it (`PushPreferenceResponse`). The caller must be a member of `groupId` (403 otherwise). Publication notifications are always-on with no per-user toggle; the API rejects any `publication` field (indeed any unknown field) in the body with `VALIDATION_FAILED`.

### 10.6 `GET /push/preferences`

Returns `PushPreferenceListResponse`, one per membership of the caller, defaults (both `true`) filled in where no row exists. §10.5 alone left the Settings page no way to read current state.

---

## 11. Health

### 11.1 `GET /healthz`

**Lambda**: `lambda-groups`. **Auth**: required (so it stays inside the protected stage; idle backend principle).

Response 200: `HealthResponse` in `shared/openapi.yaml`. `version` and `buildAt` are baked in at compile time from `BUILD_SHA`/`BUILD_AT`, which `make build-lambdas` sets. A local build reports `"unknown"` and `null`.

---

## 11a. Dev-only endpoints

These routes are wired into the API only when `ENV=dev` and refuse with `404 NOT_FOUND` in any other environment. They exist so manual testing and Playwright E2E can fast-forward state without waiting for EventBridge. See [`environments.md` §9](../development/environments.md#9-eventbridge-ticks-in-dev).

### 11a.1 `POST /admin/dev/tick/cycle`

**Lambda**: `lambda-cycle-tick`. **Auth**: required. **Role**: any group admin.

Synchronously invokes the cycle-tick handler. Optional body: `DevTickCycleRequest` in `shared/openapi.yaml`. When `advanceCycleClosesBy` is present, `groupId` is **required** and the caller must be an admin of that group: the group's active cycle has its `nextTransitionAt` (and the matching `responseOpenAt`/`responseCloseAt` field) rewound by the given duration before the tick runs, making the transition due immediately. The duration is an integer plus `m`/`h`/`d`, between 0 and 366 days; anything else is `VALIDATION_FAILED` with a `fieldErrors` entry for `advanceCycleClosesBy`. With an empty body the tick just runs globally; the role check is "caller is an admin of at least one group."

"The group's active cycle" means: the `open` cycle if one exists (its `responseCloseAt` is rewound, fast-forwarding toward publication — the common E2E case), else the `voting` cycle (its `responseOpenAt`/`voteWindowCloseAt` is rewound, fast-forwarding toward promotion). A group always has at most one of each, so this is unambiguous.

Response 200: `DevTickCycleResponse` in `shared/openapi.yaml`, one `CycleTransition` per state change; `from`/`to` are `NewsletterStatus` values. A freshly created next-cycle row ([`lifecycle.md` §5.4](lifecycle.md#54-create_next_voting_cyclegroup-after)) appears in the same array with `from: null, to: voting` rather than in a separate list — one array covers every state change a tick made, created-from-nothing included. The scheduled tick logs the same shape.

### 11a.2 `POST /admin/dev/tick/notify`

**Lambda**: `lambda-notify-tick`. **Auth**: required. **Role**: any group admin.

Synchronously invokes the notify-tick handler (globally; no body). Response 200: `DevTickNotifyResponse` in `shared/openapi.yaml`, one `DeadlineFanout` per reminder sent. Only deadline reminders run in notify-tick, so `kind` is always `deadline_reminder`; `failed` counts expired and failed sends together. Cycle-open and publication fan-outs triggered by `POST /admin/dev/tick/cycle` run asynchronously in `lambda-push` and are not reported by either route.

Both routes sit under the `dev` tag in `shared/openapi.yaml`, with their wire types in `domain::api` and rows in the contract test's `ROUTE_TABLE` like every other route. Both are also called by Playwright E2E ([`testing-and-ci.md` §4.3](../development/testing-and-ci.md#43-playwright-e2e)) — implement once, two consumers.

---

## 12. OpenAPI generation

`shared/openapi.yaml` is hand-maintained as the canonical contract. Codegen targets: TypeScript types → `frontend/src/types/api.ts` via `openapi-typescript` (driven by `scripts/codegen_types.sh`), and Rust request/response types → `backend/crates/domain/src/api.rs`, hand-written serde structs that mirror the YAML (chosen over `progenitor`-generated Rust, which would fight the newtype IDs in `domain/ids.rs`).

The contract test that keeps the Rust structs and YAML in sync (`ROUTE_TABLE` in `backend/crates/domain/tests/openapi_contract.rs`) is detailed in [`testing-and-ci.md` §2.3](../development/testing-and-ci.md#23-contract-test). Any new or changed route needs a YAML example and a `ROUTE_TABLE` row in the same PR, per [`coding-standards.md` §8](../development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task).

---

## 13. Per-route auth/role summary table

(Keep this synced with code; `infra/tests/` asserts every synthesized route has the JWT authorizer, and the contract test in [`testing-and-ci.md` §2.3](../development/testing-and-ci.md#23-contract-test) keeps routes and the YAML in step.)

| Method + Path | Role |
|---|---|
| `GET /config`, `GET /me`, `PATCH /me` | authenticated |
| `POST /admin/invites` | admin of {groupId in body} |
| `GET /admin/groups/{g}/invites`, `POST /admin/invites/{code}/revoke` | admin |
| `POST /invites/redeem` | authenticated |
| `GET /groups/{g}` | member |
| `PATCH /groups/{g}` | admin |
| `DELETE /groups/{g}/members/{u}` | self OR admin |
| `PATCH /groups/{g}/members/{u}` | admin (role only) |
| `GET /groups/{g}/newsletters*` | member |
| `*` candidate-questions GET/POST/votes | member |
| `DELETE /admin/groups/{g}/candidate-questions/{q}` | admin |
| `PUT …/my-response` | member (and self) |
| `*` comments/reactions GET/POST, reactions PUT/DELETE | member |
| `PATCH` comment | author only |
| `DELETE` comment | author OR admin |
| `POST /uploads*`, `GET /uploads/{id}`, `PATCH /uploads/{id}`, `DELETE /uploads/{id}` | member (owner for PATCH/DELETE) |
| `POST /avatars`, `GET /avatars/{id}`, `DELETE /avatars/{id}` | self (owner) |
| `*` push routes | self |
| `GET /healthz` | authenticated |
| `POST /admin/dev/tick/*` | admin (dev environment only; 404 in prod) |
