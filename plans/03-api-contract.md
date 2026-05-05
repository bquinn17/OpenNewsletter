# 03 — API Contract

This is the source-of-truth list of every HTTP endpoint OpenNewsletter exposes. Each route names its handler Lambda (matching `01-infrastructure-cdk.md` §6.2), the auth/role requirement, request/response shapes, error codes, and the access-pattern numbers from `02-data-model-dynamodb.md`.

A canonical OpenAPI 3.1 document lives at `shared/openapi.yaml`. This Markdown is the human-readable mirror; the YAML is what gets codegen'd into TS types and Rust route stubs.

---

## 1. General conventions

- Base URL: `https://api.opennewsletter.example.com` (prod) / `https://api-dev.opennewsletter.example.com` (dev)
- Content type: `application/json` (UTF-8) for everything except media uploads (which go directly to S3).
- Auth: All routes require a Cognito JWT in `Authorization: Bearer {accessToken}` UNLESS marked **Public**. There are no Public routes — the OAuth callback is a Cognito-managed redirect, not an app endpoint.
- Path tenancy: Routes that operate inside a group are prefixed `/groups/{groupId}/...`. The handler verifies caller membership before doing anything (per `02-data-model-dynamodb.md` §6).
- Correlation: Clients SHOULD send `x-correlation-id: {ulid}`. If absent the API generates one. It is echoed in responses and logged.
- Pagination: List endpoints accept `?cursor={opaque}&limit={1..100}`. Responses include `nextCursor` (null when exhausted). Cursor is base64url(JSON) encoding DynamoDB's `LastEvaluatedKey`.
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
  "fieldErrors": [{"field": "body", "code": "TOO_LONG", "message": "..."}]
}
```

Error code catalog (machine-readable codes; `fieldErrors[].code` is a subset):

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
| `RESPONSE_VERSION_CONFLICT` | 409 | Optimistic concurrency mismatch on draft save |
| `IMAGE_LIMIT_EXCEEDED` | 409 | More than 10 images attached |
| `IMAGE_TOO_LARGE` | 413 | Original > 15MB |
| `IMAGE_BAD_TYPE` | 415 | MIME type not allowed |
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
    { "groupId": "01H...", "role": "admin", "groupName": "Trail Crew" }
  ]
}
```

`memberships` is a denormalized join — each entry includes `groupName` so the frontend can render the group switcher without a follow-up query.

### 2.2 `GET /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Returns the bare User profile (no memberships). Used by the settings page.

Response 200: `User` shape from `02-data-model-dynamodb.md` §2.1.

### 2.3 `PATCH /me`

**Lambda**: `lambda-groups`. **Auth**: required.

Body: `{ "displayName"?: string, "avatarMediaId"?: string|null }`

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

Used **only** by an existing user to join an additional group. (First-signup invite consumption happens in the Cognito `PreSignUp` trigger — see `05-auth-flow.md` §3.)

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

### 3.6 `PATCH /groups/{groupId}/members/{userId}` (role change / nickname)

**Lambda**: `lambda-groups`. **Role**: admin (for role); self (for nickname).

Body: `{ "role"?: "admin"|"member", "nickname"?: string|null }`

---

## 4. Group routes

### 4.1 `GET /groups`

**Lambda**: `lambda-groups`. Returns the same `memberships` array as `/config`. Useful when the SPA refreshes group list without re-fetching everything.

### 4.2 `GET /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: member.

Response 200: full `Group` (incl. settings) plus `members: [{userId, displayName, role, nickname}]`.

### 4.3 `PATCH /groups/{groupId}`

**Lambda**: `lambda-groups`. **Role**: admin.

Body (all optional):
```json
{
  "name": "...",
  "timezone": "America/New_York",
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

Validation: `responseWindowDays` between 1 and 28; `questionsPerCycle` between 1 and 20; `votesPerUserPerCycle` between 1 and `questionsPerCycle * 2`; `timezone` must be a valid IANA TZ; `offsetsHoursBeforeClose` non-empty, descending, integers ≥1.

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
      "answers": [
        {
          "responseId": "01H...",
          "userId": "01H...",
          "displayName": "Quinn",
          "body": "...markdown...",
          "images": [{ "imageId": "01H...", "displayUrl": "https://cdn.../img/...", "thumbUrl": "...", "width": 1200, "height": 800 }],
          "publishedAt": "...",
          "comments": [{ "commentId": "01H...", "authorUserId": "01H...", "displayName": "...", "body": "...", "createdAt": "..." }],
          "reactionGroups": [{ "emoji": "🔥", "count": 3, "reactedByMe": true }]
        }
      ]
    },
    {
      "questionId": "01H...",
      "kind": "poll",
      "prompt": "Which trail had the best views?",
      "options": [
        { "optionId": "01H...", "label": "Mt Si", "voteCount": 4 },
        { "optionId": "01H...", "label": "Lake 22", "voteCount": 7 }
      ],
      "myVoteOptionId": "01H..."
    }
  ]
}
```

Image URLs are absolute CloudFront URLs. The frontend MUST first call `/media-cookie` (§9.5) to receive signed cookies before requesting any image.

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
      "voteCount": 7,
      "votedByMe": true,
      "submittedAt": "..."
    },
    {
      "questionId": "01H...",
      "kind": "poll",
      "prompt": "...",
      "pollOptions": [{ "optionId": "01H...", "label": "..." }],
      "voteCount": 5,
      "votedByMe": false,
      "submittedAt": "..."
    }
  ],
  "nextCursor": null
}
```

`submittedBy` is OMITTED for non-admins.

`sort=top` uses AP11 (GSI1, ScanIndexForward=false). `sort=recent` uses AP10 with sort by `submittedAt`.

### 6.2 `POST /groups/{groupId}/candidate-questions`

**Lambda**: `lambda-questions`. **Role**: member.

Body:
```json
{
  "kind": "text",
  "prompt": "...",
  "pollOptions": null
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
  ]
}
```

Validation: prompt 5–500 chars. Poll: 2–6 options, each label 1–80 chars, no duplicate labels.

Response 201: full candidate item (without `submittedBy`).

Errors: `VALIDATION_FAILED`, `CYCLE_NOT_VOTING` (if no eligible "next" cycle exists — e.g., the upcoming cycle has already locked).

### 6.3 `POST /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member.

Casts an upvote. Idempotent (re-POSTing returns 200 with current state).

Implementation: `TransactWriteItems` per `02-data-model-dynamodb.md` §4 transaction #1, with a precondition that counts the caller's existing votes against `votesPerUserPerCycle`. If the cap is exceeded, returns `VOTE_CAP_REACHED` and lists the questions the user has already voted on.

Response 200:
```json
{ "questionId": "...", "voteCount": 8, "votedByMe": true, "myVoteCount": 3 }
```

### 6.4 `DELETE /groups/{groupId}/candidate-questions/{questionId}/votes`

**Lambda**: `lambda-questions`. **Role**: member. Withdraws the caller's upvote (transaction #2).

Response 200: same shape as 6.3.

### 6.5 Admin curate routes (override voting outcome)

All require **admin** role.

- `POST /admin/groups/{groupId}/cycles/{cycleId}/curate/promote` — body `{ "questionIds": [...] }` — explicitly promotes a list. Cycle must still be in `voting`. Replaces `lockedQuestionIds`. Used to override a vote ranking before the cycle opens.
- `DELETE /admin/groups/{groupId}/candidate-questions/{questionId}` — delete a candidate (e.g., abusive). Removes votes too via batch delete.
- `PATCH /admin/groups/{groupId}/cycles/{cycleId}/questions/{questionId}` — body `{ "displayOrder": number, "prompt"?: string }` — rearrange or fix typos. Cycle must be `open`; once `published`, prompt is immutable.

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
  "version": 7,                       // current server version known to client; null on first save
  "kind": "text",
  "body": "...markdown...",
  "imageMediaIds": ["01H...", "01H..."],
  "publish": false
}
```

Body (poll):
```json
{
  "version": 1,
  "kind": "poll",
  "pollOptionId": "01H...",
  "publish": true
}
```

Behavior:
- Cycle must be in status `open` (else `CYCLE_NOT_OPEN`).
- If `publish=true`, status moves to `published` and `publishedAt` is set. Otherwise stays/becomes `draft`.
- Optimistic concurrency: condition `attribute_not_exists(version) OR version = :expectedVersion`. On failure return 409 `RESPONSE_VERSION_CONFLICT` with the current server item embedded.
- Validation: text body 0–20000 chars, ≤10 image IDs, all referenced ImageMedia must (a) belong to caller, (b) be in `ready` status, (c) reference this `groupId`+`cycleId`.
- For polls: `pollOptionId` must be in question's options. Last-write-wins.

Response 200: full updated `Response`.

### 7.4 Auto-republish after edit

After the cycle has closed (status `published`), responses are immutable. Attempts to `PUT` return 409 `CYCLE_NOT_OPEN`. The frontend hides edit affordances accordingly.

### 7.5 `DELETE` of own response

Not exposed. Drafts simply remain unpublished. Published responses can only be removed by the user via a future feature (out of scope for v1).

---

## 8. Engagement routes (comments + reactions)

All require the cycle to be `published`. Pre-publish, this surface is hidden in the UI.

### 8.1 `GET /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/comments`

**Lambda**: `lambda-engagement`. AP18. Paginated, oldest first.

### 8.2 `POST /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/comments`

Body: `{ "body": "..." }` (1–2000 chars).

Response 201: created comment.

### 8.3 `PATCH /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/comments/{commentId}`

Body: `{ "body": "..." }`. Allowed for the author only. Sets `editedAt`.

### 8.4 `DELETE /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/comments/{commentId}`

Soft delete (sets `deletedAt`, blanks `body`). Allowed for author OR group admin.

### 8.5 `GET /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/reactions`

**Lambda**: `lambda-engagement`. AP19. Returns grouped form (`reactionGroups`) plus a flat list scoped to caller (`myReactions`).

### 8.6 `PUT /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/reactions/{emoji}`

Idempotent toggle-on. Body empty. `emoji` URL-encoded; server validates it's a single grapheme cluster (1–12 codepoints) and otherwise rejects with `VALIDATION_FAILED`. Path-encoded emoji simplifies the `R#{user}#{emoji}` sort key.

### 8.7 `DELETE /groups/{groupId}/newsletters/{cycleId}/responses/{responseId}/reactions/{emoji}`

Removes the caller's reaction with that emoji. Idempotent.

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
  "mimeType": "image/jpeg",
  "byteSize": 4823920,
  "sha256": "base64url..."   // optional but recommended; included in the presign condition
}
```

Validation: `mimeType` ∈ {`image/jpeg`, `image/png`, `image/webp`, `image/gif`}, `byteSize` ≤ 15728640 (15MB), cycle status `open`, caller is member of group.

Server creates an `ImageMedia` row with `status=pending` and an S3 key:
```
uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}
```

Response 201:
```json
{
  "imageId": "01H...",
  "uploadUrl": "https://...s3.amazonaws.com/...",
  "headers": { "Content-Type": "image/jpeg", "x-amz-content-sha256": "..." },
  "expiresInSeconds": 600
}
```

The frontend then `PUT`s the binary directly to `uploadUrl` with the listed headers.

### 9.2 `POST /uploads/{imageId}/complete`

**Lambda**: `lambda-media`. Optional convenience endpoint — the S3 trigger does the same work. Returns the current `ImageMedia` status. Used by the client to short-circuit polling on slow networks.

### 9.3 `GET /uploads/{imageId}`

**Lambda**: `lambda-media`. Returns the `ImageMedia` row (status, dimensions, processed URLs). Frontend polls this every ~1.5s after upload until `status=ready`.

### 9.4 `DELETE /uploads/{imageId}`

**Lambda**: `lambda-media`. Detach + mark for cleanup. Allowed only when image is not referenced by any published response. Soft-marks the row `failed` and the S3 lifecycle handles real deletion.

### 9.5 `GET /media-cookie`

**Lambda**: `lambda-media`. Returns CloudFront signed cookies scoped to a single group. Cookies last 1 hour and cover paths matching `/img/{groupId}/*`.

Response 200 sets cookies via `Set-Cookie` headers (`CloudFront-Policy`, `CloudFront-Signature`, `CloudFront-Key-Pair-Id`) AND returns a JSON body with the same values for clients that can't read response cookies cross-origin:

```json
{
  "policy": "...",
  "signature": "...",
  "keyPairId": "...",
  "expiresAt": "..."
}
```

For the SPA, the cookies are sufficient (the CloudFront domain is on the same registrable domain as the API via the `cdn.` subdomain — see DNS plan).

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

Body: `{ "cycleOpen": true, "deadlineReminders": true }`. Upserts a `NotificationPref` row.

---

## 11. Health

### 11.1 `GET /healthz`

**Lambda**: `lambda-groups`. **Auth**: required (so it stays inside the protected stage; idle backend principle).

Response 200: `{ "status": "ok", "version": "git-sha", "buildAt": "..." }`.

---

## 12. OpenAPI generation

`shared/openapi.yaml` is hand-maintained as the canonical contract; codegen targets:

- TypeScript types → `frontend/src/types/api.ts` via `openapi-typescript`
- Rust request/response types → `backend/crates/domain/src/api.rs` via `progenitor` (or hand-written serde structs that mirror the YAML — see `11-testing-ci-cd.md` for the contract test).

A schema validation test in CI rejects any drift between handwritten Rust models and the YAML.

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
| `PATCH /groups/{g}/members/{u}` | self (nickname) / admin (role) |
| `GET /groups/{g}/newsletters*` | member |
| `*` candidate-questions GET/POST/votes | member |
| `*` admin/curate routes | admin |
| `*` my-response routes | member (and self) |
| `*` comments/reactions | member |
| `POST /uploads*`, `GET /uploads/{id}`, `DELETE /uploads/{id}`, `GET /media-cookie` | member |
| `*` push routes | self |
| `GET /healthz` | authenticated |
