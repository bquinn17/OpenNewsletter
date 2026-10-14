# OpenNewsletter design review

**Overall: B+.** The architecture is sound and cheap, and the correctness work is better than most production code. The two weak spots: request-path I/O is written as sequential awaits, so latency grows with group size, and none of the 13 milestones has been deployed yet.

| Area | Grade | Why |
|---|---|---|
| Architecture | A− | Lambda behind an HTTP API, single-table DynamoDB, nothing always-on, no NAT. Tenant isolation lives in one gate (`require_membership`), and every route I checked enforces it, including the ones without a `groupId` in the path. |
| Data integrity | A | Writes use conditional checks and transactions, and there are concurrency tests for the vote cap, the last admin, double counts and comment races. Several real bugs were found and fixed this way. |
| Performance at scale | C+ | Latency grows linearly in a few places, and list queries don't paginate (details below). |
| Cost | A− at idle, B at scale | M13.5 already fixes the idle overhead. At scale the costs that grow are Cognito monthly active users and stored original images. |
| Security | B+ | Strong CSP, 80-bit invite codes, group-scoped signed cookies. A few gaps are listed below. |
| Operational readiness | D | Nothing has been deployed. Thirteen milestones are "code-complete, deploy unverified". This is the biggest risk in the project. |

## Fix before prod

1. **The published-edition read is a serial N+1 chain.** In [handlers.rs:287](backend/crates/lambda-newsletters/src/handlers.rs#L287), each answer makes these calls one after another:
   - a `GetItem` for the author
   - one `GetItem` per image
   - a comments query, then a `BatchGet` for the commenters
   - a separate reactions query

   - **Typical group** (15 members, 4 text questions): about 250 round trips, roughly 1.5–2.5 s.
   - **At the 50-member cap with 10 images per answer:** about 2,800 round trips. That exceeds the 10 s Lambda timeout, so the busiest page in the app would return 504s.

   The fix fits the existing key design, so no schema change is needed:
   - Locked questions (`Q#`) and images (`IMG#`) share the `GROUP#g#NL#c` partition, so **one query** fetches the whole edition's questions and image metadata.
   - Comments (`C#`) and reactions (`R#`) share the per-answer partition, so **one query per answer** instead of two. Run them concurrently with `buffer_unordered(16)`.
   - **One `BatchGet`** covers every author and commenter.

   That's about 4 waves of round trips, roughly 50–100 ms whatever the group size.

2. **None of the list queries paginate**: `list_answers`, `list_comments`, `list_candidates`, `list_members_for_group` and `list_newsletters_for_group`. DynamoDB returns at most 1 MB per query, and anything past that is silently dropped. 50 answers at the 20,000-character limit is about 1 MB, which is exactly the member cap.
   - This matters most for M13.5: auto-publish will loop over `list_answers`, so a truncated page would leave drafts unpublished.
   - Add a `query_all` helper next to `batch_get_items` and route every list query through it.

3. **Check the Lambda concurrency quota.** Newer AWS accounts can start at 10 concurrent executions, shared across all ~13 functions. Check it with `aws service-quotas get-service-quota --service-code lambda --quota-code L-B99A9384` and request an increase before launch. If the limit stays at 10, finding 1 makes it much worse: 2-second requests on the 1st of the month will queue up and fail.

## Performance and cost at scale

- **Merge the 8 API Lambdas into one.** I recommend this most among the simplifications.
  - Today a single page load can cold-start 4 functions (`/config`, newsletters, questions, media-cookie), and each one rebuilds its own membership cache.
  - One function means one cold start and a shared cache.
  - It also makes decision #7's "≤10 free alarms" possible: per-function error alarms for 11 Lambdas can't fit.
  - Least privilege is weaker than it looks today: every function already gets read/write on the whole table ([api_stack.py:445](infra/opennewsletter/api_stack.py#L445)). Keep `image-process` and the two tick functions separate.
- **Home makes up to 20 sequential GSI queries per group.** `list_newsletters` runs one query per cycle ([handlers.rs:49](backend/crates/lambda-newsletters/src/handlers.rs#L49)), and Home uses the default limit of 20. This gets slower every month. Either compute draft/published counts only for `open` cycles, or have Home request `limit=3`.
- **`find_voting_cycle` reads every newsletter the group has ever had** ([newsletters.rs:158](backend/crates/persistence/src/newsletters.rs#L158)), on every candidate list, create and vote. The voting cycle is always the newest, so a backward query with `limit(2)` is enough.
- **Response autosave makes 5+N calls.** It does one `GetItem` per image ([responses handlers.rs:171](backend/crates/lambda-responses/src/handlers.rs#L171)), then re-reads the row after the transaction. Fetch the images with one batch read, run the independent reads with `try_join!`, and build the response locally instead of re-reading. Raising the 1.5 s debounce to about 3 s would roughly halve request volume; the `localStorage` mirror already covers crash recovery.
- **Push fan-out reads 2 rows per member, one member at a time** ([fanout.rs:235](backend/crates/lambda-push/src/fanout.rs#L235)). The notification pref and push subscriptions both live under `USER#u`, so one query per member covers both, and those queries can run concurrently.
- **Original images are kept forever after processing.** They're the storage cost that grows (about 120 MB per group per month of phone photos), and they still carry EXIF data, including GPS location. Delete them once processing succeeds, or expire them after 30 days. That saves money and cuts the location data you hold. It's your call whether you'd ever want to reprocess at new sizes.
- **Intelligent-Tiering on the processed and avatar buckets is wasted** ([media_persistent_stack.py:74](infra/opennewsletter/media_persistent_stack.py#L74)). Objects under 128 KB never move to a cheaper tier, but each lifecycle transition still costs money. Keep it only on the originals bucket, if you keep originals at all.
- **At scale, Cognito becomes the biggest bill**: free up to 10k monthly active users, then about $0.0055 each. Lambda, DynamoDB and CloudFront stay small. Put that in the [`infrastructure.md` §12](docs/design/infrastructure.md#12-cost-expectations) cost model.
- **Keep the 5-minute tick.** It costs essentially nothing and is simpler than scheduled one-off events.

## Security

- **Any Google or Facebook user can get a valid JWT.** Federated sign-in goes around `self_sign_up_enabled=False`, and `PreSignUp` is a pass-through. Data is safe because invites gate membership, but strangers' traffic does reach Lambda, which slightly bends the "idle backend" principle. HTTP APIs can't use WAF or per-client throttling. The current throttle of 25 requests/s applies to each route separately ([api_stack.py:489](infra/opennewsletter/api_stack.py#L489)), so the combined ceiling is very high. A merged Lambda with reserved concurrency (around 10–20) puts a hard cap on cost exposure.
- **The `/media-cookie` body returns the policy and signature**, so the `HttpOnly` cookie doesn't stop an XSS from reading them. In prod, where the cookie is set, leave them out of the body.
- **GitHub Pages can't send `frame-ancestors` or `X-Frame-Options`**, and a meta-tag CSP ignores `frame-ancestors`. Clickjacking risk is low but it can't be fixed on this host. Note it in [`frontend.md` §12](docs/design/frontend.md#12-build--deploy).
- **Facebook's `email` claim may not be verified.** When M13.5 adds email-bound invites, make sure they can only be redeemed with Google's `email_verified=true`, as the plan says.

## Simplification

- **`CONFIG_JSON` copies the `config.rs` defaults by hand and has already drifted** ([api_stack.py:47](infra/opennewsletter/api_stack.py#L47) still says `responseWindowDays: 4`, against the 7-day decision). Have `lambda-groups` build these defaults from `shared::config` and delete the environment variable.
- **PROGRESS.md is about 48k tokens of journal**, and CLAUDE.md tells every agent to start there, so each session pays that cost before doing any work. Keep the status table, blockers, decisions and handoff (about 150 lines), and move the "Mn detail" sections to `plans/archive/`.

## What's already strong

- Keys are centralised, and `key_timestamp` is enforced.
- Vote transactions return typed cancellation reasons.
- The fan-out marker semantics are at-most-once.
- The OpenAPI round-trip test.
- The own RFC 8291 push implementation, checked against the spec's published test vector.
- The expand → contract deploy rule.

**Suggested order:** do findings 1, 2 and the `CONFIG_JSON` fix in M13.5 (auto-publish needs finding 2 anyway), check the concurrency quota as part of B5, and decide on merging the Lambdas before M14, since it changes the alarm and infra layout. I can add these to PROGRESS.md as an M13.5 addendum if you'd like.