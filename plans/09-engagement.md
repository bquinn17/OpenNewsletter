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
  authorDisplayName: string;
  body: string;          // 1-2000 chars; markdown allowed (sanitized identical to response bodies)
  createdAt: string;
  editedAt: string | null;
  deletedAt: string | null;
};
```

A deleted comment shows as `"[deleted]"` in the UI but its row remains for audit (soft delete). Hard delete is reserved for compliance and not exposed in v1.

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

`body`: 1–2000 chars, must not be only whitespace, allowed markdown subset (paragraphs, emphasis, code, links to https only — no images embedded; if a user wants to attach an image, that's a future feature).

### 1.4 Endpoints

Detailed in `03-api-contract.md` §8.1–8.4. Server-side handler highlights:

- `POST` validates cycle status = `published`, validates response exists, inserts new row with `commentId = uuidv7()`, returns the created comment with `authorDisplayName` joined in.
- `PATCH` checks `claims.sub == row.authorUserId` (or admin), updates `body` and `editedAt`.
- `DELETE` (soft): sets `deletedAt = now()`, blanks `body`. Idempotent (re-delete of a deleted comment is a 200).

### 1.5 Author display name resolution

Comments embed `authorDisplayName` in the response payload (denormalized read; the actual stored row only carries `authorUserId`). The handler does:

```rust
// After querying comments
let user_ids: HashSet<_> = comments.iter().map(|c| c.author_user_id.clone()).collect();
let users = repo.batch_get_users(&user_ids).await?;
let mut name_map: HashMap<String, String> = users.into_iter()
    .map(|u| (u.user_id, u.display_name)).collect();
// Apply per-group nicknames if present
let memberships = repo.list_group_members(&group_id, &user_ids).await?;
for m in memberships { if let Some(n) = m.nickname { name_map.insert(m.user_id, n); } }
// Inject into output
```

Cache the membership list per cold-start (60s TTL) to avoid the second query for hot cycles.

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

- `PUT /...reactions/{emoji}` — idempotent put; URL-encoded emoji
- `DELETE /...reactions/{emoji}` — idempotent delete
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

No WebSocket / live push for v1. Users see other reactions on next page load or pull-to-refresh.

### 2.5 Picker UI

Native browser emoji input (`<input type="text">` filtered to one emoji). On mobile this brings up the keyboard's emoji panel. On desktop, fall back to a small built-in picker (a tiny library like `emoji-picker-element` is acceptable; alternatively a curated 64-emoji grid).

Frequently-used reactions render as one-tap pills next to the picker button: `🔥 🤣 ❤️ 😍 👍 🎉`.

---

## 3. Polls

A poll is a *kind* of question. Its lifecycle differs slightly from text questions:

| Phase | Text question | Poll question |
|---|---|---|
| Suggest (voting phase) | Members type a free-form prompt | Members type a prompt + 2–6 option labels |
| Lock (cycle opens) | Becomes a prompt | Becomes a prompt with options |
| Respond (cycle open) | Members write text + images | Members select **one** option; last-write-wins |
| Publish | All text answers visible | Per-option tallies + caller's vote |

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
- Labels 1–80 chars, no duplicates (case-insensitive).
- A user's `pollOptionId` on submit must match one of the question's option IDs.
- Admins can edit option labels via `PATCH /admin/.../questions/{q}` while cycle is `open`. Once `published`, all options become immutable.

### 3.4 Anonymity

Poll votes are **not anonymous** — the tallies are. The `Response` row records `userId`. The published newsletter's poll widget shows aggregate counts only; individual votes are never exposed in any response payload (the server filters them out for poll questions when assembling the published view).

This matches user expectation: "what did people vote for?" shows totals, not "Quinn voted for X."

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
- Edit affordances if `claims.sub == answer.userId` and cycle is still `open` (rare on a published view, but the edit button could appear briefly mid-transition)

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

Edit and Delete are inline (`...` menu on author's own comments; admin sees Delete on all).

### 4.4 `PollWidget.tsx`

Two states:

**Voting (during response window):**
```
○ Mt Si
● Lake 22       <- selected
○ Mailbox Peak
○ Wallace Falls
[Save]                 <- only when changed
```

Last-write-wins; saving overwrites.

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

- Allowed elements: `p, em, strong, code, pre, ul, ol, li, blockquote, a, br, hr, h1..h6, img` (img only with `src` matching the `image:` token or `https://cdn.opennewsletter.example.com/...`)
- Allowed attributes: `href` on `a` (https only, no `javascript:`/`data:`); `src/alt/width/height` on `img`; `class` is stripped.
- Forbidden: `script, style, iframe, object, embed, form, input, button, svg, math` and any `on*` attributes.

Implemented in `frontend/src/utils/markdown.ts` using `rehype-sanitize` with a custom `defaultSchema`.

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
