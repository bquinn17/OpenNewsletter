# Media uploads and image pipeline

How response, comment and avatar images move from a user's browser to S3, get resized into display/thumb variants, and get served back as presigned S3 URLs. Read this before changing anything under `lambda-media`, `lambda-image-process`, `media-urls`, or the media-delete helper. The constants in §1 and `backend/crates/shared/src/config.rs` are the source of truth for limits; the code in `backend/crates/lambda-image-process/src/process.rs` is the source of truth for the resize/encode algorithm.

Users can attach up to 10 images per text response. Uploads go directly from browser to S3 via pre-signed URLs (Lambda is never on the upload data path). A subsequent S3 trigger generates two derived variants (thumb + display), marks the image ready and deletes the original. Reads go straight to a private S3 bucket: every API response that carries an image hands out short-lived presigned GET URLs, and the service worker caches the bytes by path (§6.1 says why).

---

## 1. Constraints (single source of truth)

- Max bytes per upload: **15 MB** (15728640).
- Allowed MIME types: `image/jpeg`, `image/png`, `image/webp`, `image/gif`.
- Max images per response: **10**.
- Display variant: **WebP, 1200px on the long edge, quality 82**.
- Thumb variant: **WebP, 400px on the long edge, quality 75**.
- GIF is preserved as GIF for both variants (resized) so animations continue to work.

These constants live in `backend/crates/shared/src/config.rs` so all Lambdas agree.

---

## 2. S3 buckets and key layout

Two buckets for response and comment images, both private (defined in [`infrastructure.md` §5.1](infrastructure.md#51-s3-buckets-mediapersistentstack)). Avatars add a third, their own originals bucket (§11.1); their processed variant shares the processed bucket below.

### 2.1 `opennewsletter-media-originals-{env}-{accountId}`

Originals as uploaded. An original lives only until processing finishes: `lambda-image-process` deletes it once the row is `ready` or terminally `failed` (§5.2), and a lifecycle rule expires anything left under `uploads/` after 7 days (§8.1).

```
uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}
```

Example: `uploads/01HXG.../202605/01HX.../01HX.../01HXJ.....jpg`

### 2.2 `opennewsletter-media-processed-{env}-{accountId}`

Derived images, served by presigned GET (§4). The only bucket the browser reads from.

```
img/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}/display.webp
img/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}/thumb.webp
avatar/{avatarId}/display.webp
```

(GIFs use `.gif` extensions instead of `.webp` for both variants.)

Avatars live here too, under `avatar/` (§11.1). IAM grants are scoped by these two prefixes, so a Lambda that only shows avatars can't sign image URLs.

---

## 3. Upload flow (browser → S3)

Sequence:

```
┌─────────┐                  ┌─────────────┐                 ┌─────┐                ┌──────────────────┐
│ Browser │                  │ lambda-media│                 │  S3 │                │ lambda-image-    │
│         │                  │             │                 │     │                │ process          │
└────┬────┘                  └──────┬──────┘                 └──┬──┘                └────────┬─────────┘
     │                              │                           │                            │
     │ 1. POST /uploads             │                           │                            │
     │ ────────────────────────────►│                           │                            │
     │                              │ 2. validate; create       │                            │
     │                              │    ImageMedia(status=pending)                          │
     │                              │ 3. presign PUT URL        │                            │
     │ ◄────────────────────────────│                           │                            │
     │                              │                           │                            │
     │ 4. PUT image binary directly │                           │                            │
     │ ─────────────────────────────────────────────────────────►                            │
     │                              │                           │ 5. ObjectCreated trigger   │
     │                              │                           │ ──────────────────────────►│
     │                              │                           │                            │
     │                              │                           │ 6. probe + resize +        │
     │                              │                           │    encode WebP (or GIF)    │
     │                              │                           │ 7. Put display + thumb     │
     │                              │                           │ ◄──────────────────────────│
     │                              │                           │                            │
     │                              │                           │ 8. Update ImageMedia       │
     │                              │                           │    status=ready            │
     │                              │                           │    + delete original       │
     │                              │                           │                            │
     │ 9. poll GET /uploads/{id}    │                           │                            │
     │    until status=ready        │                           │                            │
     │ ────────────────────────────►│                           │                            │
     │ ◄────────────────────────────│                           │                            │
```

### 3.1 `POST /uploads` validation

Request shape and the field-level validation rules (MIME allowlist, byte cap, membership, locked-question, cycle-status per `purpose`) are [`api.md` §9.1](api.md#91-post-uploads).

The 10-image cap (`purpose=response` only) is enforced via a main-table Query of the cycle's partition (`pk = GROUP#{g}#NL#{c}`, `begins_with(sk, "IMG#")`) filtered to the caller, question, `purpose=response` and `status != failed` — bounded, unlike a GSI1 query over every upload the user has ever made. Two simultaneous uploads can briefly exceed the cap; that's acceptable because the response save independently caps `imageMediaIds` at 10. Comment images are exempt from this cap; their limit is one image per comment, enforced at comment create/patch time.

### 3.2 Presign

Use the AWS Rust SDK's `Client::put_object().presigned(PresigningConfig::expires_in(600))`. Set:

- `Content-Length`: S3 **cannot** enforce a size cap on a presigned PUT (`s3:content-length-range` only exists for presigned **POST** policies, and we deliberately keep the simpler PUT flow). Enforcement is layered instead: (1) the client validates before requesting the presign; (2) `POST /uploads` validates the declared `byteSize`; (3) `lambda-image-process` checks the actual object size (`ContentLength` on the fetched object) before decoding — if it exceeds the 15 MB cap it marks the row `status=failed` (`errorMessage: "IMAGE_TOO_LARGE"`) and **deletes the original**. A dishonest client can land oversized bytes in the originals bucket briefly, but they are never processed or served, and are purged on sight.
- `Content-Type` matches the requested MIME type (signed).
- Server-side encryption is on by default (bucket-level), no header needed.

If the client sent `sha256` in the body (standard base64), sign it as `x-amz-checksum-sha256`; S3 then rejects a body whose digest doesn't match. The S3 client is built with `RequestChecksumCalculation::WhenRequired`, because newer SDKs otherwise sign a CRC32 of the (empty) presign body and every browser PUT fails. The response's `headers` map is every header the presign was signed with, and the client sends them all. Both originals buckets allow any request header in CORS (`*`); the presign is what grants access.

### 3.3 Frontend uploader (`ImageUploader.tsx`)

State per pending image:

```ts
type PendingImage = {
  imageId: string;
  file: File | null;     // null for an image the draft already had
  status: "uploading" | "processing" | "ready" | "failed";
  progress: number;       // 0-1
  altText: string;        // user-entered
  error: string | null;
};
```

A ready image's URLs and dimensions aren't in that state. They're its `GET /uploads/{id}` row, cached under `queryKeys.upload(groupId, cycleId, imageId)` (`api/queries.ts#uploadQuery`, flagged `MEDIA_URLS_META`). The poll in step 5 writes the `ready` row in with `setQueryData`, and `initialImages` seeds the rows the draft already had. The uploader's tiles, the editor's preview and the read-only answer on the respond page all read that one entry, so one URL refresh (§4.4) reaches all three. `useHydratedImages` loads the draft's rows into the same entries before the editor mounts.

Algorithm:
1. User drops/picks files. Reject any failing client-side validation (MIME, size).
2. For each file, compute SHA-256 with `crypto.subtle.digest` (native and asynchronous, so it doesn't block the main thread — simpler than a Web Worker). Omit it if `crypto.subtle` is unavailable.
3. POST `/uploads` → receive `{ imageId, uploadUrl, headers, expiresInSeconds }`. Later calls on the image pass `?groupId=&cycleId=` ([`api.md` §9.0](api.md#90-addressing-an-existing-upload)).
4. PUT to `uploadUrl` using `XMLHttpRequest` (for progress events). On 200 → `status=processing`.
5. Poll `GET /uploads/{imageId}` every 1.5s (with backoff if still pending after 10s).
6. When `status=ready`, replace the in-editor token with the final image. Insert markdown token `![alt](image:{imageId})` into the body.

If the page is closed mid-upload, the partial S3 object is cleaned up by the bucket lifecycle (abort incomplete multipart after 1 day).

---

## 4. Serving: presigned S3 GETs

The processed bucket is private, and the browser reads each object with an S3 presigned GET URL that the API minted for it ([§6.1](#61-why-presigned-s3-decision-record) says why). Those URLs aren't fetched from a separate endpoint; they're the values of the image fields in ordinary API responses:

- `displayUrl` / `thumbUrl` on `PublishedImageResponse`, `CommentImageResponse` and `ImageMediaResponse` (the last is `null` until the image is `ready`)
- `avatarUrl` on `UserResponse`, `MemberResponse`, `AskedBy` and `AvatarMediaResponse` (`null` when there's no avatar)

Every handler that returns one of these has already checked that the caller may see it: group membership for anything group-scoped ([`auth.md` §11](auth.md#11-tenant-authorization-helper)), ownership for the caller's own uploads and avatar. That check is the tenant boundary. A presigned URL is just its proof, scoped to one object and an hour.

### 4.1 Bucket access and CORS

- Block Public Access stays fully on. Nothing grants anonymous reads, and S3 doesn't bill for the 403s a request without a valid signature gets from outside the account.
- A presigned URL carries the signer's permissions, so each signing Lambda's role gets `s3:GetObject` on just the prefixes it serves: `img/*` for `lambda-media`, `lambda-newsletters` and `lambda-engagement`; `avatar/*` for `lambda-media`, `lambda-groups`, `lambda-newsletters` and `lambda-questions` ([`infrastructure.md` §6.2](infrastructure.md#62-lambda-functions-request-handlers)).
- Bucket CORS allows `GET` and `HEAD` from the SPA's origin (plus `http://localhost:5173` in dev). No credentials: the signature is in the query string. The SPA loads images in CORS mode so its service worker sees real statuses (§4.4).

### 4.2 URL lifetime and signing window

- **TTL:** `MEDIA_URL_TTL_SECONDS` (1 h, `backend/crates/shared/src/config.rs`). A leaked URL exposes one object for at most that long.
- **Signing window:** the presign start time (`X-Amz-Date`) is floored to `MEDIA_URL_SIGN_WINDOW_SECONDS` (5 min), and the expiry counts from that floored time. A refetch from the same Lambda container inside one window returns byte-identical URLs, so React doesn't swap `<img src>` and the browser doesn't re-request anything. A URL therefore lives between 55 and 60 minutes.
- **Early expiry:** URLs are signed with the Lambda's own role credentials, and S3 stops honouring a presigned URL when the credentials that signed it expire, whatever its `X-Amz-Expires` says. Lambda role sessions typically last 1–12 hours, so a URL signed late in a container's life can die early. The SPA's one-retry refresh (§4.4) covers this; it's the same path as an ordinary expired URL.
- Only `host` is signed (`X-Amz-SignedHeaders=host`), because a browser `<img>` sends nothing else. The `media-urls` S3 client sets request checksum calculation and response checksum validation to `WhenRequired` so the SDK doesn't sign an `x-amz-checksum-mode` header the browser would never send; a unit test pins this.

### 4.3 The `media-urls` crate

`backend/crates/media-urls` is the one place image URLs are built. `MediaUrls::from_env` reads `PROCESSED_BUCKET`; `image_url(key)` signs a processed image key and `avatar_url(avatar_id)` signs `avatar/{avatarId}/display.webp`. `ready_image_urls(image)` signs both variants of an image row, or returns `None` until it's `ready`, and `optional_avatar_url` takes a user's optional avatar ID. Both are `async`, call the SDK's `get_object().presigned(..)` (local computation, no network round trip) and map a failure to a 500 like `presign_put` does (§3.2). Each Lambda that returns image fields holds one `MediaUrls` in its `AppState`.

If image hosting ever changes (a different bucket, or something in front of it), this crate and `MediaPersistentStack` are the only seams that would change. The API fields, the markdown `image:{id}` indirection (§7) and the SW cache don't care where a URL points.

### 4.4 SPA usage

- `MediaImage` (`components/ui/MediaImage.tsx`) and `Avatar` render the URL as-is: `crossorigin="anonymous"`, `loading="lazy"`, `decoding="async"`, with `width`/`height` when the response has them so lazy loading doesn't shift the layout. The API reports the original's dimensions, so a thumb gets them scaled down by `utils/media.ts#thumbSize`. There's no auth step before rendering.
- **Refresh on expiry:** on the first load error for an image path, `MediaImage` awaits `utils/media.ts#refreshMediaUrls()`, which refetches the active queries flagged `MEDIA_URLS_META` (the ones whose data carries media URLs; one in flight, at most once per 10 s) so they come back with fresh URLs. It does nothing while `navigator.onLine` is false: offline, the error is the network's, and every refetch would fail with its own toast. It then remounts the `<img>` once. A second failure for the same path shows a placeholder the same size as the image; `Avatar` falls back to the initials instead. After that, a different URL for the same path (a later refetch re-signed it) gets one more load without another refresh, and the browser coming back `online` starts the policy over, because offline both failures were the network's. Both share this policy through `components/ui/useMediaRetry.ts`. Every media URL the SPA renders comes from a flagged query, the uploader's included (§3.3), so this one refresh path covers them all.
- **Caching:** the service worker serves the processed bucket's `/img/*` (cache `media-images`) and `/avatar/*` (cache `media-avatars`) `CacheFirst`, keyed by origin plus path with the query string dropped (`swHelpers.mediaCacheKey`), so a new signature for the same object is a cache hit. Each device downloads an image once per 30 days and an avatar once per 90 (§11.3), and published newsletters work offline ([`frontend.md` §5.2](frontend.md#52-service-worker--caching-strategy)).
- **CSP:** `img-src` lists the processed bucket's origin (`VITE_MEDIA_BASE_URL`); `connect-src` lists the two originals buckets' origins for the upload `PUT`s (`VITE_UPLOAD_ORIGINS`) ([`frontend.md` §12.1](frontend.md#121-content-security-policy)).

### 4.5 Local development and mocks

Dev and prod serve media the same way. `make fe` (Vite `--mode dev`) talks to the deployed dev stack and gets real presigned URLs: `scripts/write_frontend_env.py` writes `VITE_MEDIA_BASE_URL` and `VITE_UPLOAD_ORIGINS` from `MediaPersistentStack`'s outputs, and the dev buckets' CORS also allows `http://localhost:5173` (§4.1). Plain `npm run dev`, Vitest and the Playwright mock suite run against the in-memory mock transport (`frontend/src/api/mockTransport.ts`) and never touch S3: a ready image or avatar gets an inline data URL. `.env.e2e` sets placeholder `https://` media and upload origins so the built CSP has real origins to assert on ([`environments.md` §7](../development/environments.md#7-frontend-pointing-at-dev)).

---

## 5. `lambda-image-process` (S3 trigger)

### 5.1 Trigger configuration

S3 event: `s3:ObjectCreated:*` filtered to `prefix: uploads/`, on both the media-originals and avatars-originals buckets. S3 invokes the Lambda once per object (no batching configurable on direct S3→Lambda notifications).

Environment: `TABLE_NAME`, `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET` (to tell the two sources apart), `PROCESSED_BUCKET` (images under `img/`, avatars under `avatar/`).

Implementation notes:
- The ready/failed updates are conditioned on `status = pending`. A lost condition means another invocation already finished, so it is logged and skipped, not retried.
- Size and decode failures are terminal and recorded on the row (`error_message` = `IMAGE_TOO_LARGE` / `IMAGE_DECODE_FAILED`), and both delete the original, best effort as on the `ready` path below. Transient S3/DynamoDB errors return `Err` so Lambda's async retry runs.
- **An original that's already gone is skipped, not an error.** S3 notifications are at-least-once, so a duplicate delivery can see the row still `pending` and then find that the first invocation has since finished and deleted the original. The S3 `NotFound`/`NoSuchKey` on its read is logged at INFO and the invocation returns `Ok`.
- **The original is deleted once the row is `ready`.** Right after the conditional ready update succeeds, the handler deletes the original from its originals bucket. This is best effort: a failure is logged at ERROR and the invocation still returns `Ok`, because a retry would find the row no longer `pending` and skip it. The 7-day lifecycle expiry (§8.1) removes whatever a failed delete leaves. When the ready update loses its condition (another invocation finished, or the image was deleted mid-processing), the original is left to whoever won. Keeping no originals is an owner decision (2026-10-14): storage stays flat, and uploaded EXIF/GPS metadata doesn't outlive processing (the variants are re-encoded from pixels and carry none). The trade-off is that variants can't be regenerated at new sizes from the full-size upload.
- Variants are never upscaled. Recorded `width`/`height` are the original's after EXIF orientation.
- Cache-Control on writes: `private, max-age=5184000, immutable` (60 days, never more than two months) for `img/*`, `private, max-age=7776000, immutable` (90 days, §11.3) for `avatar/*`; the two lifetimes are `IMAGE_CACHE_MAX_AGE_SECONDS` and `AVATAR_CACHE_MAX_AGE_SECONDS` in `backend/crates/shared/src/config.rs`. `private` because these are per-viewer authenticated reads that no shared cache should keep. The browser's HTTP cache keys on the full presigned URL; the service worker's cache (§4.4) is what makes reloads free.

### 5.2 Algorithm (Rust)

Implementation: `backend/crates/lambda-image-process/src/handler.rs` wires S3 to the row (per record: parse the key into `groupId`/`cycleId`/`questionId`/`userId`/`imageId`, skip if the row isn't still `pending`, download, hand the bytes to `process.rs`, upload the two returned variants, mark the row `ready` with their keys and the oriented dimensions, then delete the original, §5.1). `process.rs` is the pure image logic (no AWS, unit-tested in `process/tests.rs`):

- Sniffs the format from the bytes, never trusts the extension.
- Before allocating pixels, enforces the decode budget (§9 table) — refuses a header wider/taller than `MAX_IMAGE_EDGE_PX` or a decode that would exceed `MAX_DECODED_IMAGE_BYTES`.
- Applies EXIF orientation during decode; recorded `width`/`height` are post-orientation.
- GIF is decoded and resized **frame-by-frame** (`GifEncoder`) and re-encoded as GIF, so animation survives; every other format becomes lossy WebP. GIF frames are additionally capped at `MAX_GIF_FRAMES` and `MAX_GIF_TOTAL_PIXELS`, checked per frame during the stream so only one full-canvas frame is ever decoded at once.
- Never upscales (§1): an image already at or under the target long edge keeps its size.

### 5.3 Image library

`image = "0.25"` for decoding, resizing and GIF. **Lossy WebP needs the `webp` crate (libwebp)**: `image::codecs::webp::WebPEncoder` is lossless-only and cannot honour the quality settings in §1.

If the encode fails for any reason, set `status = "failed"` with an error message field; the frontend will surface a "couldn't process this image" error and offer re-upload.

### 5.4 Permissions

The Lambda's role:
- `s3:GetObject` + `s3:DeleteObject` on both originals buckets (delete removes each original after processing, §5.1)
- `s3:PutObject` on the processed bucket's `img/*` and `avatar/*`, plus `s3:DeleteObject` there for variants written for a row deleted mid-processing (§8.0)
- `dynamodb:GetItem` + `dynamodb:UpdateItem` on the table

Memory: 1024 MB (image processing is CPU-bound; arm64 Lambda's 1024 MB gives a full vCPU). Timeout 60s. Largest expected image ~15MB; processing in well under 10s typical.

---

## 6. Cache and tenant isolation summary

- The processed bucket is private. The only way to read an object is a presigned URL, and the only way to get one is an API response from a handler that has already checked membership (or ownership) for it (§4).
- A presigned URL is scoped to one object for at most an hour (§4.2). A member of group A who guesses a group B image key can't sign it; the same URL without a valid signature gets S3's 403.
- There is no shared edge cache. Caching is per device: the service worker's `media-images` and `media-avatars` caches, keyed by object path (§4.4), plus the browser's HTTP cache for an unchanged URL.
- Removing a member stops new URLs within the membership-cache TTL ([`decisions.md`](../decisions.md#membership-cache)). URLs already handed out keep working until they expire, and copies already in that device's caches stay. Both are accepted.

### 6.1 Why presigned S3 (decision record)

**Decision (2026-10-14): private S3 plus presigned GET URLs signed by each Lambda's own role, minted inline in API responses.**

Options weighed:

- **Public-read bucket with unguessable keys.** Simplest, with perfect caching, but it drops membership-based isolation: a leaked URL works for anyone until the image is deleted. Rejected.
- **Presigned with a dedicated IAM user's long-lived access key.** URLs could last up to 7 days and be stable across Lambdas, but it adds a long-lived credential to store and rotate by hand. Rejected: the service worker cache already makes URL churn cheap.
- **A CDN in front of the bucket.** This AWS account can't create CloudFront resources, and at this scale an edge cache would add a signing key and a cookie or URL-signing flow for no visible speedup. Not pursued.
- **Presigned with the Lambda role (chosen).** No secrets and no new infrastructure, and it's the mechanism uploads already use (§3.2). Its cost is URL churn, absorbed by the SW cache key and the signing window, plus early expiry when role credentials expire first, absorbed by the one-retry refresh (§4.2, §4.4).

Because URLs arrive inline with the data, an image renders on the first paint with no separate auth request, a delete takes effect the moment the object is gone (§8.0), and dev and prod serve identically (§4.5).

S3 GETs are $0.0004 per 1,000, and egress is covered by AWS's 100 GB/month always-free data transfer out ([`infrastructure.md` §12](infrastructure.md#12-cost-expectations)). Without an edge cache, the first view of each image on each device is a round trip to us-east-1. The variants are small (WebP, ≤1200 px) and lazy-loaded, so that's acceptable.

---

## 7. Image references in markdown

Stored format: `![alt text](image:{imageId})`.

Server-side: nothing. The `body` is stored verbatim.

Render-side (`utils/markdown.tsx`):
- There's no remark plugin. `MarkdownBody` overrides react-markdown's `img` renderer: an `image:{id}` src is looked up in an `images` table the parent passes in (`PublishedAnswerResponse.images`, or the editor's live uploads) and rendered with `MediaImage`. An unknown ID renders nothing.
- `image:` must pass both `react-markdown`'s `urlTransform` and the `rehype-sanitize` schema. See [`frontend.md` §9](frontend.md#9-renderers).
- Any other image URL renders as a link, never an `<img>`.
- `ImageGallery` shows an answer's images that the body doesn't reference inline.

This indirection means URLs can change (re-signed every request window, or a different host entirely) without rewriting stored markdown.

---

## 8. Cleanup paths

### 8.0 Explicit deletion

`DELETE /uploads/{imageId}` (owner only; `IMAGE_IN_USE` guards per [`api.md` §9.4](api.md#94-delete-uploadsimageid)) deletes for real, via one shared helper: the `media-delete` library crate (`backend/crates/media-delete` — not `shared`, because `shared` can't depend on `persistence` and the helper deletes the row). Before the sequence, `DELETE /uploads` drops the image's ID from the caller's draft if the draft still lists it (conditional single-item write; [`api.md` §9.4](api.md#94-delete-uploadsimageid)), so a draft never keeps pointing at a deleted image. The sequence, run in this order, with each step idempotent so a retry after a failure redoes it safely:

1. Delete the original S3 object. It's usually gone already (§5.1); deleting a missing object succeeds.
2. Delete each processed variant (display, thumb) the row has a key for. A row with no processed key (still `pending`/`failed`) was never served, so there's no variant to delete. Once a variant is gone, every outstanding presigned URL for it gets a 403.
3. Delete the `ImageMedia` row, conditioned on it still having the `status` this call read (`attribute_not_exists(pk) OR status = :read`).

Any S3 delete failure returns 500 and leaves the row in place. A repeat `DELETE` of an already-gone row returns 204 (idempotent — nothing left to leak). `ImageMedia.error_message` is never written as `"DELETED"` for a removed image; the row itself is deleted instead.

**Processing finishing mid-delete.** If `lambda-image-process` marks a `pending` row `ready` between this call's read and its row-delete, it wrote variants this call's read didn't know about, so the row-delete's status condition fails. The helper rereads the row and runs the delete again with the fresh processed keys — a row leaves `pending` only once, so this happens at most once. If the row delete lands first instead, image-process's own `status = pending` update fails on the now-missing row, and it deletes the variants it just wrote (§5.2). Avatars work the same way.

`lambda-media`, `lambda-engagement` and `lambda-groups` each get `s3:DeleteObject` on the relevant buckets/prefixes ([`infrastructure.md` §6.2](infrastructure.md#62-lambda-functions-request-handlers)).

Callers: the response editor's "remove image" ([`frontend.md` §7.3](frontend.md#73-respondpage-ggncrespondq)/§8.4), a comment's image on comment soft-delete or image removal/replacement ([`engagement.md` §1.1](engagement.md#11-shape)/§1.4/§4.3 — best effort, failure logged at ERROR), and avatar removal/replacement (§11.4 below).

**No bucket versioning** on any media or avatar bucket (with it, a delete would only add a delete marker and the object would survive). Residual copies we can't purge are accepted: a device's service-worker cache (≤30 days for images, ≤90 for avatars, [`frontend.md` §5.2](frontend.md#52-service-worker--caching-strategy)) and ordinary browser caches, plus DynamoDB PITR in prod, which keeps the deleted *row* (metadata only, no image bytes) for 35 days.

### 8.1 Orphan originals

Covered by the originals buckets' lifecycle: every object under `uploads/` expires after 7 days. Normally `lambda-image-process` has already deleted it (§5.1); the rule catches the rest, such as a failed best-effort delete, an upload whose trigger never ran, or a retried upload's abandoned first attempt (§9). No janitor Lambda is needed. The `pending`/`failed` `ImageMedia` rows those leave behind are metadata only and stay.

### 8.2 Detached uploads

If a user uploads but never references the image in a published response, it remains in the bucket. Acceptable for v1.

### 8.3 Lifecycle transitions

None. Originals expire after 7 days (§8.1), so there's nothing old enough to tier, and processed variants are mostly under the 128 KB floor below which Intelligent-Tiering never moves an object. The only other rule on every bucket aborts incomplete multipart uploads after 1 day.

---

## 9. Failure modes

| Failure | UX |
|---|---|
| Presign expired (user took >10 min) | Server returns presign with `expiresInSeconds`; SPA on PUT 403 retries with a fresh presign automatically (one retry). There is no re-presign route: the retry calls `POST /uploads` (or `/avatars`) again and gets a new ID, abandoning the old `pending` row, which §8.1 already tolerates. |
| MIME mismatch (file changed since presign) | S3 rejects (403). SPA shows "couldn't upload — try again". |
| Image too large | Caught client-side first; if bypassed, `lambda-image-process` marks the row `failed` and deletes the original (§3.2). SPA polling sees `failed` and shows the error. |
| Encoder failure | `lambda-image-process` sets `status=failed`. SPA polling sees `failed`, shows error with re-upload button. |
| Decode budget exceeded | Before allocating pixels, `lambda-image-process` refuses a header wider or taller than `MAX_IMAGE_EDGE_PX` (16384) or decoding to more than `MAX_DECODED_IMAGE_BYTES` (256 MB). GIFs are streamed one frame at a time and capped at `MAX_GIF_FRAMES` (300) and `MAX_GIF_TOTAL_PIXELS` (100 M). Over budget is handled like "image too large": `IMAGE_TOO_LARGE`, and the original is deleted. Without it, a small, highly compressed upload could run the 1024 MB Lambda out of memory and leave its row `pending` forever. |
| Presigned image URL expired (long browse session, or the signing Lambda's credentials expired first, §4.2) | `<img>` gets a 403. `MediaImage`/`Avatar` refresh once: refetch the active queries (or the upload, in the editor) for fresh URLs, then remount. A second failure shows a placeholder, or initials for an avatar (§4.4). |
| Delete removes the S3 objects but fails before the row delete | `DELETE /uploads/{id}` returns 500; the `ImageMedia` row is kept so a retry can finish (the S3 deletes themselves are idempotent — a missing object is not an error, §8.0). |

---

## 10. Tests

General strategy in [`testing-and-ci.md`](../development/testing-and-ci.md). Image-pipeline-specific tests:

1. Presign endpoint returns a URL accepted by S3: `backend/crates/lambda-media/tests/uploads.rs::create_upload_returns_a_presigned_put_with_the_right_headers`.
2. ObjectCreated trigger produces both variants for JPEG, PNG, WebP, GIF inputs: `backend/crates/lambda-image-process/src/process/tests.rs::jpeg_input_produces_both_variants_within_their_long_edge`, `::png_input_produces_both_variants_within_their_long_edge`, `::webp_input_produces_both_variants_within_their_long_edge`, `::gif_input_produces_both_variants_as_gif`; end-to-end via S3 event handling in `backend/crates/lambda-image-process/tests/handler.rs::a_pending_image_becomes_ready_with_both_variants_uploaded`.
3. Animated GIF round-trip preserves animation: `backend/crates/lambda-image-process/src/process/tests.rs::an_animated_gif_round_trips_with_all_its_frames`.
4. Image with EXIF orientation is rotated correctly in output (the `image` crate's `image::imageops::orient_from_exif` step): `backend/crates/lambda-image-process/src/process/tests.rs::exif_orientation_6_swaps_recorded_width_and_height`.
5. A 16 MB object landed in the originals bucket is marked `failed` by `lambda-image-process` and the original is deleted (size-cap enforcement per §3.2): `backend/crates/lambda-image-process/tests/handler.rs::an_oversized_object_is_marked_failed_and_the_original_is_deleted`.
6. SVG upload is rejected at the validation step (not in allowed MIME list): `backend/crates/lambda-media/tests/uploads.rs::svg_mime_type_is_rejected_as_bad_type`.
7. Image URLs are presigned for the processed bucket, sign only `host`, expire after `MEDIA_URL_TTL_SECONDS`, and are identical within one signing window (§4.2): unit tests in `backend/crates/media-urls/src/lib.rs`. Group isolation is the handlers' membership checks (each Lambda's non-member tests); no URL for another group's image is ever minted.
8. The 10-image cap is enforced (11th `POST /uploads` returns `IMAGE_LIMIT_EXCEEDED`): `backend/crates/lambda-media/tests/uploads.rs::eleventh_response_upload_hits_the_image_cap`.
9. `DELETE /uploads/{id}` removes the original, both processed variants and the row; a repeat `DELETE` is idempotent and returns 204; an S3 delete failure is a 500 that keeps the row (§8.0): `backend/crates/lambda-media/tests/uploads.rs::delete_removes_the_objects_and_deletes_the_row`, `::deleting_an_already_deleted_image_succeeds_and_touches_nothing`, `::a_failed_s3_delete_is_a_500_and_keeps_the_row`.
10. The original is deleted once the row is `ready` (image and avatar), on decode failure as on oversize, and a failed original delete still leaves the row `ready` (§5.1): `backend/crates/lambda-image-process/tests/handler.rs::a_pending_image_becomes_ready_with_both_variants_uploaded`, `::an_avatar_happy_path_produces_a_256_square_webp`, `::garbage_bytes_are_marked_failed_with_decode_failed_and_the_original_is_deleted`, `::a_failed_original_delete_still_leaves_the_row_ready_and_returns_ok`.

---

## 11. Avatars (separate pipeline)

Avatars are uploaded via a dedicated, simpler pipeline because they need to cross group boundaries (a user belongs to multiple groups; one avatar serves them all).

### 11.1 Buckets and keys

Avatars have their own originals bucket, so `lambda-image-process` can tell an avatar upload from an image upload by its source bucket (§11.2). Their processed variant lives in the shared processed bucket (§2.2):

- `opennewsletter-avatars-originals-{env}-{accountId}` — `uploads/{userId}/{avatarId}.{ext}`
- `opennewsletter-media-processed-{env}-{accountId}` — `avatar/{avatarId}/display.webp`

Constraints:
- Allowed MIME: `image/jpeg`, `image/png`, `image/webp` (no GIF — animated avatars aren't worth the encoder complexity in v1).
- Max bytes: **5 MB** (`MAX_AVATAR_BYTES`). Enforced like §3.2: client-side, at `POST /avatars`, and by `lambda-image-process`, which fails and deletes an oversized original. A presigned PUT can't cap size, and no bucket policy does.
- Processed variant: WebP, 256×256 (center-crop to square), quality 82. No thumb — a 256px avatar is already small.

### 11.2 Upload + processing

Same shape as the per-question pipeline:
1. SPA calls `POST /avatars` → receives presigned PUT URL + `avatarId`.
2. SPA PUTs binary directly to S3 (originals bucket).
3. `lambda-image-process` (same Lambda) receives the S3 event and branches on the source **bucket** (media-originals → response image, avatars-originals → avatar; both buckets use the `uploads/` key prefix internally), produces the 256×256 WebP, updates the `AvatarMedia` row to `status=ready` and deletes the original (§5.1).
4. SPA polls `GET /avatars/{avatarId}` until `ready`, then `PATCH /me` with `avatarMediaId`.

### 11.3 Serving

The same way as images (§4): `avatarUrl` fields carry a presigned GET for `avatar/{avatarId}/display.webp`, signed by `media-urls` (`MediaUrls::avatar_url`). Avatars are signed too, even though a leaked avatar shows no more than any shared group does: signing costs nothing extra and means the bucket needs no public path at all.
- Cached for 90 days (owner decision 2026-10-18): `Cache-Control: private, max-age=7776000, immutable`, and the service worker's `media-avatars` cache expires entries after 90 days. That's longer than images because an avatar's bytes never change under its key. Changing avatars produces a new `avatarId`, so a new path, and the new photo misses every cache.
- The new path only reaches other views once their data refetches. After an avatar change (photo or colour), `useUpdateProfile` writes the `PATCH /me` response into `me` and invalidates every other query flagged `MEDIA_URLS_META` (§4.4), which covers every query that shows member avatars: the group's members, candidates and the newsletter. Other members' devices pick it up on their next refetch, within each query's `staleTime`.

### 11.4 Cleanup

`DELETE /avatars/{id}`, avatar replacement (`PATCH /me` with a new `avatarMediaId`) and clearing (`PATCH /me` with `avatarMediaId: null`) delete the old avatar's original and processed objects and the `AvatarMedia` row — the same shared-helper mechanics as §8.0. The UI goes through a `ConfirmDialog` warning the delete is permanent ([`frontend.md` §7.6](frontend.md#76-settingspage)). No bucket versioning on the avatar originals or processed bucket either, so there's no delete-marker fallback; the accepted residual copies are the same two as §8.0 (SW/browser image cache, prod PITR on the row).
