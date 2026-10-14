# Decisions

The owner's standing decisions that shape the design: what was decided, why, and which page documents the resulting behaviour. Code comments and the design docs cite the numbered ones by ID ("D8"), so the IDs are stable and never reused.

Each decision is already written into its design page as current behaviour. This page records the reasoning. For how and when each one landed (bugs found, test deltas), see [`plans/archive/progress-log.md`](../plans/archive/progress-log.md). Questions still waiting on the owner are listed under "Open owner questions" in [`plans/PROGRESS.md`](../plans/PROGRESS.md).

---

## Plan review, 2026-10-07 (D1–D12)

A full read-through of the design after the hardening milestone turned up twelve questions for the owner. All twelve were decided on 2026-10-07 and implemented by 2026-10-08.

<a id="d1"></a>
### D1 — Sessions persist in `localStorage`

**Decision.** The SPA keeps its Cognito tokens in `localStorage`, not `sessionStorage`, and logout revokes the refresh token.

**Why.** An installed PWA should stay signed in between launches. XSS could read tokens from either storage, so the exposure doesn't change; what changes is lifetime, since a stolen refresh token stays valid for up to 30 days unless revoked. The CSP is the main mitigation, and revoking at logout closes the window for a token stolen earlier.

**Where.** [`frontend.md` §6](design/frontend.md#6-auth-in-the-spa), [`auth.md` §8](design/auth.md#8-logout), threat model in [`auth.md` §12](design/auth.md#12-threat-model-considerations).

<a id="d2"></a>
### D2 — Non-empty drafts auto-publish at close

**Decision.** When a cycle closes, the tick publishes every non-empty draft before it flips the cycle to `published`. Non-empty means a text answer with a body or images, or a poll pick. Empty drafts stay drafts. A poll pick saves as a draft as soon as it's made. While an answer is still an unpublished draft, the response editor says it will be published at close. Deadline reminders count a non-empty draft as answered.

**Why.** A member who wrote an answer but never pressed Publish shouldn't be left out of the edition. Running the sweep before the condition-guarded status flip also makes the close crash-safe, because a tick that dies midway leaves the cycle `open` and the next tick finishes the job.

**Where.** [`lifecycle.md` §5.3](design/lifecycle.md#53-publishnl), [`notifications.md` §7.2](design/notifications.md#72-per-recipient-body-customization), [`frontend.md` §7.3](design/frontend.md#73-respondpage-ggncrespondq).

<a id="d3"></a>
### D3 — Seven-day response window by default

**Decision.** `responseWindowDays` defaults to 7 (`DEFAULT_RESPONSE_WINDOW_DAYS` in `backend/crates/shared/src/config.rs`), and all three reminder offsets (96, 48 and 24 hours before close) fire. Groups may still choose 1–28 days.

**Why.** The earlier default was 4 days, which left no room for the 96-hour reminder. With 7 days, all three reminders fire. Groups created before the change keep their stored value until an admin edits it.

**Where.** [`lifecycle.md` §4.2](design/lifecycle.md#42-default-schedule-per-group), [`notifications.md` §7.1](design/notifications.md#71-algorithm).

<a id="d4"></a>
### D4 — Promotion order: most votes, then oldest

**Decision.** At promotion, candidates are ranked by vote count (highest first), then by suggestion time (oldest first), then by question id. Candidates with no votes still fill any remaining slots. Promotion reads the whole candidate pool and sorts it in memory, and `GET …/candidate-questions?sort=top` uses the same comparator (`CandidateQuestion::promotion_order`).

**Why.** What the UI shows as "top" must match what actually gets promoted. Breaking ties by age rewards the members who suggested first. The `#VOTES` GSI orders ties differently, so promotion doesn't use it.

**Where.** [`lifecycle.md` §5.2](design/lifecycle.md#52-promote_and_opennl), [`api.md` §6.1](design/api.md#61-get-groupsgroupidcandidate-questions).

<a id="d5"></a>
### D5 — Production's first admin is an email-bound invite

**Decision.** `scripts/bootstrap_admin.py --env prod` reads `BOOTSTRAP_ADMIN_EMAIL`. In one conditional transaction it creates the first group, its first cycle and an admin invite that only that email, verified through Google, can redeem. The `admin-bootstrap` password client exists in dev only.

**Why.** Production has no password sign-in at all, so there's no password surface to attack, and the first admin signs in the same way every other member does.

**Where.** [`auth.md` §9](design/auth.md#9-bootstrap-admin--first-group), the bound-email check in [`auth.md` §4.2](design/auth.md#42-algorithm), and "Bootstrap the production admin" in [`operations/runbook.md`](operations/runbook.md).

<a id="d6"></a>
### D6 — One deploy workflow with a gated production promotion

**Decision.** A single `deploy.yml` deploys `main` to dev. A `promote-prod` job behind the `production` environment's required reviewer then deploys the backend, then the frontend, from the same commit. API changes are expand → contract. The service worker activates new builds promptly (`skipWaiting` plus `clients.claim`).

**Why.** One build is promoted unchanged, so what reached prod is exactly what ran in dev. An installed PWA keeps running its old code until its service worker updates, so the backend must keep serving the previous frontend release for a while. Prompt activation keeps that window short without reloading the page mid-session, which `autoUpdate` would do.

**Where.** [`testing-and-ci.md` §6.4](development/testing-and-ci.md#64-deployyml--dev-auto-deploy--gated-prod-promotion), the pre-flight checklist in [`coding-standards.md` §8](development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task), and [`.github/workflows/README.md`](../.github/workflows/README.md).

<a id="d7"></a>
### D7 — Run for about $1–2 a month

**Decision.** Use one Secrets Manager secret per environment (`opennewsletter/{env}`). Create alarms in prod only and stay within the 10 free alarm metrics. Keep one account-wide $5 budget in `CiStack`. Put DynamoDB on the AWS-owned key.

**Why.** As first built, the stacks would have cost about $8–9 a month, nearly all of it fixed charges that didn't depend on usage: four secrets per environment ($3.20) and about 30 alarm metrics per environment (about $5).

**Where.** [`infrastructure.md` §12](design/infrastructure.md#12-cost-expectations), the combined secret in [`operator-setup.md` §7](operations/operator-setup.md#7-combine-credentials-into-one-secrets-manager-secret-per-env).

<a id="d8"></a>
### D8 — Media deletes are real

**Decision.** Deleting an image or avatar removes the S3 objects, invalidates the exact CloudFront paths and deletes the row. Buckets aren't versioned. The UI warns that the delete is permanent.

**Why.** A member who deletes a photo expects it gone, not hidden. Invalidations use exact paths because CloudFront allows only 15 wildcard invalidations in progress per distribution, and a burst of deletes would hit that cap.

**Where.** [`media.md` §8.0](design/media.md#80-explicit-deletion), [`api.md` §9.4](design/api.md#94-delete-uploadsimageid).

<a id="d9"></a>
### D9 — Shared-device privacy is accepted as-is

**Decision.** On a shared device, some local state can outlive logout: the draft mirror and cached CDN images. This is accepted.

**Why.** The risk predates D1, and D1 doesn't make it materially worse. The service worker never caches API responses, so another user can't be served one person's data offline.

**Where.** [`auth.md` §12](design/auth.md#12-threat-model-considerations), [`frontend.md` §5.2](design/frontend.md#52-service-worker--caching-strategy).

<a id="d10"></a>
### D10 — Settings changes apply to the current cycle where possible

**Decision.** When an admin changes `responseWindowDays` or `timezone`, the current `voting` cycle is rescheduled whether or not it has candidates. An `open` cycle's close moves only if the new close is still in the future; otherwise the request fails with 422, field code `CLOSES_IN_PAST`. A cycle already past its close but not yet published is left alone. The settings form says the change "applies now".

**Why.** Admins expect a settings change to take effect now, not next month. Moving a close into the past would publish the cycle unexpectedly, and moving one that has already passed would reopen it to edits after some drafts have auto-published.

**Where.** [`lifecycle.md` §6](design/lifecycle.md#6-settings-changes-mid-cycle), [`api.md` §4.3](design/api.md#43-patch-groupsgroupid).

<a id="d11"></a>
### D11 — Flat comments, Web Push and the admin UI as built

**Decision.** Comments stay flat. Web Push and the admin UI stay as built. Threaded comments and comments or reactions on poll questions are not built.

**Why.** A small group's threads are short, and flat lists are simpler to read on a phone. The designs for the deferred features are kept in [`future-ideas.md`](future-ideas.md) so they can be picked up later.

**Where.** [`engagement.md` §1](design/engagement.md#1-comments), [`future-ideas.md` §1](future-ideas.md#1-threaded-nested-comments)–§2.

<a id="d12"></a>
### D12 — No per-PR CI stacks

**Decision.** End-to-end tests run against the long-lived `dev` stack. Ephemeral per-PR `ci-pr-*` stacks are not built.

**Why.** Per-PR isolation isn't worth the extra stacks, deploy time and clean-up machinery for a project this size. The design is kept in [`future-ideas.md` §3](future-ideas.md#3-ephemeral-per-pr-ci-stacks).

**Where.** [`environments.md` §11](development/environments.md#11-ephemeral-ci-stacks).

---

## Other standing decisions

<a id="apple-idp-dropped"></a>
### Sign in with Apple is not offered (2026-10-05)

Sign in with Apple requires a paid Apple Developer membership ($99/yr), so the user pool federates Google and Facebook only. Adding Apple back means restoring the IdP construct in `AuthStack`, its config fields and the operator-setup step. See [`infrastructure.md` §4.2](design/infrastructure.md#42-identity-providers-federated).

<a id="broken-drafts"></a>
### Broken drafts are retried, then skipped (2026-10-09)

One draft that can't be published must not hold the whole edition back. The close sweep carries on past a failing draft and keeps the cycle `open`. After `AUTO_PUBLISH_MAX_FAILED_SWEEPS` failing ticks, it publishes without that draft and emits the `AutoPublishAbandoned` metric, which the cycle-tick alarm counts. See [`lifecycle.md` §5.3](design/lifecycle.md#53-publishnl) and the operator procedure in [`operations/runbook.md`](operations/runbook.md).

<a id="field-errors"></a>
### Field errors name the field, and the reason is a code (2026-10-09)

A 422's `fieldErrors[]` entries carry one plain field name each, and the reason goes in `code`, a `FieldErrorCode` enum in `shared/openapi.yaml`. Clients switch on the code; they never parse a compound field name. See [`api.md` §1.1](design/api.md#11-error-format).

<a id="ci-hardening"></a>
### CI can't read user data, and every action is SHA-pinned (2026-10-11)

The pull-request `cdk diff` job runs the branch's own code, so its role assumes no CDK bootstrap role and can only read stack templates. Every `uses:` in the workflows is pinned to a full commit SHA, and Dependabot bumps them in one weekly PR. See [`testing-and-ci.md` §6](development/testing-and-ci.md#6-github-actions-workflows) and [`.github/workflows/README.md`](../.github/workflows/README.md).

<a id="credentialed-cdn-cors"></a>
### CDN images load with credentialed CORS (2026-10-11)

CloudFront sends credentialed CORS headers for the app's origins on `/img/*` and `/avatar/*`, and images load with `crossorigin="use-credentials"`. The service worker then sees real response statuses and can cache images for offline reading. A wrong CORS policy breaks every image, so the CORS check in [`smoke-test.md` §6](operations/smoke-test.md#6-draft-publish-comment-react-dev-on-prod-during-a-real-open-cycle) runs after deploys. See [`infrastructure.md` §5.2](design/infrastructure.md#52-cloudfront-distribution-mediapersistentstack), [`frontend.md` §5.2](design/frontend.md#52-service-worker--caching-strategy).

<a id="membership-cache"></a>
### Up to 60 seconds of stale access after removal is accepted (2026-10-11)

Each Lambda container caches a caller's memberships for `MEMBERSHIP_CACHE_TTL_SECONDS` (60 s). A member who is removed or demoted can keep their old access for up to that long. See [`auth.md` §11](design/auth.md#11-tenant-authorization-helper).

---

## Design review, 2026-10-08

A performance, cost and security review before the first production deploy. These decisions came out of it:

- **Whole-set queries paginate.** Every Query that needs the whole set goes through `persistence::query::query_all` and follows `LastEvaluatedKey`. Only cursor-paged routes and deliberately capped reads stop early. See [`data-model.md` §3](design/data-model.md#3-access-patterns--index-map).
- **Current-cycle lookups read three rows.** `find_voting_cycle` and `find_open_cycle` read only a group's three newest `NL#` rows. This relies on `yyyymm` cycle ids and on a group gaining one successor cycle at a time. See [`data-model.md` §3](design/data-model.md#3-access-patterns--index-map).
- **The published edition reads in four concurrent waves**, not one round of reads per answer, so its latency doesn't grow with group size. See [`api.md` §5.2](design/api.md#52-get-groupsgroupidnewsletterscycleid).
- **Group defaults come from code.** `GET /config` serves group defaults from `shared::config::default_group_settings`. The hand-copied `CONFIG_JSON` environment variable is gone, after it drifted once. See [`infrastructure.md` §6.4](design/infrastructure.md#64-group-default-config-json).
- **Processed-image buckets skip Intelligent-Tiering.** Their objects are below the 128 KB minimum, so transitions would cost money and save nothing. The originals buckets keep it. See [`infrastructure.md` §5.1](design/infrastructure.md#51-s3-buckets-mediapersistentstack).

---

## Recording a new decision

When the owner settles a design question:
1. Write the outcome into the owning design page as current behaviour.
2. Add an entry here with what was decided, why, and where. Give it the next `D` number if code or docs will cite it by ID; otherwise give it a descriptive `<a id>` anchor.
3. Record what landed, and when, in that milestone's entry in [`plans/archive/progress-log.md`](../plans/archive/progress-log.md).
