# Media uploads and image pipeline

How response, comment and avatar images move from a user's browser to S3, get resized into display/thumb variants, and get served back through CloudFront. Read this before changing anything under `lambda-media`, `lambda-image-process`, or the media-delete helper. The constants in §1 and `backend/crates/shared/src/config.rs` are the source of truth for limits; the code in `backend/crates/lambda-image-process/src/process.rs` is the source of truth for the resize/encode algorithm.

Users can attach up to 10 images per text response. Uploads go directly from browser to S3 via pre-signed URLs (Lambda is never on the upload data path). A subsequent S3 trigger generates two derived variants (thumb + display) and marks the image ready. Reads happen via CloudFront with signed cookies scoped per group.

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

Two buckets, both private (defined in [`infrastructure.md` §5.1](infrastructure.md#51-s3-buckets-mediapersistentstack)):

### 2.1 `opennewsletter-media-originals-{env}-{accountId}`

Originals as uploaded.

```
uploads/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}.{ext}
```

Example: `uploads/01HXG.../202605/01HX.../01HX.../01HXJ.....jpg`

### 2.2 `opennewsletter-media-processed-{env}-{accountId}`

Derived images served via CloudFront.

```
img/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}/display.webp
img/{groupId}/{cycleId}/{questionId}/{userId}/{imageId}/thumb.webp
```

(GIFs use `.gif` extensions instead of `.webp` for both variants.)

The `groupId` prefix is meaningful: it's the boundary of CloudFront-signed-cookie access (§6).

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
type Pending = {
  imageId: string;
  file: File;
  status: "uploading" | "processing" | "ready" | "failed";
  progress: number;       // 0-1
  thumbUrl: string | null;
  displayUrl: string | null;
  altText: string;        // user-entered
  error: string | null;
};
```

Algorithm:
1. User drops/picks files. Reject any failing client-side validation (MIME, size).
2. For each file, compute SHA-256 with `crypto.subtle.digest` (native and asynchronous, so it doesn't block the main thread — simpler than a Web Worker). Omit it if `crypto.subtle` is unavailable.
3. POST `/uploads` → receive `{ imageId, uploadUrl, headers, expiresInSeconds }`. Later calls on the image pass `?groupId=&cycleId=` ([`api.md` §9.0](api.md#90-addressing-an-existing-upload)).
4. PUT to `uploadUrl` using `XMLHttpRequest` (for progress events). On 200 → `status=processing`.
5. Poll `GET /uploads/{imageId}` every 1.5s (with backoff if still pending after 10s).
6. When `status=ready`, replace the in-editor token with the final image. Insert markdown token `![alt](image:{imageId})` into the body.

If the page is closed mid-upload, the partial S3 object is cleaned up by the bucket lifecycle (abort incomplete multipart after 1 day).

---

## 4. CloudFront serving

### 4.1 Distribution config

Recap from [`infrastructure.md` §5.2](infrastructure.md#52-cloudfront-distribution-mediapersistentstack):
- One distribution; default behavior maps `/img/*` to the processed bucket via OAC.
- Trusted KeyGroup with one CloudFront public key: RSA 2048, its own pair per env ([`auth.md` §10](auth.md#10-identity-provider-setup-design-relevant-facts)). CloudFront's signed-cookie/URL scheme requires an RSA key — the `sign_policy` call in §4.3 can't use an ECDSA key.
- Custom domain: `cdn.opennewsletter.example.com`.
- An image delete invalidates the deleted paths so a cached edge copy doesn't keep serving it after the row and originals are gone — mechanics in §8.0.

### 4.2 Signed-cookie scope

We use **signed cookies** (not signed URLs) so the SPA can request many images without each URL needing pre-signing.

The cookie's policy restricts to a single resource pattern — but we want the cookie to span all groups the caller is a member of. Two options:

- **Option A**: One cookie per group, scoped to `https://cdn.opennewsletter.example.com/img/{groupId}/*`. The SPA fetches a fresh cookie when the user switches groups.
- **Option B**: One cookie scoped to `https://cdn.opennewsletter.example.com/img/*`, granting access to ALL groups. Simpler but lets a member exfiltrate URLs from groups they're not in (if they can guess paths).

**Decision: Option A.** The SPA calls `GET /media-cookie?groupId=...` whenever it switches groups (and on initial load). Cookie lifetime: 1 hour. Auto-refresh 5 minutes before expiry.

### 4.3 `GET /media-cookie` implementation

Request/response shape is [`api.md` §9.5](api.md#95-get-media-cookie). The handler (`get_media_cookie` in `backend/crates/lambda-media/src/handlers.rs`) requires membership in `groupId`, builds the CloudFront custom policy scoped to `/img/{groupId}/*` with a 1-hour expiry, and signs it via the pure helpers in `backend/crates/lambda-media/src/cloudfront.rs`: `policy_json` for the exact (whitespace-free, field-order-fixed) policy string, `sign_policy` for the RSA PKCS#1 v1.5 signature over its SHA-1 digest, and `modified_base64_encode` for CloudFront's modified base64 (`+`→`-`, `=`→`_`, `/`→`~`). The signing key PEM comes from Secrets Manager and accepts either PKCS#1 or PKCS#8 form. `Set-Cookie` is emitted only when `MEDIA_COOKIE_DOMAIN` is configured (prod); dev returns the JSON body alone (§4.5).

### 4.4 SPA usage

Before rendering any image from a given group:

```ts
await api.media.ensureCookie(groupId);
// proceeds; <img src="..."> requests will include the cookie automatically
```

Cookies are first-party from the browser's perspective because `cdn.opennewsletter.example.com` shares a registrable domain with the SPA. The fetch must be made with `credentials: "include"` (handled in `api/client.ts`).

### 4.5 Dev environments: signed query params instead of cookies

The cookie scheme requires the SPA, API, and CDN to share a registrable domain. In `dev` there are no custom domains ([`environments.md` §2](../development/environments.md#2-environments)) — the API is an `execute-api.amazonaws.com` URL and the CDN a `cloudfront.net` URL — so the browser can neither receive the `Set-Cookie` for the CDN's domain nor send cookies to it.

Dev therefore uses **CloudFront signed URLs built from the same values**: `GET /media-cookie` already returns `{ policy, signature, keyPairId }` in its JSON body, and CloudFront accepts exactly those as query parameters (`?Policy=...&Signature=...&Key-Pair-Id=...`). A helper `utils/media.ts#withMediaAuth(url)` appends them to every image URL when `env.appEnv === "dev"` and is a pass-through in prod (where cookies do the work). Same signing code server-side, same key group, same per-group tenant isolation — only the transport differs.

Trade-off: query-stringed URLs churn the service worker's `CacheFirst` image cache when the signature refreshes hourly. Acceptable — dev doesn't need offline-cache fidelity.

---

## 5. `lambda-image-process` (S3 trigger)

### 5.1 Trigger configuration

S3 event: `s3:ObjectCreated:*` filtered to `prefix: uploads/`, on both the media-originals and avatars-originals buckets. S3 invokes the Lambda once per object (no batching configurable on direct S3→Lambda notifications).

Environment: `TABLE_NAME`, `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET` (to tell the two sources apart), `PROCESSED_BUCKET`, `AVATARS_PROCESSED_BUCKET`.

Implementation notes:
- The ready/failed updates are conditioned on `status = pending`. A lost condition means another invocation already finished, so it is logged and skipped, not retried.
- Size and decode failures are terminal and recorded on the row (`error_message` = `IMAGE_TOO_LARGE` / `IMAGE_DECODE_FAILED`). Transient S3/DynamoDB errors return `Err` so Lambda's async retry runs.
- Variants are never upscaled. Recorded `width`/`height` are the original's after EXIF orientation.
- Cache-Control on writes: `public, max-age=5184000, immutable` (60 days, never more than two months) for `img/*`, `public, max-age=86400, immutable` for `avatar/*`.

### 5.2 Algorithm (Rust)

Implementation: `backend/crates/lambda-image-process/src/handler.rs` wires S3 to the row (per record: parse the key into `groupId`/`cycleId`/`questionId`/`userId`/`imageId`, skip if the row isn't still `pending`, download, hand the bytes to `process.rs`, upload the two returned variants, mark the row `ready` with their keys and the oriented dimensions). `process.rs` is the pure image logic (no AWS, unit-tested in `process/tests.rs`):

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
- `s3:GetObject` + `s3:DeleteObject` on both originals buckets (delete purges oversized uploads)
- `s3:PutObject` on both processed buckets
- `dynamodb:GetItem` + `dynamodb:UpdateItem` on the table

Memory: 1024 MB (image processing is CPU-bound; arm64 Lambda's 1024 MB gives a full vCPU). Timeout 60s. Largest expected image ~15MB; processing in well under 10s typical.

---

## 6. CloudFront cache and tenant isolation summary

- Distribution serves only the processed bucket via OAC.
- Cache is **shared** across users — keyed by URL only (no `Vary` on cookies).
- Access is gated by **trusted signers**: viewer must present valid signed cookies for the path requested.
- A user's cookie is scoped to one group at a time; the SPA refreshes when switching.
- Worst case: a member of group A guesses an image URL belonging to group B. They lack cookies for group B's path → CloudFront returns 403. Tenant boundary preserved.

### 6.1 Why CloudFront (decision record)

**Decision: CloudFront + signed cookies**, over two alternatives. A public S3 bucket with unguessable keys is radically simpler (static URLs, no signing, perfect browser/SW cache) but abandons the membership-based tenant isolation the rest of the system enforces — a leaked URL works forever. Private S3 with per-image SigV4 presigned GETs preserves tenant isolation but pushes signing into every handler that returns image URLs, and signed URLs change per request, breaking the SW `CacheFirst` strategy in [`frontend.md` §5.2](frontend.md#52-service-worker--caching-strategy).

Cost is effectively a wash at our scale: CloudFront egress ($0.085/GB, PriceClass_100) is actually slightly *cheaper* than S3 direct egress ($0.09/GB), and dropping CloudFront would only save ~$0.40/mo (one Secrets Manager secret for the signing keypair). The deciding, non-cost benefits are an edge cache for repeated reads of published newsletters, and stable URLs that the markdown renderer's `image:{imageId}` resolution and the SW cache both depend on.

---

## 7. Image references in markdown

Stored format: `![alt text](image:{imageId})`.

Server-side: nothing. The `body` is stored verbatim.

Render-side (`utils/markdown.tsx`):
- There's no remark plugin. `MarkdownBody` overrides react-markdown's `img` renderer: an `image:{id}` src is looked up in an `images` table the parent passes in (`PublishedAnswerResponse.images`, or the editor's live uploads) and rendered with `CdnImage`. An unknown ID renders nothing.
- `image:` must pass both `react-markdown`'s `urlTransform` and the `rehype-sanitize` schema. See [`frontend.md` §9](frontend.md#9-renderers).
- Any other image URL renders as a link, never an `<img>`.
- `ImageGallery` shows an answer's images that the body doesn't reference inline.

This indirection means we can change the CDN host (or rotate signed cookies) without rewriting stored markdown.

---

## 8. Cleanup paths

### 8.0 Explicit deletion

`DELETE /uploads/{imageId}` (owner only; `IMAGE_IN_USE` guards per [`api.md` §9.4](api.md#94-delete-uploadsimageid)) deletes for real, via one shared helper: the `media-delete` library crate (`backend/crates/media-delete` — not `shared`, because `shared` can't depend on `persistence` and the helper deletes the row). The sequence, run in this order, with each step idempotent so a retry after a failure redoes it safely:

1. Delete the original S3 object, then each processed variant (display, thumb) the row has a key for. A row with no processed key (still `pending`/`failed`) was never served, so there's nothing to delete or invalidate there.
2. CloudFront invalidation for the exact processed paths the row had — never a `/*` wildcard, which CloudFront caps at 15 in progress per distribution against 3,000 exact paths (the first 1,000 paths/month are free).
3. Delete the `ImageMedia` row, conditioned on it still having the `status` this call read (`attribute_not_exists(pk) OR status = :read`).

Any S3 delete or invalidation failure returns 500 and leaves the row in place. A repeat `DELETE` of an already-gone row returns 204 (idempotent — nothing left to leak). `ImageMedia.error_message` is never written as `"DELETED"` for a removed image; the row itself is deleted instead.

**Processing finishing mid-delete.** If `lambda-image-process` marks a `pending` row `ready` between this call's read and its row-delete, it wrote variants this call's read didn't know about, so the row-delete's status condition fails. The helper rereads the row and runs the delete again with the fresh processed keys — a row leaves `pending` only once, so this happens at most once. If the row delete lands first instead, image-process's own `status = pending` update fails on the now-missing row, and it deletes the variants it just wrote (§5.2). Avatars work the same way.

`lambda-media`, `lambda-engagement` and `lambda-groups` each get `s3:DeleteObject` on the relevant buckets/prefixes and `cloudfront:CreateInvalidation` on the distribution ([`infrastructure.md`](infrastructure.md)).

Callers: the response editor's "remove image" ([`frontend.md` §7.3](frontend.md#73-respondpage-ggncrespondq)/§8.4), a comment's image on comment soft-delete or image removal/replacement ([`engagement.md` §1.1](engagement.md#11-shape)/§1.4/§4.3 — best effort, failure logged at ERROR), and avatar removal/replacement (§11.4 below).

**No bucket versioning** on any media or avatar bucket (with it, a delete would only add a delete marker and the object would survive). Residual copies we can't purge are accepted: a device's service-worker image cache (≤30 days, [`frontend.md` §5.2](frontend.md#52-service-worker--caching-strategy)) and ordinary browser caches, plus DynamoDB PITR in prod, which keeps the deleted *row* (metadata only, no image bytes) for 35 days.

### 8.1 Orphan originals

**Not built.** A janitor Lambda that hard-deletes originals once their `ImageMedia` row has sat `failed` or `pending` for more than 24h (e.g. a closed tab mid-upload, or a failed trigger) would close this gap. For now the only automatic cleanup on originals is the bucket lifecycle's multipart-abort (§3.3); storage cost for a few orphan files is trivial, so this is accepted as-is.

### 8.2 Detached uploads

If a user uploads but never references the image in a published response, it remains in the bucket. Acceptable for v1.

### 8.3 Lifecycle transitions

Both buckets transition to `INTELLIGENT_TIERING` after 30 days (configured in CDK). At scale, this halves storage cost.

---

## 9. Failure modes

| Failure | UX |
|---|---|
| Presign expired (user took >10 min) | Server returns presign with `expiresInSeconds`; SPA on PUT 403 retries with a fresh presign automatically (one retry). There is no re-presign route: the retry calls `POST /uploads` (or `/avatars`) again and gets a new ID, abandoning the old `pending` row, which §8.1 already tolerates. |
| MIME mismatch (file changed since presign) | S3 rejects (403). SPA shows "couldn't upload — try again". |
| Image too large | Caught client-side first; if bypassed, `lambda-image-process` marks the row `failed` and deletes the original (§3.2). SPA polling sees `failed` and shows the error. |
| Encoder failure | `lambda-image-process` sets `status=failed`. SPA polling sees `failed`, shows error with re-upload button. |
| Decode budget exceeded | Before allocating pixels, `lambda-image-process` refuses a header wider or taller than `MAX_IMAGE_EDGE_PX` (16384) or decoding to more than `MAX_DECODED_IMAGE_BYTES` (256 MB). GIFs are streamed one frame at a time and capped at `MAX_GIF_FRAMES` (300) and `MAX_GIF_TOTAL_PIXELS` (100 M). Over budget is handled like "image too large": `IMAGE_TOO_LARGE`, and the original is deleted. Without it, a small, highly compressed upload could run the 1024 MB Lambda out of memory and leave its row `pending` forever. |
| CloudFront cookie expired during a long browse session | `<img>` request → 403 → SPA's image error handler refreshes cookie and retries. |
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
7. CloudFront signed cookies for group A do not allow access to group B images: `backend/crates/lambda-media/tests/media_cookie.rs::a_group_a_cookie_does_not_scope_to_a_group_b_url`.
8. The 10-image cap is enforced (11th `POST /uploads` returns `IMAGE_LIMIT_EXCEEDED`): `backend/crates/lambda-media/tests/uploads.rs::eleventh_response_upload_hits_the_image_cap`.
9. `DELETE /uploads/{id}` removes the original, both processed variants and the row, and issues a CloudFront invalidation; a repeat `DELETE` is idempotent and returns 204 (§8.0): `backend/crates/lambda-media/tests/uploads.rs::delete_removes_the_objects_invalidates_the_cdn_and_deletes_the_row`, `::deleting_an_already_deleted_image_succeeds_and_touches_nothing`.

---

## 11. Avatars (separate pipeline)

Avatars are uploaded via a dedicated, simpler pipeline because they need to cross group boundaries (a user belongs to multiple groups; one avatar serves them all).

### 11.1 Buckets and keys

A pair of buckets, structured like §2 but with avatar-specific prefixes:

- `opennewsletter-avatars-originals-{env}-{accountId}` — `uploads/{userId}/{avatarId}.{ext}`
- `opennewsletter-avatars-processed-{env}-{accountId}` — `avatar/{avatarId}/display.webp`

Constraints:
- Allowed MIME: `image/jpeg`, `image/png`, `image/webp` (no GIF — animated avatars aren't worth the encoder complexity in v1).
- Max bytes: **5 MB** (`MAX_AVATAR_BYTES`). Enforced like §3.2: client-side, at `POST /avatars`, and by `lambda-image-process`, which fails and deletes an oversized original. A presigned PUT can't cap size, and no bucket policy does.
- Processed variant: WebP, 256×256 (center-crop to square), quality 82. No thumb — a 256px avatar is already small.

### 11.2 Upload + processing

Same shape as the per-question pipeline:
1. SPA calls `POST /avatars` → receives presigned PUT URL + `avatarId`.
2. SPA PUTs binary directly to S3 (originals bucket).
3. `lambda-image-process` (same Lambda) receives the S3 event and branches on the source **bucket** (media-originals → response image, avatars-originals → avatar; both buckets use the `uploads/` key prefix internally), produces the 256×256 WebP, updates the `AvatarMedia` row to `status=ready`.
4. SPA polls `GET /avatars/{avatarId}` until `ready`, then `PATCH /me` with `avatarMediaId`.

### 11.3 Serving

A second CloudFront **behavior** (not a second distribution) on the same `cdn.{domain}` host:
- Path pattern: `/avatar/*` → processed-avatars bucket via OAC.
- **No trusted KeyGroup** — avatars are not signed. Tenant isolation is unnecessary: an avatar URL leak grants someone the ability to render a 256×256 image of a user, which is identical to seeing them in any group they share with the leaker.
- The `avatarId` is a UUIDv7, unguessable in practice.
- Cache-control: `public, max-age=86400, immutable`. Changing avatars produces a new `avatarId`, so cached entries never collide.

### 11.4 Cleanup

`DELETE /avatars/{id}`, avatar replacement (`PATCH /me` with a new `avatarMediaId`) and clearing (`PATCH /me` with `avatarMediaId: null`) delete the old avatar's original and processed objects, invalidate `/avatar/{avatarId}/display.webp`, and delete the `AvatarMedia` row — the same shared-helper mechanics as §8.0. The UI goes through a `ConfirmDialog` warning the delete is permanent ([`frontend.md` §7.6](frontend.md#76-settingspage)). No bucket versioning on the avatars buckets either, so there's no delete-marker fallback; the accepted residual copies are the same two as §8.0 (SW/browser image cache, prod PITR on the row).
