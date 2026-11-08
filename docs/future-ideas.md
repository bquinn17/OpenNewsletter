# Future ideas

Features the owner wants kept but that aren't built. Nothing here is scheduled. Each entry says what the feature is, why it's deferred and what it would touch, so it can be picked up later without re-deriving the design. Deferring the first three was decided in [D11](decisions.md#d11) and [D12](decisions.md#d12).

---

## 1. Reply comments

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

## 8. Voice clips on answers and comments

A member records a short voice clip in the app, or picks an audio file, and attaches it to a text answer or a comment. Readers play it inline. It's for the answers that are easier said than typed: a story, a laugh, a song you can't stop humming.

**Limits** (new constants in `backend/crates/shared/src/config.rs`): up to 3 clips per text answer and 1 per comment. Each clip is at most 2 minutes (`MAX_AUDIO_SECONDS = 120`) and 25 MB as uploaded (`MAX_AUDIO_BYTES`), which is enough for a 2-minute WAV picked from Files. A caption is optional and capped at 140 characters, the same as an image caption.

### 8.1 Recording UX

- **Entry point.** Two items in the "+" attach menu (§10), in both the response editor and the comment composer. **Record a voice clip** starts the recorder below. **Upload audio** opens an `<input type="file" accept="audio/*">`, so a voice memo exported to Files works on iOS. If `MediaRecorder` or `getUserMedia` is missing, the menu hides **Record a voice clip** and keeps **Upload audio**.
- **Permission is asked on the first tap only.** If it's denied, an inline note says: "Microphone is blocked. Allow it in your browser settings, or upload a recording instead," with the upload link.
- **While recording**, a recorder bar replaces the composer's attachment row:
  - a pulsing red dot and the elapsed time against the cap ("0:42 / 2:00"); the timer turns coral in the last 15 seconds
  - a live level meter of about 12 bars, drawn from a Web Audio `AnalyserNode`, so the member can see the mic is picking them up
  - **Stop**, the primary button with a 48 px tap target, and **Cancel**, which discards the clip without a confirm because nothing has been uploaded yet

  Recording stops by itself at the cap, with a toast: "That's the 2-minute limit." Recording is tap to start and tap to stop, not hold to talk, because 2 minutes is too long to hold a button. A Wake Lock is held while recording where the browser supports one, because a phone that locks its screen ends the recording.
- **After Stop**, a clip card appears with:
  - play/pause, a waveform drawn from the local blob (`decodeAudioData`) and the duration
  - **Re-record** and **Remove**
  - an optional "Describe this clip" field (the caption), which is also the clip's accessible label

  The upload starts the moment recording stops, and the card shows the same uploading → processing → ready/failed states as an image tile ([`media.md` §3.3](design/media.md#33-frontend-uploader-imageuploadertsx)), with **Retry** on failure. Publishing follows the same rule as images: a clip joins the draft once it's `ready`.
- **Not losing a clip.** The recorded `Blob` stays in memory until its upload succeeds. Leaving the page with a clip that hasn't uploaded asks first. The draft mirror ([`frontend.md` §8.3](design/frontend.md#83-offline--failure)) doesn't hold audio, because it's too big for `localStorage`. An IndexedDB stash could come later.

### 8.2 Playback UX

A `VoiceClip` component goes in `AnswerCard`, below the body and above the gallery, and in `CommentList`, below the comment's text.

```
 ( ▶ )  ▂▅▇▅▃▂▆▇▅▃▂▁▃▅▇▆▄▂▁▂▄▆▅▃   0:12 / 1:04   1.5×
        "The time the tent blew into the lake"
```

- **Controls.** The round play button uses the author's avatar color. The waveform shows 48 bars, and the part already played is filled in. Tapping or dragging on the waveform seeks. The speed toggle cycles 1× → 1.5× → 2×.
- **Waveform and duration come from the API**, so the card draws at full size without downloading any audio. `<audio preload="none">` means a page with twenty clips downloads nothing until someone presses play.
- **One clip plays at a time.** Starting one pauses whichever was playing, through a small shared store.
- **Accessibility.** The waveform is a `role="slider"` with `aria-valuetext` ("12 seconds of 1 minute 4"). Space toggles play and ←/→ seek 5 seconds. The label is "Voice clip from Alex, 1 minute 4 seconds", followed by the caption when there is one.
- **An expired URL** fires the `<audio>` `error` event, which goes through `useMediaRetry` exactly as an image's load error does ([`media.md` §4.4](design/media.md#44-spa-usage)). Offline, a clip that isn't cached shows a disabled play button: "Available when you're back online."

### 8.3 Storage and processing

It's the image pipeline ([`media.md` §3](design/media.md#3-upload-flow-browser--s3)) with an audio processor in place of the image one: a presigned PUT straight to the originals bucket, an S3 trigger, a processed copy, and presigned GETs inline in API responses. Lambda is never on the bytes' path.

- **Routes** in `lambda-media`: `POST /audio-uploads`, `GET /audio-uploads/{audioId}` and `DELETE /audio-uploads/{audioId}`. They use the same `?groupId=&cycleId=` addressing ([`api.md` §9.0](design/api.md#90-addressing-an-existing-upload)), the same `purpose` (`response` | `comment`) and the same cycle-status checks as `POST /uploads` ([`api.md` §9.1](design/api.md#91-post-uploads)). They're separate routes rather than a `kind` on `/uploads`, because the response shapes differ (duration and peaks, no dimensions). The MIME allowlist is a first filter only: `audio/webm`, `audio/ogg`, `audio/mp4`, `audio/x-m4a`, `audio/aac`, `audio/mpeg`, `audio/wav`, `audio/x-wav`. Parameters such as `;codecs=opus`, which `MediaRecorder` adds, are stripped before the check.
- **An `AudioMedia` row** mirrors `ImageMedia` ([`data-model.md` §2.10](design/data-model.md#210-imagemedia)):
  - keys: `pk=GROUP#{g}#NL#{c}`, `sk=AUD#{audioId}`, `gsi1pk=USER#{u}#AUD`
  - fields: `purpose`, `mime_type`, `original_key`, `clip_key`, `status`, `bytes`, `duration_ms`, `peaks` (48 integers 0–255, about 200 bytes), `caption`, `attached_comment_id` and `error_message`, which is `AUDIO_TOO_LARGE`, `AUDIO_TOO_LONG` or `AUDIO_DECODE_FAILED`
- **Where clips attach.**
  - `Response.audio_media_ids` holds up to 3 IDs, on text answers only.
  - `Comment.audio_media_id` holds one.
  - A comment needs a body, an image or a clip ([`engagement.md` §1.3](design/engagement.md#13-validation)).
  - The auto-publish "non-empty" rule ([`data-model.md` §4](design/data-model.md#4-transactional-invariants) #4a) gains "or `audio_media_ids` non-empty", so an answer that's only a clip publishes at close.
  - Clips render as attachments, not inline markdown tokens, so the sanitizer and `urlTransform` stay as they are.
- **S3 keys.** Originals go in the existing media-originals bucket under a separate prefix, `audio-uploads/{g}/{c}/{q}/{u}/{audioId}.{ext}`, so `lambda-image-process`'s `uploads/` trigger never fires for audio. The bucket's 7-day expiry rule ([`media.md` §8.1](design/media.md#81-orphan-originals)) extends to that prefix. Processed clips go to `aud/{g}/{c}/{q}/{u}/{audioId}/clip.webm` in the processed bucket. The signing Lambdas that hold `img/*` today get `s3:GetObject` on `aud/*` too, scoped by prefix as images are.
- **`lambda-audio-process`** is a new crate laid out like `lambda-image-process`, triggered on `audio-uploads/`. Per object it:
  1. checks the object's real size against `MAX_AUDIO_BYTES`
  2. runs `ffprobe` to confirm there's an audio stream and read its duration. It never trusts the extension or MIME type, and anything over the cap (plus 1 second of slack) fails with `AUDIO_TOO_LONG`
  3. transcodes to mono Opus at 24 kbps in a WebM container: `ffmpeg -vn -map_metadata -1 -ac 1 -ar 48000 -c:a libopus -b:a 24k -vbr on -application voip -cues_to_front 1 clip.webm`. `-application voip` tunes the encoder for speech, which is what clips mostly are. `-map_metadata -1` strips tags and any location from the recording. `-cues_to_front 1` puts the seek index at the start, so the player can seek before the file has fully downloaded
  4. decodes to PCM and buckets the peak amplitude into 48 bars for the waveform
  5. puts the clip with `Cache-Control: private, max-age=5184000, immutable`, marks the row `ready` on the `status = pending` condition, then deletes the original. Failure, retry and duplicate-delivery handling are the same as images ([`media.md` §5.1](design/media.md#51-trigger-configuration))

  `ffmpeg`/`ffprobe` are static arm64 builds bundled in the Lambda zip, or in a layer. An LGPL build with `libopus` (BSD-licensed) is enough; no GPL or non-free codecs are needed. Memory is 1024 MB and the timeout 60 s; a 2-minute clip transcodes in a second or two.
- **Why Opus in WebM.** Opus is the most storage-efficient audio codec browsers play. At 24 kbps mono, speech is clear, which is roughly a third of the bitrate AAC needs for the same quality. WebM is the container Safari plays it in; Safari doesn't play Ogg at all.
  - **Browser support:** Chrome, Edge and Firefox play Opus in WebM, and so does Safari 17+. On iOS it plays from 17.4, and caniuse marks iOS fully supported from 18.4.
  - **Older devices:** `VoiceClip` checks `audio.canPlayType('audio/webm; codecs="opus"')`. Where that's empty it shows the duration and "Update your browser to play voice clips", not a broken player. If older iPhones turn out to matter, the fallback is a second AAC rendition, at roughly 3× the storage per clip.
- **Why transcode at all.** Browsers record in different formats: Chrome and Firefox produce WebM/Opus, Safari produces MP4/AAC. Picked files can be anything, including large WAVs. Serving a single format means a single playback and caching story.
  - The recorder still asks `MediaRecorder` for `audioBitsPerSecond: 32000`, so uploads start small. Its defaults are several times that.
  - Options weighed:
    - **Serving originals as-is** needs no processor, but leaves format compatibility to chance, keeps metadata and serves WAVs at full size.
    - **Pure Rust:** `symphonia` decodes well and libopus has Rust bindings, but WebM muxing, resampling and the long tail of input formats are what ffmpeg already does.
    - **MediaConvert** charges per minute of output and is far more than this needs.

  Record the choice in [`decisions.md`](decisions.md).
- **Serving.** `media-urls` gains `audio_url(key)`. A `PublishedAudioResponse { audioId, url, durationMs, peaks, caption }` is returned as `PublishedAnswerResponse.audio[]` and `CommentResponse.audio`. Both are new fields, so this is an expand-only change.
- **CSP.** Add `media-src 'self' blob: {mediaBaseUrl}`. Today `<audio>` falls back to `default-src 'self'`, which would block both S3 clips and `blob:` previews ([`frontend.md` §12.1](design/frontend.md#121-content-security-policy)). The Playwright CSP test asserts the new directive.
- **Service worker.** A `media-audio` cache, keyed by path like the image caches ([`frontend.md` §5.2](design/frontend.md#52-service-worker--caching-strategy)), with a 30-day expiry.
  - Safari always requests audio with a `Range` header, so the route needs Workbox's `RangeRequestsPlugin`, and it may store only full `200` responses, never a `206`.
  - Playback streams from the network. After the first play, the client fetches the whole object once to fill the cache.
  - `<audio crossorigin="anonymous">` makes the responses non-opaque. Bucket CORS adds `Range` to the allowed headers, and `Content-Range`, `Accept-Ranges` and `Content-Length` to the exposed ones.
- **Deletion and archival.** The `media-delete` helper ([`media.md` §8.0](design/media.md#80-explicit-deletion)) learns `AudioMedia`, with an `AUDIO_IN_USE` 409 that mirrors `IMAGE_IN_USE`. Soft-deleting a comment deletes its clip, best effort, as it does the comment's image. Archival ([`archival.md`](design/archival.md)) treats `AUD#` rows the same as `IMG#` rows.
- **Cost.** At 24 kbps, a clip is about 180 KB a minute, so a 2-minute clip is about 360 KB. A hundred clips a month adds under 40 MB of storage and a few hundred GB-seconds of Lambda, all inside the [D7](decisions.md#d7) budget.
- **Mock and demo** ([`frontend.md` §17](design/frontend.md#17-demo-mode)). `mockUploads` keeps the recorded `Blob` as a `blob:` URL and computes the peaks locally. The demo seed gains one freely licensed sample clip in `public/demo-media/`, with an entry in `CREDITS.txt`.

**Rollout.** Ship the playback renderer first and the recorder in a later release. An installed PWA on the old release shows an answer that's only a clip as an empty answer until it updates.

**No transcripts.** A member who wants their words as text can use their device's own speech-to-text in the editor. The caption covers the accessible label.

**Open questions.**
- Should the cap differ between answers (2 minutes) and comments (perhaps 1 minute)?
- iOS: test the mic permission in an installed (standalone) PWA on a real device. It has re-prompted every session in some iOS versions.

## 9. GIFs from GIPHY

A **GIF** item in the "+" attach menu (§10) of the response editor and the comment composer opens an embedded GIPHY search, much like Slack's. The chosen GIF renders inline in the answer or comment.

### 9.1 Picker UX

- **Layout.** On desktop the picker is a popover anchored to the button, about 360×420 px. On mobile it's a bottom sheet covering about 70% of the screen.
```
 ┌──────────────────────────────────┐
 │ [ tent fail               ] (🔍) │
 │ lol · yes · wow · thank you      │
 │ ┌────┐ ┌────┐ ┌────┐             │
 │ │    │ │    │ │    │  …          │
 │ └────┘ └────┘ └────┘             │
 │          [ More GIFs ]           │
 │               Powered by GIPHY   │
 └──────────────────────────────────┘
```

- **Contents.**
  - A search field ("Search GIPHY") with focus, and a **search button** (a magnifying-glass icon, `aria-label="Search GIPHY"`) beside it.
  - Opening the picker shows no Trending grid, because opening it shouldn't cost an API call (§9.3). Instead a row of suggestion chips ("lol", "yes", "wow", "thank you", "celebrate") is hard-coded in the app.
  - The grid has 2 columns on mobile and 3 on desktop. It uses GIPHY's `fixed_width` MP4 rendition, muted and looping. Under `prefers-reduced-motion` it shows the `_still` rendition instead.
  - Each search asks for 50 results, GIPHY's maximum, so most picks need no second page. A **More GIFs** button under the grid fetches the next 50. Scrolling to the bottom doesn't fetch anything by itself.
  - A **"Powered by GIPHY"** mark sits at the bottom of the picker. GIPHY's terms require it.
- **A search is sent only when the member asks for it.** That means pressing the search button, pressing Enter in the field, or tapping a suggestion chip. Typing never sends a request: there's no search-as-you-type and no debounce.
  - The button is disabled while the field is blank.
  - A query already searched while the picker is open is shown again from memory, without a request.
- **States.**
  - Before a search: the chips and "Search for a GIF".
  - Searching: a spinner in place of the grid, and the button disabled until the request returns.
  - No results: "No GIFs for 'tent fail'".
  - Offline: "You're offline. GIF search needs a connection."
  - Any other error: "Couldn't reach GIPHY" with a Retry button.
  - The rate limit: §9.4.
- **Picking.** Tap a GIF and the sheet closes.
  - In an answer, the GIF goes in at the cursor.
  - In a comment, it shows as a preview chip with ✕ above the send button.
  - Optionally, as in Slack, typing `/gif tent fail` in the comment box opens the picker with "tent fail" filled in. The search is still sent only when the member presses the search button or Enter.
- **Keyboard.** Arrow keys move through the grid, Enter picks and Esc closes. Each tile's `alt` is GIPHY's title.
- **Display.** A `GiphyEmbed` component renders `<video autoplay loop muted playsinline>`, which is smaller than a `.gif`. Its aspect ratio is reserved from the returned dimensions so nothing shifts.
  - In answers it's capped at the content width. In comments it's the 200 px `fixed_width`.
  - Under reduced motion it shows the still frame with a play button. A tap pauses or plays it.
  - A small "GIPHY" badge sits in the corner.

### 9.2 Storage: a markdown token, no backend change

- **The token.** A pick is stored in the body as `![title](giphy:{gifId})`, mirroring `image:{id}` ([`media.md` §7](design/media.md#7-image-references-in-markdown)). Answers allow up to 5 GIFs and comments 1. The editor enforces the cap, and the renderer draws no more than it.
- **No backend change.** The token is ordinary body text, so no route or row changes:
  - The comment "body or image" rule is met, because the token is non-whitespace text.
  - So is the auto-publish "non-empty" rule.
  - The ~40-character token counts toward the body limits.
- **Renderer.**
  - `urlTransform` and the `rehype-sanitize` schema allow the `giphy:` scheme ([`frontend.md` §9](design/frontend.md#9-renderers), [`engagement.md` §5](design/engagement.md#5-sanitization)).
  - Comments keep `allowImages={false}` for everything except `giphy:`.
  - A `gifId` must match `^[A-Za-z0-9]+$` before it's looked up. Anything else renders nothing.
- **Resolving at render.** The page collects every GIF ID it shows, in answers and comments, and makes one batched `GET https://api.giphy.com/v1/gifs?ids=…` (GIPHY's Get GIFs by ID). The returned media URLs are used unmodified.
  - It's one call per page, however many GIFs the page has. A page with no GIFs makes none.
  - The result is kept only in the React Query memory cache, with `staleTime: Infinity`, so going back to a page in the same session costs nothing. It isn't persisted, and the service worker doesn't cache GIF media, because GIPHY's terms forbid it (§9.3).
  - A new comment's GIF is merged into the cached result from the search result it was picked from, so posting one doesn't trigger another lookup.
  - An ID GIPHY no longer returns renders as "GIF unavailable". A lookup that hits the rate limit is covered in §9.4.
- **Rating.** Search sends `rating=pg-13`. A token typed by hand can name any GIF, so the renderer also hides a resolved GIF whose `rating` is above `pg-13`.
- **Old clients.** An older release doesn't know `giphy:` and renders the token's title as a link. Ship the renderer first and the picker in a later release.

### 9.3 Frontend only, on a beta key

Everything GIPHY-related happens in the browser. The backend never calls GIPHY, and no Lambda, route or S3 object is involved. GIPHY's API terms require this anyway: every API call and media load must come straight from the client. They forbid:
- proxying API calls or media through a server
- caching API responses or media URLs, or storing copies of GIPHY's media
- modifying the returned URLs
- reordering or filtering search results, or mixing them with another provider's

So this can't follow the place-search pattern (§6, a Lambda proxy) or the image pattern (re-hosting in S3). The consequences:

- **The idle-backend principle holds trivially.** Nothing runs on our side, and GIPHY doesn't charge for beta keys.
- **The API key ships in the bundle** as `VITE_GIPHY_API_KEY`, one per environment. GIPHY keys are designed to be public, so the risk is someone using up the quota, not a breach.
- **A free beta key is enough; no production key.** A beta key allows 100 calls an hour, counted per key and shared by everyone using the app. GIPHY's docs say that "for most applications a rate limited beta key is sufficient."
  - **Calls are made only by explicit actions** (§9.1, §9.2):
    - opening the picker costs nothing
    - typing costs nothing
    - each press of the search button, Enter, a chip or **More GIFs** costs one call, and a query repeated while the picker is open is free
    - a page view costs at most one lookup, and none on return visits in the same session
  - **Budget.** At about two searches per GIF picked, 100 calls an hour is enough for a handful of small groups. The tightest hour is just after an edition publishes, when push brings every member in at once.
  - **Hitting the limit loses nothing.** The token is in the body, so a GIF that can't load now loads later, and §9.4 tells members what's happening. Watch usage on GIPHY's developer dashboard.
  - **If 429s ever become routine,** the first lever is a second beta key used only for the display lookups. GIPHY's docs advise a separate key per section of an app, so searching and reading would stop competing for one quota. Applying for a production key would come after that.
- **No other provider.** Tenor isn't a fallback: Google shut its API down on 2026-06-30.
- **Privacy is the real trade-off.** Every reader's browser contacts GIPHY when a GIF renders, and searches go to GIPHY too. It needs an owner decision before it's built (§9.5).
- **No analytics pings.** GIPHY's responses carry `onload`/`onclick`/`onsent` URLs, but its [Action Register docs](https://developers.giphy.com/docs/api/endpoint/action-register/) present them as optional, there to "improve your users' search results". The client doesn't fire them and doesn't fetch a `random_id`, because they'd link a member's device to every device that views the GIF they posted (§9.5). Revisit only if a production-key application ever requires them.
- **Offline**, GIFs don't render, because the service worker isn't allowed to cache them. They show their title in a placeholder.
- **CSP** ([`frontend.md` §12.1](design/frontend.md#121-content-security-policy)):
  - `connect-src` gains `https://api.giphy.com` and GIPHY's analytics host.
  - `img-src` and `media-src` gain `https://*.giphy.com`, because media comes from several `mediaN.giphy.com` hosts. The wildcard is scoped to GIPHY's domain.
- **Mock and demo.** A mock GIPHY client returns a few local GIFs from `public/demo-media/`. The demo must make no GIPHY calls, and the Playwright `demo` project's off-site-request check already enforces that ([`frontend.md` §17](design/frontend.md#17-demo-mode)).

### 9.4 When the rate limit is hit

A `429` from GIPHY means the app's shared hourly allowance is used up. Members should see that plainly: it isn't their fault, it isn't a bug, and nothing they wrote is lost.

- **One shared cooldown.** A small in-memory store (`giphyLimit`) records when a `429` happened, whether from a search or a display lookup. Every GIPHY call checks it first, so while it's set nothing is sent.
  - It lasts 10 minutes, with a countdown. GIPHY doesn't document when its hourly window resets.
  - A `429` after it ends starts it again.
  - It isn't persisted. A reload clears it, and costs at most one more rejected request.
- **In the picker.** A banner replaces the "Search for a GIF" prompt, or sits above the results already shown:

  > **GIF limit reached.** GIPHY lets this app make a limited number of requests each hour, shared by everyone using it. Try again in 9 minutes.

  - The search button, chips and **More GIFs** stay disabled while the countdown runs, and the button's tooltip repeats the time left.
  - GIFs from an earlier search stay on screen and can still be picked, because picking one doesn't call GIPHY.
  - When the countdown ends, the banner goes and the controls come back. There's no automatic retry, because the member hasn't asked for a search.
- **In the "+" menu (§10).** The GIF item stays enabled, so earlier results can still be opened. Its subtitle changes from "Search GIPHY" to "GIF limit reached · try again in 9 min".
- **In posts.** A GIF whose display lookup was rate-limited shows a placeholder card in place of the GIF:
  - it's 4:3, because the real size isn't known without the lookup
  - it shows the GIF's title from the token, and the line "GIF limit reached — it'll load later."
  - a **Load GIF** button is disabled during the countdown, then retries the page's whole batched lookup in one call
- **Distinct from other failures.** Offline and other GIPHY errors keep their own copy (§9.1). Only a `429` mentions the limit.
- **Tests:**
  - a `429` from search shows the banner and disables the controls
  - a `429` from either side blocks the other
  - placeholders render with the title
  - the countdown re-enables the controls without sending a request

### 9.5 Privacy

GIFs would be the first feature where something a member posts makes other members' browsers contact a third party. Today the only third party is Google Fonts, which learns that someone opened the app but nothing about what they read ([`frontend.md` §12.1](design/frontend.md#121-content-security-policy)). [`frontend.md` §8.4](design/frontend.md#84-image-uploads-in-editor) renders a remote `![](https://…)` as a link precisely so that an answer can't make readers' browsers call a host the author chose. A GIF does exactly that, through one fixed host. That's why GIFs need an owner decision, recorded in [`decisions.md`](decisions.md), before they're built.

**What GIPHY receives.**
- **From a member searching:** the search text, their IP address, user agent and the time, and the app's API key and origin. Every API call is a CORS request, so the browser always sends `Origin: https://betterloop.site`.
- **From every reader of a page with a GIF**, whether or not they've ever used the picker: the IDs of every GIF on that page (the batched lookup, §9.2), their IP, user agent and the time, plus one media request per GIF to GIPHY's media hosts.
- **Never:** names, email addresses, the group's name, question or answer text, comments, or anything else from the body. A token carries only the GIF's ID and GIPHY's own title.

**What GIPHY could work out.** Each request says little; together they say more.
- **Who knows whom.** The same few GIF IDs get looked up from the same handful of IP addresses, mostly in the hours after an edition publishes. That's enough to cluster a group's members by IP, especially when the GIF is an obscure one, with no account or cookie involved. The analytics pings with a `random_id` would make this exact rather than inferred ("this device sent GIF X; these devices viewed it"), which is why §9.3 leaves them out.
- **Rough location and habits.** An IP address gives roughly the city, and request times show when members read.
- **What a member searched for.** A search for "funeral" or "chemo" says something about the person typing it, tied to their IP address.

**Consent is uneven.** The author chooses GIPHY; the readers don't. A member who wants nothing to do with GIPHY still sends it requests just by reading the edition. That is the heart of the decision.

**Legal exposure.** If a group has members in the EU, loading third-party content without consent is a known GDPR risk. In 2022 a Munich court (LG München I, 3 O 17493/20) awarded damages against a website that loaded Google Fonts from Google's servers, because each visitor's IP address went to Google without consent. A private app for a few friends may be judged differently, but the reasoning carries over, and GIF lookups reveal more than font requests do. The same ruling bears on the existing Google Fonts gap; self-hosting the fonts fixes that one.

**Mitigations, whatever is decided.**
- **No `random_id` and no analytics pings** (§9.3). GIPHY still sees IP addresses, but only timing links a member's device to the devices that view what they posted.
- **No referrer.** Every GIPHY fetch sets `referrerPolicy: "no-referrer"` and every media element `referrerpolicy="no-referrer"`. The browser default (`strict-origin-when-cross-origin`) already strips the path, which contains the group ID (`/g/{groupId}/…`), but setting it explicitly doesn't rely on the default. `Origin` is still sent on API calls; that's unavoidable, and it names only the app.
- **No cookies.** API fetches use `credentials: "omit"`. Media elements use `crossorigin="anonymous"`, so GIPHY's media hosts get none of the cookies they may have set elsewhere (Chrome still sends third-party cookies). This only works if the media hosts answer with CORS headers, because otherwise the element fails to load, so check that before relying on it.
- **Plain labelling.** The "Search GIPHY" placeholder and the "Powered by GIPHY" mark tell a member where their search text goes.

**The options.**
1. **On for every group.** The simplest. Every reader in every group contacts GIPHY once anyone posts a GIF.
2. **A per-group setting, on by default.** `Group.gifs_enabled`, with a missing value read as on. Admins who care can switch it off; most never look.
3. **A per-group setting, off by default.** A missing value reads as off, so existing groups need no backfill. An admin turns it on in the group settings form, which already sends changed fields on `PATCH /groups/{groupId}` ([`frontend.md` §7.7](design/frontend.md#77-groupadminpage)). The field is new, so the change is expand-only. The toggle carries one sentence of disclosure: "Members can add GIFs from GIPHY. Everyone who reads a GIF loads it from GIPHY, which sees their IP address."
4. **Tap to load, for readers, on top of 2 or 3.** A per-device "Load GIFs automatically" switch, kept in `localStorage` like a mail client's remote-images setting. When it's off, a GIF shows as the §9.4 placeholder card (its title and a **Load GIF** button), and nothing goes to GIPHY until the reader taps. It also saves display lookups against the hourly limit. The cost is friction, and the switch has to be found on each device.

With 2 or 3, the flag rides on the group's memberships in `/config`, which the settings form already invalidates on save. When it's off:
- the GIF item disappears from the "+" menu (§10)
- existing tokens render as their title text with no lookup, so switching it off stops all of the group's GIPHY traffic at once, including traffic from old posts

**Recommendation: option 3**, with the mitigations above. It matches how the app already treats remote content (§8.4). It puts the choice with the person who already makes the group's other settings, and every group's GIPHY traffic becomes a deliberate choice with the disclosure in front of whoever makes it. Option 4 is frontend-only, so it can be added later if a member asks.

**Open questions.**
- The privacy decision (§9.5): which option, recorded in [`decisions.md`](decisions.md).
- Is `pg-13` the right rating, or should it be a group setting?

New glossary terms when §8–§9 are built: *Voice clip*, *GIF* ([`overview.md` §5](overview.md#5-glossary-use-these-terms-exactly)).

## 10. The "+" attach menu

The dashed "Drag photos here, or choose files" box (`ImageDropzone`, which `ImageUploader` renders in both the response editor and the comment composer) becomes a single **+** button. It opens a menu of every kind of attachment, so voice clips (§8) and GIFs (§9) are added to that menu instead of each getting its own button.

### 10.1 The button

- A round **+** button at the start of the composer toolbar, under the textarea. It's 40 px across with a 48 px tap target. While the menu is open it turns into **×**.
- In the response editor it reads **+ Add** on screens wide enough for the label. In the comment composer it's just the icon. Both have `aria-label="Add a photo, voice clip or GIF"` and the same text as a tooltip.
- The box's counts and format copy ("3/10 images · JPEG, PNG, WebP or GIF · up to 15 MB each") move into the menu (§10.2). File errors show under the toolbar, where the box showed them.

### 10.2 The menu

```
 ┌───────────────────────────────┐
 │ 📷  Photo                     │
 │     3/10 · up to 15 MB each   │
 │ 🎙  Record a voice clip       │
 │     Up to 2 minutes           │
 │ 🎵  Upload audio              │
 │ GIF GIF                       │
 │     Search GIPHY              │
 └───────────────────────────────┘
          ( × )
```

- **Layout.** On desktop it's a menu that opens upward from the button. On mobile it's an action sheet that slides up from the bottom, with large rows and a Cancel row.
- **Built on `useDismissableMenu`** (`components/ui/useDismissableMenu.ts`), which the account menu and `GroupSwitcher` already use. It's a `role="menu"` with `role="menuitem"` rows. Opening it moves focus to the first item, the arrow keys move between items, Enter picks, and Escape or an outside click closes it and returns focus to **+**.
- **What each item does.**
  - **Photo** opens the existing image file input (`multiple`, the same `accept` list). On a phone the OS's own sheet offers the camera or the photo library, so no separate camera item is needed.
  - **Record a voice clip** starts the recorder (§8.1).
  - **Upload audio** opens the audio file input (§8.1).
  - **GIF** opens the GIPHY picker (§9.1), in place of the menu.
- **Items only appear when they can work.** Record is hidden when the browser can't record. GIF is hidden when the group has GIFs switched off (§9.3). If this release builds only some of §8–§9, the menu lists only what's built.
- **Items at their limit stay visible but disabled, with the reason as the subtitle:** "10/10 photos" on an answer, or "Already attached" on a comment, which takes one photo, one clip and one GIF.

### 10.3 Dropping and pasting

Dropping files onto the box goes away with the box, so the drop target becomes the whole composer:
- Dragging files over the textarea and toolbar shows a "Drop to attach" overlay.
- Dropped files are routed by MIME type: images to the image uploader, audio to the clip uploader (§8.3). Anything else gets the existing unsupported-file error.
- Pasting an image from the clipboard into the textarea attaches it the same way. That's new and cheap once routing by MIME type exists.

### 10.4 Where attachments show

Everything attached shows in one tray under the toolbar, in the order it was added: image tiles (`ImageUploaderTile`, as today), clip cards (§8.1) and the GIF chip in a comment (§9.1). GIFs in an answer are inline tokens in the body, so they show in the Preview tab, not in the tray. Removing anything works as it does today, including the permanent-delete confirm for uploaded media ([`frontend.md` §7.3](design/frontend.md#73-respondpage-ggncrespondq)).

### 10.5 Components and rollout

- A new `AttachMenu` component owns the button and the menu. It holds no upload state. Each item calls back into its owner: `ImageUploader` exposes an `openPicker()` and accepts dropped files from the composer's drop target, and the clip and GIF pieces do the same.
- `ImageDropzone` shrinks to the hidden `<input>` plus the drop overlay. The visible box goes.
- **This can ship first, on its own, with Photo as the only item.** It's a frontend-only change with no API or contract impact. §8 and §9 then each add a menu item without touching the layout again.
- **The risk is discoverability.** A big dashed box tells a first-time member they can add photos, and a **+** says less. That's the reason for the **+ Add** label on wide screens. If people stop attaching photos after the change, the fallback is to show the box once on an empty answer and switch to the **+** after the first attachment.
- **Tests to update.** `ImageUploader.test.tsx`, `RespondPage.test.tsx` and `e2e/demo/demo.spec.ts` find the bare `input[type=file]`. That still works while Photo is the only item. Once §8 adds an audio input, the selector matches two inputs, so give each input a label and select by it. Add tests for the menu itself: which items show, disabled items at their limit, and keyboard navigation.

## 11. Year in review

At the end of the year, each group gets a deck of summary cards in the spirit of Spotify Wrapped: the most-reacted answer of the year, the busiest comment thread, the year's leaders and a few lighter awards. Members swipe through it like a story. Each member also gets a few personal cards about their own year. Don't call it "Wrapped" in the UI, because that's Spotify's name.

The rule from the leaderboard (§5) holds here too: cards celebrate the top few and never rank anyone last.

### 11.1 The cards

**Group cards**, everyone sees the same ones:
- **The year in numbers:** editions published, answers, words written, photos shared, comments and reactions.
- **Answer of the year:** the answer with the most reactions, with an excerpt and a link to its edition. A runner-up or two sit underneath it.
- **The thread of the year:** the answer with the most comments (counting non-deleted ones), and how many members joined in.
- **Question of the year:** the locked question whose answers drew the most reactions and comments in total, crediting whoever suggested it unless it was anonymous.
- **The year's leaders:** year-scoped versions of the §5 boards: most reactions received, most editions answered and longest streak within the year.
- **Early bird:** the member with the shortest median time from `response_open_at` to their first manual publish. It uses the median, so one late month doesn't sink anyone, and only counts members who answered at least half the year's editions. Auto-published drafts are left out, because their `published_at` is `response_close_at` and says nothing about the member.
- **Question machine:** the member who suggested the most candidate questions, counting only ones that weren't anonymous, so the card can't unmask anyone. Unpromoted candidates still count, because promotion copies the winners into locked questions and leaves every candidate row where it was ([`lifecycle.md` §5.2](design/lifecycle.md#52-promote_and_opennl)). A second line names whose questions were promoted most often.
- **Lighter awards**, each shown only if someone qualifies:
  - *Perfect attendance:* everyone who answered every edition.
  - *The novelist:* the most words written.
  - *The photographer:* the most photos attached to answers.
  - *Biggest fan:* the most reactions given.
  - *Chatterbox:* the most comments written.
  - *Best buds:* the pair who comment on each other's answers most.
  - *The group's emoji:* the emoji used most in reactions.
  - *Closest poll:* the poll with the narrowest margin. Polls are anonymous ([`engagement.md` §3.5](design/engagement.md#35-anonymity)), so it uses tallies only and names no one.
  - Once §6 is built, *Places of the year:* a map of the year's check-ins.

**Personal cards**, only the member themselves sees them: how many editions they answered, their most-reacted answer, the emoji they got most, who reacted to them most, and any award they won. Personal cards don't compare the member to anyone else. "You answered 11 of 12 editions" is fine; "You were 9th" isn't.

**Who's eligible.** Awards go only to current members at the time the recap is generated. Group totals count everyone, including members who have left. Ties share a card.

### 11.2 Computing it once, as a snapshot

The recap is computed once by a batch job and stored, not counted at write time as §5 does. It runs once a year over at most 12 editions, so reading every edition's partitions in one pass costs little, and it needs no new counters or transactions. It doesn't depend on §5 being built.

- **When.** A group's year is the cycles whose `cycle_id` starts with that year. The December edition publishes about a week into December ([`lifecycle.md` §4.2](design/lifecycle.md#42-default-schedule-per-group)), so `publish(nl)` for a `{yyyy}12` cycle asynchronously invokes `lambda-recap` after the status flip ([`lifecycle.md` §5.3](design/lifecycle.md#53-publishnl)). That way a slow recap never holds up the tick. The recap is skipped for a group with fewer than `RECAP_MIN_EDITIONS` published editions in the year (new constant in `config.rs`, perhaps 3), so a group that started in October waits until next year.
- **The job** reads, for each of the year's editions, its locked questions, answers, comments and reactions (the same partitions `GET …/newsletters/{cycleId}` reads, [`api.md` §5.2](design/api.md#52-get-groupsgroupidnewsletterscycleid)), plus the year's candidate pools, then computes every card in memory.
- **A `YearRecap` row:** `pk=GROUP#{g}`, `sk=RECAP#{yyyy}`, holding `cards` (the group cards), `personal` (a map from `user_id` to that member's cards), `generated_at` and `edition_count`. A 50-member group's recap is a few tens of KB, well under DynamoDB's 400 KB item limit. The Put is conditioned on `attribute_not_exists(sk)`, so a retried invocation does nothing. An admin-only script can regenerate it.
- **It's frozen.** Reactions added in January don't change the recap. Featured answers store their IDs and a short excerpt (about 200 characters) at generation time, so a card still reads correctly after archival ([`archival.md`](design/archival.md)) prunes the answer itself. The excerpt is taken from text the whole group could already read.

### 11.3 API, notification and frontend

- **API.** `GET /groups/{groupId}/recaps/{year}` in `lambda-groups`, open to any member. It returns the group cards plus the caller's own personal cards, never anyone else's. `GET /groups/{groupId}/recaps` lists the years available. Excerpts come back with presigned URLs for any featured images and avatars, as published answers do. New routes, so the change is expand-only.
- **Notification.** A new push kind, `recap_ready` ("Your group's 2026 in review is here 🎉"), with its own `NotificationPref` toggle ([`notifications.md` §4](design/notifications.md#4-notification-preferences)).
- **Frontend.** `/g/:groupId/recap/:year`, a full-screen story view:
  - Tap or swipe to move between cards, with a progress bar of segments along the top. Personal cards come last.
  - Each card is a big number or a featured answer on a bold colored background, with a short animated entrance. Under `prefers-reduced-motion` the cards just change.
  - Featured answers link to their edition.
  - A banner on the group's newsletter list links to it for the rest of December and January, and it stays reachable after that from the group menu.
- **Sharing stays inside the group.** Each card has a **Share** button that copies a link to that card (`/g/:groupId/recap/:year#card-{n}`). Like every group page, the link opens only for a signed-in member of the group, and anyone else gets the usual not-a-member screen. There's no save-as-image, no public link and no OS share sheet carrying card content, so a card never leaves the app. A shared link to a personal card opens the viewer's own personal cards, never the sharer's.
- **Mock and demo.** The demo seed gains a `YearRecap` fixture, so the story view works in `/demo` ([`frontend.md` §17](design/frontend.md#17-demo-mode)).

**Open questions.**
- Should an admin be able to switch the recap off for the group?
- Should the recap wait for the quiz (§12), so that it can include a "Quiz champion" card?

## 12. Group quiz

Once a year (or at an interval the group picks), the app writes a quiz from what members posted, and the group competes. The questions are about each other: "Where did Sam go for their birthday?", "What new hobby did Priya pick up?", "At which competition did Jo win a medal?" Members take the quiz during an open window. When it closes, the top three scores make the podium, and each question reveals the answer it came from.

A language model has to write the questions, because they come from free-text answers and comments. That makes this the first feature that sends group content to an AI model (§12.5).

**Nobody writes or sees the questions in advance, admins included.** An admin decides only two things: to generate a quiz, and when to make it available. Everyone, admins too, sees a question for the first time when they start the quiz.

### 12.1 How a quiz runs

1. **An admin generates it.** In the admin page they choose the period it covers (default: the last 12 published editions), how many questions to ask (default 15, capped by `QUIZ_MAX_QUESTIONS` in `config.rs`) and how long it stays open once it opens (default 7 days, the same as a response window). Quizzes never start on their own.
2. **Generation** runs in the background and takes a minute or two (§12.3). The admin gets a push when it's ready. The admin page then shows the quiz as **Ready** with its question count, but not the questions. If generation fails, the page shows why, and the admin can try again.
3. **The admin makes it available**, either with **Open now** or by picking a date and time for it to open. A scheduled quiz can be rescheduled, or opened early, until it opens.
4. **Open.** Members get a push ("The 2026 group quiz is open — how well do you know everyone?"). Each member takes it once: one question per screen, four options each, with answers saved locally until they press **Submit**. Once submitted, answers can't be changed. Nobody sees the correct answers or any scores while the quiz is open, so the answers can't be passed around.
5. **Close.** At `close_at` the tick grades every attempt and closes the quiz. Members get a push with the results.
6. **Results.** A podium shows 1st, 2nd and 3rd. Ties share a place. Each member also sees their own score ("12 / 15") and goes through each question: what they picked, the right answer, and a link to the answer it came from. As with §5, there's no full ranking and nobody is shown in last place.

**Scoring.**
**Keep it light.** The quiz is for fun, not a serious contest, so scoring is deliberately simple:
- **One point per correct answer.** Every question counts for everyone, including the ones about yourself.
- **Questions are spread evenly over subjects:** each member with enough material is the subject of about the same number of questions, so the quiz covers the whole group.
- **Ties share a place.** If two members both get 13, both are first.
- **A bad question is part of the fun.** There's no flagging or voiding. If the model asks something odd, the group can laugh about it in the results.
- **Admins play like everyone else** and can make the podium. They haven't seen the questions either.
- Anyone in the group can play, including members who joined recently or posted nothing.

### 12.2 What questions are made from

- **Text answers** published in the chosen period, plus image captions.
- **Comments** on those answers, leaving out soft-deleted ones. A comment's author is the subject of any question made from it. Comments are sent with the answer they reply to, so the model can tell a fact ("I ran my first marathon last week!") from a joke that only makes sense in the thread.
- **Not poll votes.** Polls promise aggregate-only tallies ([`engagement.md` §3.5](design/engagement.md#35-anonymity)), so a question can never say how one member voted. Questions about a poll's group-wide result ("What did most of the group pick for best pizza?") are allowed.
- **Every current member's posts are used.** There's no opt-out (§12.5).
- **Question styles**, mixed:
  - *About a member:* "Where did Sam go for their birthday?", with four places as options.
  - *Who did it:* "Who picked up pottery this year?", with four members as options.
  - Wrong options should be plausible, ideally drawn from other members' real answers to similar questions, so they can't be guessed.
- **Topics to avoid**, stated in the prompt: health, grief, relationships ending, money, work trouble, and anything the author framed as private or sad. No person reads the questions before they go out, so this instruction and the validation in §12.3 are the only filters.

### 12.3 Generation

- **Amazon Bedrock, running Claude.** It's AWS-native, so `lambda-quiz-gen` calls it with IAM credentials and there's no API key to manage. Group content stays inside the AWS account that already stores it, and Bedrock doesn't use prompts or outputs for training. CDK grants `bedrock:InvokeModel` on the one model or inference profile chosen, and the model's access has to be enabled in the account first. Record the provider and model in [`decisions.md`](decisions.md).
- **`lambda-quiz-gen`** is invoked asynchronously by the generate route. It:
  1. reads the period's locked questions, published text answers and their comments
  2. labels each member with a short alias (`M1`, `M2`…) in place of their name, and keeps the mapping in memory. For each answer the model sees the question prompt, the author's alias, the answer text and captions, and its comments with their authors' aliases, and nothing else: no email addresses, no user IDs
  3. makes a single Converse call with a JSON output schema. It asks for about 1.5 times the number of questions wanted, so validation has spares. For each question it asks for the prompt, four options, the correct option, the subject's alias, the source (an answer's index, or a comment's index within it) and a **verbatim quote** from that source supporting the correct option
  4. validates every question and drops any that fail (below)
  5. picks the final set, spreading it evenly across subjects, and stores it with `status = ready`

  A year for a 30-member group is about 100k words of answers, plus its comments, which fits in one call. A group with more material is trimmed oldest-first to a token budget (`QUIZ_INPUT_TOKEN_BUDGET`).
- **Validation is the guard against made-up facts**, and with no human review it's the only one. A question is kept only if:
  - its source exists and was written by the subject: a published answer in the period, or a non-deleted comment on one
  - the quote appears exactly in that answer's body or caption, or in the comment's body, after whitespace is normalized
  - its four options are distinct and the correct option is one of them
  - for a "who" question, every option is a current member

  If fewer questions than `QUIZ_MIN_QUESTIONS` survive, the job retries once, then marks the quiz `failed` with a reason the admin can see.
- **Cost.** One call of about 100–150k input tokens and a few thousand output tokens per quiz, once a year. Check Bedrock's on-demand pricing for the chosen model when it's picked; it should be well inside the [D7](decisions.md#d7) budget at one quiz per group per year, but confirm that before allowing quarterly quizzes.

### 12.4 Data, API and frontend

**Rows.**
- **`Quiz`:** `pk=GROUP#{g}`, `sk=QUIZ#{quiz_id}`. It holds:
  - `status`: `generating` | `ready` | `open` | `closed` | `failed`
  - the period's first and last `cycle_id`, `question_count`, `open_days`, `open_at` (set when the admin schedules it), `opened_at` and `close_at`
  - `created_by`, `opened_by`, `model_id` and `failure_reason`

  A scheduled quiz gets `gsi2pk=QUIZ_STATUS#ready` and `gsi2sk={open_at}#{g}#{quiz_id}`. An open one gets `gsi2pk=QUIZ_STATUS#open` and `gsi2sk={close_at}#{g}#{quiz_id}`. That way `lambda-cycle-tick` finds quizzes due to open or close the way it finds cycles ([`lifecycle.md` §5.1](design/lifecycle.md#51-algorithm)). Opening sets `close_at = opened_at + open_days`, conditioned on `status = ready`, so **Open now** and a scheduled opening can't both happen. The tick's existing queries use `gsi2pk=NL_STATUS#…`, so these rows don't show up in them.
- **`QuizQuestion`:** `pk=GROUP#{g}#QUIZ#{quiz_id}`, `sk=QQ#{question_id}`. It holds `display_order`, `style`, `prompt`, `options`, `correct_option`, `subject_user_id`, `source` (cycle, question and answer IDs, plus the comment ID when the question came from a comment) and `source_quote`.
- **`QuizAttempt`:** same partition, `sk=ATT#{user_id}`. It holds `choices`, `submitted_at`, and after close `correct` and `place`. The Put is conditioned on `attribute_not_exists(sk)`, so a member gets one attempt, and on the Quiz row's `status = open` and `close_at > now` through a ConditionCheck.
- **Closing** follows the same pattern as `publish(nl)`: grade each attempt with its own update, then flip the Quiz to `closed` conditioned on `status = open`. A crash partway through leaves it `open`, and the next tick redoes the grading. Places are computed after the flip, from the graded attempts.

**API** (a new `lambda-quiz` crate following the crate template in `CLAUDE.md`):
- Admin: `POST /groups/{groupId}/quizzes` to generate one (or retry a failed one), and `POST …/quizzes/{quizId}/open` with an optional `openAt`, which opens it now or schedules it. A scheduled quiz can be posted to again to reschedule it.
- Any member: `GET /groups/{groupId}/quizzes` to list them, `GET …/quizzes/{quizId}`, and `PUT …/quizzes/{quizId}/my-attempt` to submit.
- **No route returns a question before the quiz is open, for any caller, admins included.** Before it opens, `GET …/quizzes/{quizId}` returns only its status, period, question count and opening time. While it's open, it returns the questions and options but never `correct_option`, `subject_user_id` or `source_quote`. Nothing in the response tells a caller which questions are about them. Once it's closed, the response adds the answers, sources, the caller's own result and the podium. Tests assert both of those for an admin caller as well as a member.
- All routes are new, so the change is expand-only.

**Notifications.** New push kinds `quiz_ready` (to the admin who generated it), `quiz_open`, `quiz_closing` (24 hours before close, to members who haven't submitted) and `quiz_results`, with one `NotificationPref` toggle covering the member-facing three.

**Frontend.**
- `/g/:groupId/quiz/:quizId`: an intro card, one question per screen with large tap targets and a progress bar, and a summary to check before **Submit**. A half-finished attempt is kept in `localStorage` so leaving the page doesn't lose it.
- After close, the same route shows the podium with avatars, then the question-by-question review.
- The admin page's **Quiz** card has the generate form, then the quiz's status (generating, ready, scheduled, open, closed or failed), with **Open now**, **Schedule** and, after a failure, **Try again**. It never shows the questions.
- **Mock and demo.** The demo seed gains a closed quiz with results and an open one to take. The demo never calls Bedrock.

### 12.5 Privacy

Until now, members' answers and comments have only ever been read by the group, and AWS stores them. A quiz sends a year of them to a model. Bedrock keeps that inside AWS and doesn't train on it, which is a strong reason to choose it, but it's still a new use of what members wrote, and members wrote it for each other, not for a quiz generator.

- **Consent is up to the admin.** There's no group setting and no member opt-out. It's the admin's job to make sure the group is happy for their posts to be used before they generate a quiz. To make that easy, the generate form says what will be sent: "Questions are written by an AI model (Claude, on Amazon Bedrock) from everyone's published answers and comments in this period. Nothing is used to train the model." It also links to a short explanation the admin can pass on to the group.
- **Minimum data.** The model sees aliases, prompts, answer text, captions and comment text only. No images, names or emails are sent. Calls aren't logged with their payloads: CloudWatch logs record token counts and timing only.

Record the provider choice and the no-opt-out rule in [`decisions.md`](decisions.md).

**Open questions.**
- Should images be used later ("Whose photo is this?")? That would send images to the model.
- Should quarterly or half-yearly quizzes be allowed, or is once a year special enough?

New glossary terms when §11–§12 are built: *Year in review*, *Quiz* ([`overview.md` §5](overview.md#5-glossary-use-these-terms-exactly)).

## 13. Dark mode

A dark theme, switched from the account menu and saved on the member's profile, so it follows them to every device. Settings also offers **System**, which follows the OS or browser setting and switches live when that changes.

### 13.1 UX

- **Account menu.** A **Dark mode** switch sits between Settings and Sign out, as a `role="menuitemcheckbox"` with `aria-checked`. It shows the theme currently on screen. Flipping it saves an explicit `dark` or `light`, so a member following the system who flips it stops following the system. The menu stays open after a flip, so the member sees the change happen.
  - While the preference is `system`, a muted second line reads "Following system". Choosing **System** again is done in Settings, which keeps the menu to one control.
- **Settings → Appearance**, a new card above Notifications. It has a three-way segmented control: **Light**, **Dark**, **System**, as a `radiogroup`. A line under it says "System matches your device's light or dark setting."
- **The default is `system`.** A member who never touches it gets dark mode exactly when their device is dark.
- **The change applies at once**, before the save returns. If the save fails, the theme stays as picked on this device, and a toast says "Couldn't save your theme. It'll apply on this device only." The next load uses whatever the server holds.

### 13.2 Storage and API

- **`User.theme`**: `light` | `dark` | `system`. A missing value reads as `system`, so existing users need no backfill.
- **API.** `PATCH /me` accepts `theme`, validated against a new `ThemePreference` enum in `shared/openapi.yaml`, and `UserResponse` gains it ([`api.md` §2.3](design/api.md#23-patch-me)). Both are new fields, so this is an expand-only change.
- **Rollout order: backend first.** `PatchMeRequest` has `additionalProperties: false`, so the current backend answers a `theme` field with `VALIDATION_FAILED`. The backend that accepts it must be deployed before the frontend that sends it. An old frontend simply ignores the new `UserResponse` field.
- **A local copy for first paint.** `GET /me` arrives after the first render, so a member who chose dark would see a flash of light on every load. The client therefore keeps the last-known preference in `localStorage` (`theme`) and rewrites it whenever `useMe` returns or the member changes it. The server is the source of truth; the local copy is only a cache.
- **Before sign-in and before an invite is redeemed** (`/config` has `userId: null`, so there's no User row, [`frontend.md` §7.6](design/frontend.md#76-settingspage)), the local copy applies, or `system` when there isn't one. The join page and the auth callback look right that way.
- **Demo mode** ([`frontend.md` §17](design/frontend.md#17-demo-mode)): the mock transport's `PATCH /me` keeps the value in memory like the rest of the demo profile, and the local copy keeps it across reloads.

### 13.3 Applying the theme

- **Tailwind `darkMode: "class"`**, with `dark` on `<html>`. A small `state/theme.ts` store turns the preference into the effective theme. For `system` it listens to `matchMedia("(prefers-color-scheme: dark)")`'s `change` event, so the app follows the OS live.
- **The first-paint script.** A tiny `public/theme-init.js`, loaded with a plain blocking `<script src>` in `<head>`, reads the local copy and the media query and sets the class before anything renders. It has to be a file, because the CSP's `script-src 'self'` forbids inline scripts ([`frontend.md` §12.1](design/frontend.md#121-content-security-policy)). It also needs to be in the service worker's precache, so it loads offline.
- **`color-scheme`** is set on `<html>` to match, so native controls, scrollbars and autofill take the theme too.
- **Browser chrome.** `index.html` gets two `<meta name="theme-color">` tags, one for each `prefers-color-scheme` media query. When the member's choice differs from the system, `state/theme.ts` overrides the meta's `content`. The manifest's `theme_color` and `background_color` ([`frontend.md` §5.1](design/frontend.md#51-manifest)) stay light: the manifest takes only one value, and they only color the splash screen of an installed PWA.

### 13.4 The color work

This is most of the job. Today's palette is fixed hex values in `tailwind.config.ts`, with no CSS variables and no `dark:` styles anywhere in `src/`.

- **Move the tokens to CSS variables.** Each color in `theme.extend.colors` becomes `rgb(var(--ink) / <alpha-value>)`, so opacity modifiers such as `bg-ink/5` and `bg-mist/90` keep working. `:root` holds the light values and `.dark` the dark ones. Components then need no `dark:` classes for the shared tokens.
- **Dark values to choose:**
  - the neutrals (`cream`, `mist`, `surface`, `line`, `ink`, `inkmuted`): dark grey surfaces with a slight warm or violet tint, so it stays the same app rather than becoming generic grey
  - the pastels (`lavender`, `blush`, and the `phase.*` tints): deep, low-saturation versions, so status pills and tinted cards still read as the same phase
  - the accents (`coral`, `grape`, `mint`, `sky`, `sun`, `peach`): mostly unchanged, but each checked for WCAG AA contrast against the dark surface, with lighter variants where a color fails
- **Classes that bypass the tokens** need converting first: about 50 `bg-white`, which become `bg-surface`, plus a few `bg-white/60`, `ring-white` and one `from-slate-600`. `text-white` on accent buttons stays. `bg-ink` and `text-cream` used as an inverted pair, such as the primary button, need a deliberate dark-mode look rather than a straight swap.
- **Shadows.** The `soft`, `card` and `pop` shadows barely show on a dark background. In dark mode cards rely on a `line` border and a slightly lighter surface for depth.
- **Also covered:** the hand-written markdown prose ruleset in `styles/tailwind.css` ([`frontend.md` §4.3](design/frontend.md#43-typography)), the avatar fallback colors, `ConfirmDialog` and toast backdrops, and the `animate-pulse` loading skeletons.
- **Untouched:** uploaded photos and avatars aren't dimmed or filtered. Push notifications are drawn by the OS.

### 13.5 Tests and rollout

- **Unit tests:** the store resolves `system` from the media query and follows its `change` event. The account-menu switch and the Settings control both send `PATCH /me` and update `<html>`. The local copy is written on load and on change.
- **Playwright:** the demo project runs once with `colorScheme: "dark"` and once with an explicit `dark` preference. It checks there's no flash of light on reload, measured as the `dark` class being present at `DOMContentLoaded`. An axe contrast pass runs on the main pages in dark mode.
- **Rollout:**
  1. The backend field ships.
  2. The token refactor ships with light values only. It should change nothing visible, which makes it easy to review on its own.
  3. The dark palette, the first-paint script, the menu switch and the Settings card ship together.
- **Docs to update when it's built:**
  - [`frontend.md` §4.2](design/frontend.md#42-theming), which says the app is light-only with fixed hex tokens. Rewrite it for the CSS-variable tokens, the `dark` class and `state/theme.ts`, and drop its link to this section.
  - [`frontend.md` §7.6](design/frontend.md#76-settingspage), for the Appearance card.
  - [`frontend.md` §12.1](design/frontend.md#121-content-security-policy), for the first-paint script.
  - [`api.md` §2.3](design/api.md#23-patch-me), for the `theme` field.
  - [`data-model.md` §2.1](design/data-model.md#21-user), for `User.theme`.

**Open questions.**
- How should the hero gradients (`from-lavender … to-blush`) and the edition-phase colors look in the dark? This needs a design pass with real screenshots, not just token values.
- Should the accent colors move to slightly lighter variants in dark mode across the board, or only where a contrast check fails?
