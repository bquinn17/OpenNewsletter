# 08 — Media Uploads & Image Pipeline

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

Two buckets, both private (defined in `01-infrastructure-cdk.md` §5.1):

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

```
- caller is member of body.groupId
- body.cycleId matches a Newsletter for that group with status=open
- body.questionId is a LockedQuestion for (groupId, cycleId)
- body.mimeType in allowed set
- body.byteSize ≤ 15728640
- caller's image count for (cycleId, questionId) (status != failed) < 10
```

The 10-image cap is enforced via a Query of GSI1 (AP17) filtered to the question; cheaper than scanning the response.

### 3.2 Presign

Use the AWS Rust SDK's `Client::put_object().presigned(PresigningConfig::expires_in(600))`. Set:

- `Content-Length` constraint: signed via `presigning` won't enforce this server-side, so the request includes a returned `headers.Content-Length` directive AND the bucket policy enforces a **maximum size** via a separate `s3:PutObject` policy condition `s3:content-length-range: [1, 15728640]`. Files exceeding the limit are rejected by S3 itself.
- `Content-Type` matches the requested MIME type (signed).
- Server-side encryption is on by default (bucket-level), no header needed.

If the client sent `sha256` in the body, include `x-amz-content-sha256` header in the signed request — gives object integrity guarantee.

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
2. For each file, compute SHA-256 in a Web Worker (off the main thread) for ~50MB-eligible files.
3. POST `/uploads` → receive `{ imageId, uploadUrl, headers, expiresInSeconds }`.
4. PUT to `uploadUrl` using `XMLHttpRequest` (for progress events). On 200 → `status=processing`.
5. Poll `GET /uploads/{imageId}` every 1.5s (with backoff if still pending after 10s).
6. When `status=ready`, replace the in-editor token with the final image. Insert markdown token `![alt](image:{imageId})` into the body.

If the page is closed mid-upload, the partial S3 object is cleaned up by the bucket lifecycle (abort incomplete multipart after 1 day).

---

## 4. CloudFront serving

### 4.1 Distribution config

Recap from `01-infrastructure-cdk.md` §5.2:
- One distribution; default behavior maps `/img/*` to the processed bucket via OAC.
- Trusted KeyGroup with one CloudFront public key.
- Custom domain: `cdn.opennewsletter.example.com`.

### 4.2 Signed-cookie scope

We use **signed cookies** (not signed URLs) so the SPA can request many images without each URL needing pre-signing.

The cookie's policy restricts to a single resource pattern — but we want the cookie to span all groups the caller is a member of. Two options:

- **Option A**: One cookie per group, scoped to `https://cdn.opennewsletter.example.com/img/{groupId}/*`. The SPA fetches a fresh cookie when the user switches groups.
- **Option B**: One cookie scoped to `https://cdn.opennewsletter.example.com/img/*`, granting access to ALL groups. Simpler but lets a member exfiltrate URLs from groups they're not in (if they can guess paths).

**Decision: Option A.** The SPA calls `GET /media-cookie?groupId=...` whenever it switches groups (and on initial load). Cookie lifetime: 1 hour. Auto-refresh 5 minutes before expiry.

### 4.3 `GET /media-cookie` implementation

```rust
fn handle(req) -> Response {
    let claims = extract_claims(req)?;
    let group_id = req.query_param("groupId").ok_or(VALIDATION_FAILED)?;
    require_membership(&repo, &claims.sub, &group_id, false).await?;

    let resource = format!("https://cdn.opennewsletter.example.com/img/{}/*", group_id);
    let expires = (Utc::now() + Duration::hours(1)).timestamp();

    let policy = json!({
        "Statement": [{
            "Resource": resource,
            "Condition": {
                "DateLessThan": { "AWS:EpochTime": expires }
            }
        }]
    }).to_string();

    let private_key_pem = SIGNING_KEY.get().await;  // from Secrets Manager
    let signature = rsa_sha1_sign(&private_key_pem, policy.as_bytes());

    let policy_b64 = base64_aws_encode(policy);
    let signature_b64 = base64_aws_encode(signature);

    let cookies = [
        format!("CloudFront-Policy={}; Domain=.opennewsletter.example.com; Path=/img/{}/; HttpOnly; Secure; SameSite=None", policy_b64, group_id),
        format!("CloudFront-Signature={}; Domain=.opennewsletter.example.com; Path=/img/{}/; HttpOnly; Secure; SameSite=None", signature_b64, group_id),
        format!("CloudFront-Key-Pair-Id={}; Domain=.opennewsletter.example.com; Path=/img/{}/; HttpOnly; Secure; SameSite=None", KEY_PAIR_ID, group_id),
    ];

    Response::ok()
        .header_multi("Set-Cookie", cookies)
        .json(json!({ "policy": policy_b64, "signature": signature_b64, "keyPairId": KEY_PAIR_ID, "expiresAt": expires }))
}
```

`base64_aws_encode` uses CloudFront's modified base64 (replace `+` → `-`, `=` → `_`, `/` → `~`).

### 4.4 SPA usage

Before rendering any image from a given group:

```ts
await api.media.ensureCookie(groupId);
// proceeds; <img src="..."> requests will include the cookie automatically
```

Cookies are first-party from the browser's perspective because `cdn.opennewsletter.example.com` shares a registrable domain with the SPA. The fetch must be made with `credentials: "include"` (handled in `api/client.ts`).

---

## 5. `lambda-image-process` (S3 trigger)

### 5.1 Trigger configuration

S3 event: `s3:ObjectCreated:*` filtered to `prefix: uploads/`. Async batch with `BatchSize: 1` for simplicity.

### 5.2 Algorithm (Rust)

```rust
async fn handle(event: S3Event) -> Result<()> {
    for record in event.records {
        let key = record.s3.object.key;
        let bucket = record.s3.bucket.name;
        let parts = parse_key(&key)?;   // (groupId, cycleId, questionId, userId, imageId, ext)
        let img_id = parts.image_id;

        // Fetch the row
        let row = repo.get_image_media(&parts.group_id, &parts.cycle_id, &img_id).await?;
        if row.status != "pending" {
            continue;   // already processed; idempotent
        }

        // Download
        let object = s3.get_object().bucket(&bucket).key(&key).send().await?;
        let bytes = object.body.collect().await?.into_bytes();

        // Probe (using `image` crate)
        let format = image::guess_format(&bytes)?;
        let img = image::load_from_memory_with_format(&bytes, format)?;
        let (width, height) = (img.width(), img.height());

        let (display_bytes, thumb_bytes, ext) = match format {
            ImageFormat::Gif => (
                resize_gif(&bytes, 1200)?,
                resize_gif(&bytes, 400)?,
                "gif",
            ),
            _ => (
                encode_webp(&img.resize_to_long_edge(1200), 82)?,
                encode_webp(&img.resize_to_long_edge(400), 75)?,
                "webp",
            ),
        };

        // Upload variants
        let display_key = format!("img/{}/{}/{}/{}/{}/display.{}",
            parts.group_id, parts.cycle_id, parts.question_id, parts.user_id, img_id, ext);
        let thumb_key = format!("img/{}/{}/{}/{}/{}/thumb.{}",
            parts.group_id, parts.cycle_id, parts.question_id, parts.user_id, img_id, ext);

        s3.put_object().bucket(PROCESSED_BUCKET).key(&display_key)
            .body(display_bytes.into()).content_type(mime_for(ext)).send().await?;
        s3.put_object().bucket(PROCESSED_BUCKET).key(&thumb_key)
            .body(thumb_bytes.into()).content_type(mime_for(ext)).send().await?;

        // Mark ready
        repo.update_image_media(&parts.group_id, &img_id,
            UpdateImage {
                status: "ready",
                width,
                height,
                display_key: Some(display_key),
                thumb_key: Some(thumb_key),
                processed_at: Some(Utc::now()),
            }
        ).await?;
    }
    Ok(())
}
```

### 5.3 Image library

`image = "0.25"` for non-GIF formats. GIF resize uses `gifski` or `image::codecs::gif::GifEncoder` frame-by-frame to preserve animation. WebP via `image::codecs::webp::WebPEncoder`.

If the encode fails for any reason, set `status = "failed"` with an error message field; the frontend will surface a "couldn't process this image" error and offer re-upload.

### 5.4 Permissions

The Lambda's role:
- `s3:GetObject` on originals bucket (prefix `uploads/`)
- `s3:PutObject` on processed bucket (prefix `img/`)
- `dynamodb:UpdateItem` on the table

Memory: 1024 MB (image processing is CPU-bound; arm64 Lambda's 1024 MB gives a full vCPU). Timeout 60s. Largest expected image ~15MB; processing in well under 10s typical.

---

## 6. CloudFront cache and tenant isolation summary

- Distribution serves only the processed bucket via OAC.
- Cache is **shared** across users — keyed by URL only (no `Vary` on cookies).
- Access is gated by **trusted signers**: viewer must present valid signed cookies for the path requested.
- A user's cookie is scoped to one group at a time; the SPA refreshes when switching.
- Worst case: a member of group A guesses an image URL belonging to group B. They lack cookies for group B's path → CloudFront returns 403. Tenant boundary preserved.

---

## 7. Image references in markdown

Stored format: `![alt text](image:{imageId})`.

Server-side: nothing. The `body` is stored verbatim.

Render-side (`utils/markdown.ts`):
- Custom `remark-image-token` plugin replaces `image:{id}` URLs with the resolved `displayUrl`.
- The plugin needs a lookup table (`imageId -> ImageMedia`), provided by the parent component (the response includes `images` array).
- Sanitizer allows `image:` and `https://cdn.opennewsletter.example.com/...` URLs only.

This indirection means we can change the CDN host (or rotate signed cookies) without rewriting stored markdown.

---

## 8. Cleanup paths

### 8.1 Orphan originals

If `ImageMedia` row is `failed` or `pending` for >24h (e.g. user closed tab mid-upload, or trigger failed), a future janitor Lambda hard-deletes the originals. **Deferred — for v1, the bucket lifecycle deletion (no automatic deletion configured for originals beyond the multipart-abort) is acceptable; storage cost for a few orphan files is trivial.**

### 8.2 Detached uploads

If a user uploads but never references the image in a published response, it remains in the bucket. Acceptable for v1.

### 8.3 Lifecycle transitions

Both buckets transition to `INTELLIGENT_TIERING` after 30 days (configured in CDK). At scale, this halves storage cost.

---

## 9. Failure modes

| Failure | UX |
|---|---|
| Presign expired (user took >10 min) | Server returns presign with `expiresInSeconds`; SPA on PUT 403 retries with a fresh presign automatically (one retry). |
| MIME mismatch (file changed since presign) | S3 rejects (403). SPA shows "couldn't upload — try again". |
| Image too large | Caught client-side first; if somehow bypassed, S3 rejects via bucket policy. |
| Encoder failure | `lambda-image-process` sets `status=failed`. SPA polling sees `failed`, shows error with re-upload button. |
| CloudFront cookie expired during a long browse session | `<img>` request → 403 → SPA's image error handler refreshes cookie and retries. |

---

## 10. Tests required

Detailed in `11-testing-ci-cd.md`. Image-pipeline-specific tests:

1. Presign endpoint returns a URL accepted by S3.
2. ObjectCreated trigger produces both variants for JPEG, PNG, WebP, GIF inputs.
3. Animated GIF round-trip preserves animation.
4. Image with EXIF orientation is rotated correctly in output (the `image` crate's `image::imageops::orient_from_exif` step).
5. 16 MB upload is rejected by S3 policy.
6. SVG upload is rejected at the validation step (not in allowed MIME list).
7. CloudFront signed cookies for group A do not allow access to group B images.
8. The 10-image cap is enforced (11th `POST /uploads` returns `IMAGE_LIMIT_EXCEEDED`).
