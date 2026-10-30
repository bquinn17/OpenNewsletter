# Future ideas

Features the owner wants kept but that aren't built. Nothing here is scheduled. Each entry says what the feature is, why it's deferred and what it would touch, so it can be picked up later without re-deriving the design. Deferring the first three was decided in [D11](decisions.md#d11) and [D12](decisions.md#d12).

---

## 1. Threaded (nested) comments

The original spec asked for "nested comments". Comments are flat ([`engagement.md` §1](design/engagement.md#1-comments)) because a small group's threads are short and flat lists are simpler to render on mobile.

If added:
- `Comment` gains `parent_comment_id` (null = top level). Keep one level of nesting (replies to a top-level comment), not arbitrary depth.
- Keys can stay as they are (`C#{created_at}#{comment_id}` in the answer's partition). The client groups replies under their parent, since a thread is at most a few dozen rows.
- Soft-deleting a parent keeps its `[deleted]` placeholder so replies stay anchored. That works because soft delete already exists.
- API: optional `parentCommentId` on `POST …/comments`; validate that the parent is in the same answer and is top-level.

## 2. Comments and reactions on poll questions

Poll questions show only tallies, and engagement attaches to text answers only ([`engagement.md` §3](design/engagement.md#3-polls), [`api.md` §8](design/api.md#8-engagement-routes-comments--reactions)).

If added:
- **Poll-level discussion.** A poll has no per-user answer worth commenting on, so comments and reactions would attach to the *question*. A new partition `GROUP#{g}#NL#{c}#Q#{q}#POLL` would mirror the per-answer comment/reaction rows.
- **Routes** nested as `…/questions/{questionId}/comments` and `…/questions/{questionId}/reactions/{emoji}`, with the same check order as [`api.md` §8](design/api.md#8-engagement-routes-comments--reactions) (published, not archived, the question is a poll).
- **Optional "show who voted"**, per poll and opted into by the suggester at suggestion time. It's off by default, because [`engagement.md` §3.5](design/engagement.md#35-anonymity) promises aggregate-only tallies.

## 3. Ephemeral per-PR CI stacks

Today `e2e.yml` runs against the long-lived `dev` stack ([`environments.md` §11](development/environments.md#11-ephemeral-ci-stacks)), and `config.py` knows only `dev` and `prod`.

The design, if per-PR isolation becomes worth the cost:
1. A PR opens (or `e2e` is run manually), and the workflow assumes the deploy role via OIDC.
2. `cdk deploy --context env=ci-pr-{n} --all` creates the stacks. They reuse the persistent `dev` `AuthStack`, so Cognito isn't recreated.
3. Playwright runs against the new stack.
4. When the workflow finishes, pass or fail, `cdk destroy --context env=ci-pr-{n} --all --force` removes it.

Safety nets:
- **Sweeper Lambda**: runs daily and destroys any `ci-pr-*` stack older than 24h.
- **Budget**: the account-wide AWS Budget ([`infrastructure.md` §9](design/infrastructure.md#9-monitoringstack)) catches stranded resources.

Prerequisites: `config.py` must accept `ci-pr-*` envs, and `ApiStack` and the media stacks must be able to point at an existing user pool. Each stack costs about $0.50 and stands up and tears down in minutes.

## 4. Other deferred items (pointers)

Already designed elsewhere, listed here so this file is the one place to look:
- **Archival** of old editions: [`archival.md`](design/archival.md).
- **Inactive Cognito account cleanup**: [`archival.md` §6](design/archival.md#6-inactive-account-cleanup-separate-but-related).
- **Comment notifications** to the answer's author: [`engagement.md` §1.6](design/engagement.md#16-notifications-on-comments).
- **Unread tracking** on the newsletter list: [`notifications.md` §14](design/notifications.md#14-failure-modes--ux).
- **Offline draft background sync**, periodic prefetch and `share_target`: [`frontend.md` §14.9](design/frontend.md#149-not-built).
- **Self-hosted fonts**, for offline use and to drop the Google Fonts CSP origins: [`frontend.md` §12.1](design/frontend.md#121-content-security-policy).
- **"Hype check" banner** and **recurring-question helper text**: [`frontend.md` §14a](design/frontend.md#14a-client-derived-display-data-not-api-fields).
- **Markdown formatting toolbar** in the response editor. The editor has Write and Preview tabs only; the gap is tracked in [`PROGRESS.md`](../plans/PROGRESS.md) under Known gaps.

## 5. Group leaderboard

A page of light-hearted superlatives for each group: the most-reacted answer, the most editions answered, the longest answering streak, and so on. It's for fun in a small group, so each board shows the top few members and never ranks anyone last. Only the group's members can see it.

The boards:
- **Most-reacted answer** (all time): the answers with the most reactions, each linking to its edition.
- **Most reactions received**: the total across all of a member's answers.
- **Most editions answered**: `GroupMembership.editions_answered`, which already exists ([`data-model.md` §2.2](design/data-model.md#22-groupmembership-also-serves-as-group--user-index-via-gsi1)).
- **Longest streak**, with the current streak beside it. A streak counts consecutive editions in which the member published at least one answer.
- Cheap extras once the counters exist: most questions asked (counting only promoted questions that weren't anonymous, so the board can't unmask anyone), most comments written, most places checked into (§6).

**Counters are kept at write time, not computed on read.** Computing on read would mean reading every edition's engagement partitions on each page view, so the cost would grow with the group's history. One published edition already needs concurrent read waves ([`api.md` §5.2](design/api.md#52-get-groupsgroupidnewsletterscycleid)).
- **A `MemberStats` row** for each member of each group: `pk=GROUP#{g}#STATS`, `sk=MEMBER#{u}`. It holds `reactions_received`, `reactions_given`, `comments_written`, `questions_asked`, `current_streak`, `longest_streak` and `streak_cycle`. These go on their own row, not on `GroupMembership`, for two reasons: the stats survive a leave and rejoin, and an `ADD` upsert can't recreate a membership row that a leave deleted.
- **Reactions.** `PUT …/reactions/{emoji}` becomes a transaction:
  - Put the reaction, conditioned on `attribute_not_exists(sk)`.
  - `ADD reaction_count 1` on the Response row.
  - `ADD` on the author's and the reactor's `MemberStats`.

  If the Put's condition fails, the request is a duplicate and returns success, as a duplicate vote does ([`data-model.md` §4](design/data-model.md#4-transactional-invariants) #1). `DELETE` does the reverse: it requires `attribute_exists(sk)` and subtracts 1.
- **Most-reacted answer** needs a top-N across the whole group. The Response row gets `gsi2pk=GROUP#{g}#REACTED` and `gsi2sk={zero-padded reaction_count}#{response_id}`, rewritten in the same transaction and conditioned on the prior count. Candidate questions already rank by vote count in GSI1 the same way ([`data-model.md` §2.6](design/data-model.md#26-candidate-question-in-voting-pool)). A Query with `ScanIndexForward=false` and `Limit=3` returns the top three. The tick's GSI2 queries use `gsi2pk=NL_STATUS#…`, so these rows don't show up in them.
- **Streaks are settled once per edition**, inside `publish(nl)`, after the auto-publish sweep and before the status flip ([`lifecycle.md` §5.3](design/lifecycle.md#53-publishnl)). At that point the set of authors is final, and the sweep's AP14 reads have already listed them.
  - For each current member (AP3): a member who answered gets `current_streak + 1`, and `longest_streak` is raised if the new streak beats it. A member who didn't answer gets `current_streak = 0`.
  - Each write sets `streak_cycle = :c`, conditioned on `streak_cycle <> :c`, so a rerun after a crashed tick skips members it already updated.
  - An edition with no locked questions ([`lifecycle.md` §9](design/lifecycle.md#9-empty-cycle-edge-cases)) is skipped: it neither extends nor breaks a streak.
  - Leaving the group resets `current_streak` in the leave transaction, so a member who rejoins doesn't pick up a stale streak.
- **API.** `GET /groups/{groupId}/leaderboard` in `lambda-groups`, open to any member. It returns each member's raw stats keyed by `userId`, plus the top three answers with their question prompt and cycle ID. The client joins the stats with the member list from `GET /groups/{groupId}` for names and avatars, then builds and orders the boards, so a new board that uses an existing stat needs only a frontend change.
- **Frontend.** `/g/:groupId/leaderboard`, linked from the members page (§7) and from the published edition's header.
- **Backfill.** A one-off `scripts/backfill_member_stats.py` recomputes the counters and streaks from history when the feature ships. It sets absolute values, so it's safe to run again.
- **Open questions.**
  - Can an admin switch the leaderboard off for their group? That would be a `Group.leaderboard_enabled` flag.
  - Archival ([`archival.md`](design/archival.md)) prunes old Response rows, and their GSI2 entries go with them. The most-reacted board would then cover only unarchived editions, unless archival snapshots the top answers first.

## 6. Check-ins (places on answers)

A member can attach a place to any text answer ("📍 Lucali, Brooklyn"). A question can also be a **check-in question**, such as "Check into a place you really loved visiting this month", whose answers lead with the place. A published check-in question shows its answers' places on a map.

**Data.**
- `Response.place` holds `{ name, lat, lng, address, provider_place_id }`. It's optional and only on text answers. It's stored exactly as the member picked it, so a published answer never changes when the provider's data does.
- `checkin: bool` goes on `CandidateQuestion`. At promotion it's copied to `LockedQuestion`, as `is_anonymous` is.
- **A check-in question stays `kind=text`.** It isn't a new `kind`, because installed PWAs run the previous release until their service worker updates. That frontend branches on `kind` in several places, some of them as `=== "text"`. Given a check-in question, it renders the answers as ordinary text answers; a new enum value would be one it doesn't know.
- **The server never requires a place**, even on a check-in question. The new editor does, by keeping Publish disabled until a place is picked. That way old clients can still publish. It also keeps the auto-publish "non-empty" rule simple: a draft with a `place` counts as non-empty ([`data-model.md` §4](design/data-model.md#4-transactional-invariants) #4a).

**API.**
- `PUT …/my-response` accepts an optional `place`, and `null` clears it. Validation: `name` is 1–120 characters, and `lat`/`lng` are in range.
- `PublishedAnswerResponse` gains `place`. The candidate, open-question and published-question responses gain `checkin`. Every change is a new field, so this is an expand-only change.
- **Rollout order:** first ship the frontend that renders places. The suggest form's "Check-in" option ships after it.

**Place search goes through the backend.**
- The route is `GET /places/search?q=&near=lat,lng`, in a new `lambda-places` crate that follows the crate template in `CLAUDE.md`. Any caller with a `userId` can use it.
- The Lambda calls the provider with IAM credentials. No search key ships in the bundle, and every search sits behind the Cognito authorizer.
- A per-route API Gateway throttle caps cost. The client waits until there are at least 3 characters and 300 ms without typing before it searches.
- **"Use my location"** asks the browser's Geolocation API only when the member taps it. The result is reverse-geocoded into suggestions for the member to pick from. Nothing is captured automatically.
- Amazon Location Service is the AWS-native choice. Its terms on storing results need checking first, because it prices results you keep differently from single-use ones.

**Display.**
- Each answer card shows the place as a chip that opens it in a maps app.
- A published check-in question shows a map above its answers, with each author's face as the marker. The members map (§7) uses the same map component.

**Map rendering** (shared with §7) is the main open decision:
- **Tile source.** The browser fetches tiles itself, so tile traffic never reaches Lambda and the idle-backend principle holds. Options are Amazon Location Maps, with an API key limited to the app's origin and created in CDK, or a hosted tile provider. Either way:
  - The provider's origin is added to the CSP's `img-src`/`connect-src` ([`frontend.md` §12.1](design/frontend.md#121-content-security-policy)).
  - The map carries the provider's attribution.
  - The provider has no fixed monthly charge, and small-group usage stays inside the [D7](decisions.md#d7) budget of about $1–2 a month.
- **Renderer.** The app ships as one bundle with no lazy chunks ([`frontend.md` §14.2](design/frontend.md#142-service-worker)), so every page load carries the map library. Leaflet with raster tiles is about 40 KB gzipped. MapLibre GL, which draws vector tiles, is several times larger, and it needs its CSP build so that `worker-src` can stay `'self'`. Prefer Leaflet if the chosen tile source serves raster tiles.
- Record the choice in [`decisions.md`](decisions.md).

**Privacy.** Only the group can see a place, the same as the answer it's attached to. Uploaded photos already lose their GPS metadata during processing ([`media.md` §5.1](design/media.md#51-trigger-configuration)), so the only places an answer can carry are ones the member chose.

## 7. Members page and hometown map

A page that any member can open from an edition. It lists who's in the group, and its map view puts each member's face on their hometown.

**List.**
- `/g/:groupId/members`, opened from an avatar stack ("12 members") in the edition page's header. It links to the leaderboard (§5).
- Each row shows the member's avatar, display name, role, the month they joined and the editions they've answered.
- **No new route is needed.** `GET /groups/{groupId}` already returns `members`, with `avatarUrl`, `avatarColor`, `joinedAt` and `editionsAnswered`, to any member ([`api.md` §4.2](design/api.md#42-get-groupsgroupid)). Today only the admin page's Members tab displays it.

**Hometown** is a profile field, set in Settings → Profile with the place search from §6, limited to cities and towns.
- `User.hometown` holds `{ name, lat, lng, provider_place_id }`, or is absent.
- **City-level only.** The search returns only cities and towns, and the stored point is the town's centre, so the map never shows a street.
- **Opt-in.** It's unset by default. Because it lives on the User row, every group the member belongs to sees it, and the Settings copy says so. Hiding it from a single group could come later if someone asks.
- **API.** `PATCH /me` accepts `hometown`, and `null` clears it. `UserResponse` and `MemberResponse` gain it. Both are new fields, so this is an expand-only change.

**Map view.** The members page gets a List | Map toggle that uses the map component from §6.
- Markers are the members' avatars: the presigned `avatarUrl`, or the colored initial when there isn't one.
- Members from the same town share one marker, which spreads out into their faces when tapped. A group has at most about 50 members, so it doesn't need a clustering library.
- The map zooms to fit all the markers. Members without a hometown are listed below it.
- Avatar URLs are presigned and expire. Markers should go through `useMediaRetry`, the retry policy `MediaImage` and `Avatar` use, rather than plain `<img>` tags ([`media.md` §4.4](design/media.md#44-spa-usage)).

New glossary terms when §5–§7 are built: *Check-in*, *Hometown*, *Leaderboard* ([`overview.md` §5](overview.md#5-glossary-use-these-terms-exactly)).
