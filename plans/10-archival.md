# 10 — Archival (Future)

Archival is **deferred** — not part of v1. This document captures the plan so the v1 build doesn't paint us into a corner. Implementers should ensure the data model and key shapes documented elsewhere are forward-compatible with this design (they are; this document is intentionally pinned to existing key formats).

---

## 1. Goal

Move newsletters older than a configurable threshold (default 12 months past publication) out of:
- The hot DynamoDB table (where each row contributes to PITR storage cost and warm-cache fan-out)
- Hot S3 storage class (originals + processed)

…into:
- A flat JSON blob per cycle, stored in a dedicated archive bucket in S3 Glacier Instant Retrieval
- Cold storage for the corresponding image variants (Glacier Flexible Retrieval or Glacier Deep Archive)

Archived newsletters remain readable on demand via a "thaw" endpoint that re-presents the JSON for the SPA.

---

## 2. Trigger

A daily EventBridge rule invokes `lambda-archive-tick`. It scans GSI2 for `gsi2pk = NL_STATUS#published` and `publishedAt < now - 12 months`. For each match, kicks off the archive job (one cycle at a time; archive is idempotent).

---

## 3. Per-cycle archive procedure

```
1. Read Newsletter row + all child entities (Querying the multiple partitions:
     - Locked questions:        GROUP#g#NL#c
     - Responses:               GROUP#g#NL#c#Q#qN          (one query per locked question)
     - Comments:                GROUP#g#NL#c#Q#qN#A#uN     (one query per answer)
     - Reactions:               same partition as comments
     - Image media:             GROUP#g#NL#c (IMG# items)
   ).
2. Compose a self-contained JSON blob:
     {
       "version": 1,
       "groupId": ...,
       "cycleId": ...,
       "questions": [ { ...prompt, kind, options, answers: [...with comments, reactions...] } ],
       "members": [ { userId, displayName, role } ]   // snapshot at time of archive
     }
3. Write to archive bucket: s3://opennewsletter-archive-{env}/g/{groupId}/c/{cycleId}.json
   storage class: STANDARD_IA  (we want quick reads for "thaw" via signed URL)
4. Move image variants:
   - Originals → STANDARD_IA  (or DEEP_ARCHIVE if we never expect to regenerate)
   - Processed → STANDARD_IA  (kept readable so the SPA can fetch them)
5. Stream-driven check: ensure all rows for the cycle are reflected in the blob. If anything has been written since step 1 (DDB stream tail), restart the archive (rare for 12-month-old cycles).
6. TransactWriteItems:
   - Update Newsletter: status="archived", gsi2pk="NL_STATUS#archived", gsi2sk=now, archiveLocation="s3://..."
   - Delete every child row (paginated TransactWriteItems batches of 100)
7. Stream consumer ignores `archived` cycles.
```

The full procedure runs in `lambda-archive` (separate from the daily tick), invoked async with the cycle pointer. Each invocation handles exactly one cycle and is idempotent.

---

## 4. Read path

### 4.1 SPA behavior

- Newsletter list (`GET /groups/{g}/newsletters`) keeps returning archived cycles, but with `status: "archived"` and `archiveLocation` field present.
- Clicking an archived cycle: SPA calls `GET /groups/{g}/newsletters/{c}` which returns 410 Gone with body `{ "type": ".../archived", "archiveLocation": "...", "thawUrl": "..." }` and a `Link: <thawUrl>; rel="thaw"` header.
- SPA fetches `thawUrl` (a pre-signed URL to the archive blob in S3, returned by the server), renders the JSON.

### 4.2 Backend `GET /groups/{g}/newsletters/{c}` for archived

If the row's `status == "archived"`:
- Verify membership.
- Generate a 60s pre-signed S3 URL for the archive blob in the archive bucket.
- Return 410 with the signed URL.

### 4.3 Image visibility

Archived images are still in the processed bucket (we moved storage class to STANDARD_IA, not deleted). CloudFront signed cookies still grant access. Cost: STANDARD_IA storage is ~50% of STANDARD; small wins per archived cycle.

If we later move processed images to DEEP_ARCHIVE, the SPA would need to display "image unavailable" placeholders for archived editions. Leaving processed images in STANDARD_IA keeps archives fully readable.

---

## 5. Forward compatibility checklist

Implementers building v1 should NOT make these mistakes:

- ❌ Don't bake assumptions about cycle counts into a single DDB partition. Each cycle's rows fit comfortably in their own partition (already designed).
- ❌ Don't store derived data only in DynamoDB if it can't be regenerated. The archive procedure regenerates everything from canonical rows; the canonical rows are what we delete.
- ❌ Don't allow comments/reactions to be created on `archived` cycles. The handler must reject `archived` status with `CYCLE_NOT_PUBLISHED`-like error before performing the write.
- ✅ DO ensure every entity carries `groupId` and `cycleId` directly in attributes (already designed) so the archiver can reconstruct without re-deriving from keys.

---

## 6. Inactive-account cleanup (separate but related)

Cognito users who never finished onboarding (created an account but never redeemed an invite) take up directory space. A separate `lambda-cleanup-tick` (daily) deletes Cognito users whose:
- `userCreateDate < now - 30 days`
- AND have no DynamoDB `User` row

The same Lambda also deletes expired invite rows that aren't auto-cleaned by TTL (defensive).

---

## 7. Timeline

This work is **roadmap, not v1**. Estimated effort once needed: ~1 week of focused implementation. The decision point will be when the table approaches a few hundred MB of data or PITR costs become noticeable.

Until then, the only v1 requirement is: nothing in v1 prevents us from building this later.
