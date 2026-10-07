# 09 — Engagement (Comments, Reactions, Polls)

Engagement features apply to **published** newsletters only. While a cycle is `voting` or `open`, comments and reactions are not exposed in the API or UI. Polls behave differently: poll *votes* are first-class responses (cast during the response window), but the *tally* renders as a feature of the published edition.

---

## 1. Comments

### 1.1 Shape

Flat (no threading). One author per comment. Stored in DynamoDB per `02-data-model-dynamodb.md` §2.11.

```ts
type Comment = {
  commentId: string;
  authorUserId: string;
  displayName: string;   // author's account display name (wire name decided M10; matches the inline newsletter shape)
  body: string;          // 0-2000 chars; markdown allowed (sanitized identical to response bodies)
  image: {               // optional single attached image; null when none
    imageId: string;
    displayUrl: string;
    thumbUrl: string;
    width: number;
    height: number;
    caption: string | null;
  } | null;
  createdAt: string;
  editedAt: string | null;
  deletedAt: string | null; // set on soft-deleted placeholders (added M10)
};
```

Either `body` (non-whitespace) OR `image` must be present; an entirely empty comment is rejected.

Comments are **soft deleted** in v1 — `DELETE` sets `deletedAt` and blanks `body`. The row remains so thread ordering is preserved and an admin can audit moderation history. The UI renders soft-deleted comments as a muted `"[deleted]"` placeholder. The API returns them, both from the comments route and inline in the published newsletter, with `deletedAt` set, `body` empty and `image` null (decided M10). **Decided 2026-10-07; lands in M13.5:** if the comment had an attached image, that image is deleted for real (`08-media-uploads.md` §8.0) after the soft-delete transaction commits — best effort, failure logged at ERROR — so `image: null` on a deleted comment reflects an actually-gone object, not just a hidden reference. The delete confirmation warns when an attached image will be permanently removed.

### 1.2 Permissions

| Action | Who |
|---|---|
| Read comments | Any group member |
| Create comment | Any group member |
| Edit own comment | Author only |
| Delete own comment | Author only |
| Delete any comment | Group admin |

Editing reopens the markdown body and sets `editedAt`. The UI marks edited comments with a `(edited)` label.

### 1.3 Validation

`body`: 0–2000 chars, allowed markdown subset (paragraphs, emphasis, code, links to https only — no `![]()` images embedded in the markdown body; instead, comments support a single optional **attached** image via `imageMediaId`).

`imageMediaId` (optional): references an `ImageMedia` row owned by the caller, in `status=ready`, with `purpose: "comment"`. The image is uploaded through the same pipeline as response images — `POST /uploads` with `purpose: "comment"`, which is valid while the cycle is `published` (matching when comments are writable; see `03-api-contract.md` §9.1 and `08-media-uploads.md` §3.1) — and rendered beneath the comment text. One image per comment.

A comment with no `body` (or whitespace-only) AND no `imageMediaId` is rejected with `VALIDATION_FAILED`.

### 1.4 Endpoints

Detailed in `03-api-contract.md` §8.1–8.4. Server-side handler highlights:

- `POST` validates cycle status = `published`, validates response exists, inserts new row with `commentId = uuidv7()`, returns the created comment with `displayName` joined in.
- `PATCH` checks the caller's resolved `userId == row.authorUserId` (JWT `sub` → `userId` via the lookup row, `05-auth-flow.md` §6 — never compare `sub` directly), updates `body` and `editedAt`. Admins cannot edit other users' comments.
- `DELETE` soft-deletes: sets `deletedAt`, blanks `body`. Allowed when the caller's `userId` matches `row.authorUserId` OR caller is a group admin. Idempotent — `DELETE` of an already-soft-deleted row returns 204; `DELETE` of a missing row returns 404. If the comment held an image, the image is permanently deleted too (decided 2026-10-07; lands in M13.5 — §1.1 above).

### 1.5 Author display name resolution

Comments embed the author's `displayName` in the response payload (denormalized read; the actual stored row only carries `authorUserId`). The handler does:

```rust
// After querying comments
let user_ids: HashSet<_> = comments.iter().map(|c| c.author_user_id.clone()).collect();
let users = repo.batch_get_users(&user_ids).await?;
let name_map: HashMap<String, String> = users.into_iter()
    .map(|u| (u.user_id, u.display_name)).collect();
// Inject into output
```

Cache the user batch per cold-start (60s TTL) to avoid repeating the read for hot cycles. **M10 decision:** v1 dedupes author IDs within a request and uses one `BatchGetItem`, with no cross-request cache. Add the TTL cache only if user reads show up as hot. (Per-group nicknames are not a v1 feature; everyone shows up under their account display name.)

Routes are addressed by `questionId` + `responseId` (`03` §8, decided M10). The attached image is claimed through `ImageMedia.attached_comment_id`, written in the same transaction as the comment, so one image can't back two comments and `DELETE /uploads` can refuse a claimed image.

Every comment edit and soft delete is also conditioned on the comment still holding the image the handler read (decided M10, post-review). The old image's release is unconditioned, so a write built on a stale read would otherwise release the wrong image and strand the new one, claimed by a `[deleted]` row. On a cancelled transaction the handler re-reads and retries, up to `COMMENT_WRITE_ATTEMPTS` (3). A PATCH whose comment still holds the image it read reports 422 on `imageMediaId` instead, because its new image was claimed elsewhere. (Decided 2026-10-07; lands in M13.5: "release" now means permanently deleting the image, not just unclaiming it — §1.1 above.)

### 1.6 Notifications on comments

**Out of scope for v1.** Future: notify the answer's author on a new comment. Tracked separately.

---

## 2. Reactions

### 2.1 Shape

```ts
type Reaction = {
  reactorUserId: string;
  emoji: string;
  createdAt: string;
};
```

The "key" of a reaction is `(answer, reactorUserId, emoji)`. The same user can react to the same answer with multiple distinct emojis. Re-reacting with the same emoji is a no-op (last-write-wins on `createdAt`).

### 2.2 Validation

`emoji` is a string of 1–12 codepoints. The server tolerates any Unicode but rejects ASCII-only strings (length 1–12 after normalization, with at least one codepoint that is in the `\p{Extended_Pictographic}` Unicode property OR a regional-indicator). This rules out the path containing letters/digits while allowing arbitrary emoji including ZWJ sequences (👨‍👩‍👧‍👦 = 7 codepoints).

Server normalizes the emoji to NFC and uses the normalized form as the storage key.

### 2.3 Endpoints

Detailed in `03-api-contract.md` §8.5–8.7.

- `PUT /...reactions/{emoji}`: idempotent put of a URL-encoded emoji
- `DELETE /...reactions/{emoji}`: idempotent delete
- Both return the answer's reactions after the change, in the `GET` shape (decided M10).
- `reactionGroups` order: count descending, then earliest reaction, then emoji. The inline newsletter view uses the same order.
- `GET /...reactions` — returns:
  ```json
  {
    "reactionGroups": [
      { "emoji": "🔥", "count": 4, "reactedByMe": true },
      { "emoji": "🤣", "count": 2, "reactedByMe": false }
    ],
    "myReactions": ["🔥"]
  }
  ```

In practice the published-newsletter endpoint (`GET /groups/{g}/newsletters/{c}`) returns reactions inlined per answer; the dedicated reactions endpoints are mainly used for live updates after toggling.

### 2.4 Live update strategy

After PUT or DELETE, the SPA optimistically updates local state, then revalidates by invalidating the newsletter query (or just the affected answer's reactions, if we expose a lightweight per-answer reactions query — we'll do this for hot cycles).

**M10 decision:** the SPA patches the answer's `reactionGroups` in the cached newsletter optimistically. On success it replaces them with the server's `ReactionsResponse`, and on error it rolls back. No newsletter refetch is needed. Comment mutations patch the cache and then invalidate the newsletter query.

No WebSocket / live push for v1. Users see other reactions on next page load or pull-to-refresh.

### 2.5 Picker UI

`EmojiPickerPopover`: a curated 64-emoji grid plus a native text input filtered to one emoji (on mobile the input brings up the keyboard's emoji panel). Decided M10; `emoji-picker-element` was dropped, cutting the lazy chunk from about 150 kB to 2.6 kB.

Frequently-used reactions render as one-tap pills next to the picker button: `🔥 🤣 ❤️ 😍 👍 🎉`.

---

## 3. Polls

A poll is a *kind* of question. Its lifecycle differs slightly from text questions:

| Phase | Text question | Poll question |
|---|---|---|
| Suggest (voting phase) | Members type a free-form prompt | Members type a prompt + 2–6 option labels |
| Lock (cycle opens) | Becomes a prompt | Becomes a prompt with options |
| Respond (cycle open) | Members write text + images | Members select **one** option; last-write-wins |
| Publish | All text answers visible; comments + reactions on each answer | Per-option tallies + caller's vote. No comments or reactions on the poll widget itself — engagement lives on text answers only. |

### 3.1 Storage

Question side: `LockedQuestion.kind = "poll"`, `pollOptions = [{optionId, label}]`.

Response side: `Response.kind = "poll"`, `Response.pollOptionId = "01H..."`. Same row family as text responses (`pk=GROUP#g#NL#c#Q#q sk=A#userId`). One row per (user, question).

Tally is computed at read time (not denormalized): the `GET /groups/{g}/newsletters/{c}` handler queries all responses for each poll question and groups by `pollOptionId`. With 50 members × 5 questions = 250 reads per newsletter view; trivially cheap.

For cycles with very large groups (>200) this would warrant denormalized tallies. Out of scope for v1.

### 3.2 Visibility during the response window

- A user who has not voted: sees the prompt + options, no tally.
- A user who has voted: sees their own selection. **No mid-window tally** for any user (avoids vote-anchoring). Tally appears on publish.
- Admins also do not see the tally during the response window. Discoverable via DDB query if needed; not exposed in UI.

### 3.3 Validation

- `pollOptions` length 2–6 at suggestion time.
- Labels 1–80 chars (after trimming), no duplicates. The duplicate check is **case-insensitive** (trimmed labels compared after Unicode lowercasing), so "Yes" and "yes" are the same option and the second is rejected with `VALIDATION_FAILED` on `pollOptions`. The label is stored trimmed, in the case the suggester typed. Implemented in `lambda-questions/src/validation.rs` and mirrored by `SuggestQuestionForm` and the mock API.
- A user's `pollOptionId` on submit must match one of the question's option IDs.
- Option labels are immutable once a candidate is promoted into a cycle; whatever the suggester typed is what the group votes on.

### 3.4 Vote lock at publish

While the cycle is `open`, a user can change their vote freely (overwrites their existing `Response` row). When the cycle transitions to `published`, votes are frozen — the published-newsletter endpoint omits any vote-mutation surface, the SPA's poll widget is read-only, and any late `PUT my-response` for a poll question returns 409 `CYCLE_NOT_OPEN`. The user's last pre-publish selection is the one that gets counted.

### 3.5 Anonymity

Poll votes are **not anonymous** — the tallies are. The `Response` row records `userId`. The published newsletter's poll widget shows aggregate counts only; individual votes are never exposed in any response payload (the server filters them out for poll questions when assembling the published view).

This matches user expectation: "what did people vote for?" shows totals, not "Quinn voted for X."

### 3.6 Auto-publish at close

**Decided 2026-10-07; lands in M13.5:** when the response window closes, `lambda-cycle-tick` publishes every non-empty draft — text (non-whitespace `body` or a non-empty `imageMediaIds`) and poll (a `pollOptionId` set) alike — before flipping the cycle to `published` (`06-newsletter-lifecycle.md` §8). An empty draft stays `draft` and is excluded, same as today. The explicit Publish button is unchanged in meaning ("I'm done"); this just means an unpublished-but-non-empty answer isn't lost at the deadline. See `04-frontend-architecture.md` §7.3 for the RespondPage warning copy.

---

## 4. Per-feature pages and components

(Cross-references `04-frontend-architecture.md`.)

### 4.1 `AnswerCard.tsx`

Renders one published answer with:
- Author avatar + display name
- Markdown-rendered body
- `<ImageGallery>` for attached images (lightbox on click)
- `<ReactionBar>`
- `<CommentList>` (collapsible; show 3, "Show all" expands)

### 4.2 `ReactionBar.tsx`

```
[🔥 4] [🤣 2] [❤️ 1] [+]
```

Each pill toggles on click. The `+` opens the picker; submitting an emoji POSTs and inserts the pill.

### 4.3 `CommentList.tsx`

```
Quinn — 2 hours ago
> "Yeah I had the same experience..."

Alex — 1 hour ago (edited)
> "..."

[Reply textarea]
```

Edit and Delete are inline (`...` menu on author's own comments; admin sees Delete on all). **Decided 2026-10-07; lands in M13.5:** Delete goes through a `ConfirmDialog`; when the comment has an attached image the copy adds "This also permanently deletes the attached image. It can't be recovered." (`08-media-uploads.md` §8.0).

### 4.4 `PollWidget.tsx`

Two states:

**Voting (during response window):**
```
○ Mt Si
● Lake 22       <- selected
○ Mailbox Peak
○ Wallace Falls
[Publish]
```

**Decided 2026-10-07; lands in M13.5 (supersedes the M9 "a pick is local only before first publish" behaviour):** every pick saves immediately as a draft (`publish:false`), including the first one. This also means an unpublished pick auto-publishes at the response-window deadline (§3.6 below). Publish still exists and marks the answer `published:true`. Last-write-wins (`04` §7.3).

**Tally (after publish):**
```
Mt Si        ████░░░░░░  4 (33%)
Lake 22      ████████░░  7 (58%) ← your vote
Mailbox Peak █░░░░░░░░░  1 (8%)
Wallace Falls░░░░░░░░░░  0 (0%)
```

Bars are pure CSS (no chart library). Caller's pick gets the ring highlight.

---

## 5. Sanitization

All user-authored markdown (responses + comments) goes through a strict allowlist sanitizer:

- Allowed elements: `p, em, strong, code, pre, ul, ol, li, blockquote, a, br, hr, h1..h6, img` (img only for `image:{id}` tokens, rendered as `CdnImage`; any other image URL renders as a link — decided M9, `04` §9). Comments render with `allowImages={false}`, so no inline images at all.
- Allowed attributes: `href` on `a` (https only, no `javascript:`/`data:`); `src/alt/width/height` on `img`; `class` is stripped.
- Forbidden: `script, style, iframe, object, embed, form, input, button, svg, math` and any `on*` attributes.

Implemented in `frontend/src/utils/markdown.tsx` using `rehype-sanitize` with a custom `defaultSchema`.

The backend does NOT sanitize on write — sanitation is a render-time concern. We accept that the raw markdown can contain anything; the client-side render makes it safe. (If we ever add a server-rendered email digest, that renderer must apply the same sanitizer.)

---

## 6. Indexes & access patterns recap

Already in `02-data-model-dynamodb.md`:

- Comments — AP18, base index, sk prefix `C#{ts}#{id}` (oldest first)
- Reactions — AP19, base index, sk prefix `R#{user}#{emoji}`
- Poll vote — single item per user per question, GetItem on `A#{user}` (the `Response` row carries the vote)

No new GSIs needed.

---

## 7. Limits

| Limit | Value | Why |
|---|---|---|
| Comment body | 2000 chars | Long enough for thoughtful replies, not a runaway. |
| Comments per answer | unbounded | Activity-driven; will revisit if a runaway happens. |
| Reaction emoji length | 12 codepoints | Allows ZWJ family/profession sequences. |
| Reactions per user per answer | unbounded | Each emoji is its own row; no need to cap. |
| Poll options | 2–6 | Sweet spot for legibility. |
| Poll labels | 1–80 chars | Fits on a single line in the bar widget. |

---

## 8. Tests required

Specified in `11-testing-ci-cd.md`. Engagement-specific tests:

1. Comment create / edit / soft-delete round trips.
2. Non-author cannot edit / delete (403).
3. Admin can delete any comment.
4. Reaction PUT/DELETE idempotent.
5. Last-write-wins on poll vote (overwrite).
6. Poll vote visibility: caller sees own pre-publish; tally hidden until publish.
7. Markdown sanitizer rejects `<script>` and `javascript:` URLs in comments.
8. Reaction emoji with ZWJ sequence stores and retrieves correctly.
9. Comment list ordering oldest-first across pagination boundaries.
