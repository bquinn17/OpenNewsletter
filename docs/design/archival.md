# Archival

**Status: not built.** Nothing in this document is implemented — there is no `lambda-archive-tick`, no `lambda-archive`, and no cycle has ever reached `status=archived`. It records the intended design for moving old newsletters out of hot storage, for whoever picks that work up, and it exists now so that building it later doesn't need a migration: the key shapes and attributes chosen for [`data-model.md`](data-model.md) are already pinned to what this design needs (every entity carries `group_id`/`cycle_id` as plain attributes, §5), and the `archived` status this document adds to the Newsletter state machine is already reserved in [`lifecycle.md`](lifecycle.md) and rejected by the engagement routes ([`api.md` §8](api.md#8-engagement-routes-comments--reactions)).

---

## 1. Goal

Move newsletters older than a configurable threshold (default 12 months past publication) out of:
- The hot DynamoDB table (where each row contributes to PITR storage cost and warm-cache fan-out)
- Hot S3 storage class (originals + processed)

…into:
- A flat JSON blob per cycle, stored in a dedicated archive bucket in `STANDARD_IA` (§3 step 3: it's read on demand via a signed URL)
- Cold storage for the corresponding image variants (Glacier Flexible Retrieval or Glacier Deep Archive)

Archived newsletters remain readable on demand via a "thaw" endpoint that re-presents the JSON for the SPA.

---

## 2. Trigger

A daily EventBridge rule would invoke `lambda-archive-tick`, which queries GSI2 for `gsi2pk = NL_STATUS#published` and filters on `published_at < now - 12 months`. (Published rows carry the `9999-12-31T00:00:00Z` sentinel in `gsi2sk`, so the key can't range on publication date. Either filter, as here, or have `publish` write the archival cutoff into `gsi2sk` instead of the sentinel, as [`lifecycle.md` §5.3](lifecycle.md#53-publishnl) anticipates.) Each match would kick off the archive job, one cycle at a time; the archive job is designed to be idempotent.

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

The full procedure would run in `lambda-archive` (separate from the daily tick), invoked async with the cycle pointer. Each invocation is designed to handle exactly one cycle and to be idempotent.

---

## 4. Read path

### 4.1 SPA behavior

- Newsletter list (`GET /groups/{g}/newsletters`) would keep returning archived cycles, but with `status: "archived"` and an `archiveLocation` field present.
- Clicking an archived cycle: the SPA would call `GET /groups/{g}/newsletters/{c}`, which returns 410 Gone with body `{ "type": ".../archived", "archiveLocation": "...", "thawUrl": "..." }` and a `Link: <thawUrl>; rel="thaw"` header.
- The SPA would fetch `thawUrl` (a pre-signed URL to the archive blob in S3, returned by the server) and render the JSON.

### 4.2 Backend `GET /groups/{g}/newsletters/{c}` for archived

If the row's `status == "archived"`:
- Verify membership.
- Generate a 60s pre-signed S3 URL for the archive blob in the archive bucket.
- Return 410 with the signed URL.

### 4.3 Image visibility

Archived images would stay in the processed bucket — only their storage class moves to `STANDARD_IA`, not a delete — so a presigned GET would still grant access the same way as any other image ([`media.md` §4](media.md#4-serving-presigned-s3-gets)). `STANDARD_IA` storage is roughly 50% of `STANDARD`; a small win per archived cycle.

Moving processed images to `DEEP_ARCHIVE` instead would need the SPA to display "image unavailable" placeholders for archived editions. Leaving processed images in `STANDARD_IA` keeps archives fully readable, at the cost of a smaller storage saving.

---

## 5. Forward compatibility

The current data model and API already guarantee what this design needs, with no changes required to build it later:

- **Each cycle's rows live in their own partitions.** Nothing elsewhere bakes in an assumption about how many cycles a group accumulates, so the archiver can delete one cycle's rows without touching any other cycle's.
- **Everything the archive blob needs is regenerable from canonical rows.** The archive procedure (§3) derives the blob entirely from the rows it's about to delete; no data this design needs is stored only in a form that can't be reconstructed.
- **Writes to an `archived` cycle are already rejected.** The engagement routes' check order ([`api.md` §8](api.md#8-engagement-routes-comments--reactions)) returns 410 `NEWSLETTER_ARCHIVED` before the `published`-status check, so comments and reactions can't be created on an archived cycle once one exists — even though no cycle can reach `archived` today without the archival job this document describes.
- **Every entity carries `group_id` and `cycle_id` as plain attributes**, not only encoded in its key ([`data-model.md` §2](data-model.md#2-entity-catalog)), so the archiver can reconstruct a cycle's data without re-deriving identifiers from key strings.

---

## 6. Inactive-account cleanup (separate but related)

Cognito users who never finished onboarding (created an account but never redeemed an invite) take up directory space. A separate `lambda-cleanup-tick`, running daily, would delete Cognito users whose:
- `userCreateDate < now - 30 days`
- AND have no DynamoDB `User` row

The same Lambda would also delete expired invite rows that aren't auto-cleaned by TTL (defensive).

---

## 7. Timeline

This is roadmap work, not scheduled. Estimated effort once needed: about a week of focused implementation. The trigger for picking it up will be the table approaching a few hundred MB of data, or PITR costs becoming noticeable.
