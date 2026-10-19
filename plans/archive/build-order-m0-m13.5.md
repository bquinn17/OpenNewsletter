# Build order, M0–M13.5 (archived)

The full milestone definitions (goal, deliverables, done-when gate) for M0–M13.5, moved verbatim out of [`plans/build-order.md`](../build-order.md) on 2026-10-11 once all of them were code-complete. The live file keeps a one-row summary and the done-when gate for each, because the deploy-shaped gates are still pending. What each milestone actually landed is in [`progress-log.md`](progress-log.md).

*Paths updated 2026-10-12: the numbered plan docs moved to [`docs/`](../../docs/README.md) under descriptive names (e.g. `plans/06-newsletter-lifecycle.md` is now [`docs/design/lifecycle.md`](../../docs/design/lifecycle.md)), and the links below were rewritten to match. Section numbers are unchanged, so every § reference still points at the same content. The text is otherwise as written.*

---

## Milestone 0 — Repo skeleton (½ day)

**Goal**: Empty but well-shaped repo. Nothing runs, but every directory and config file exists so subsequent milestones can drop code into the right place.

**Deliverables**:
- Repo layout per [`overview.md` §4](../../docs/overview.md#4-repo-layout) created with empty placeholder files (`README.md` in each dir explaining its purpose).
- Top-level `README.md` describing how to run dev locally (filling in detail later).
- `CLAUDE.md` summarizing repo conventions for AI assistants (use the same language as [`overview.md`](../../docs/overview.md)).
- `.gitignore` covering `node_modules/`, `target/`, `cdk.out/`, `.env*`, `*.pem`, `dist/`.
- `.editorconfig` (2-space JS/Py/Yaml, 4-space Rust, LF line endings).
- `.nvmrc` (20 at the time; now 24), `rust-toolchain.toml` (1.79+ stable at the time; now pinned to 1.99.0), `infra/runtime.txt` (3.12).

**Done when**: `tree -L 2 .` matches the layout in [`overview.md` §4](../../docs/overview.md#4-repo-layout).

---

## Milestone 1 — Operator prerequisites (½ day, parallelizable with M0)

Read: [`auth.md` §10](../../docs/design/auth.md#10-identity-provider-setup-design-relevant-facts).

A human operator (the user) completes the out-of-band setup:
- [ ] Register Google, Facebook OAuth apps with the Cognito redirect URI (need Cognito domain set up first, OR placeholder updated after Milestone 3). No Apple IdP — dropped, see `plans/PROGRESS.md` decisions (M1); Apple Developer membership is $99/yr.
- [ ] Generate VAPID keys (`scripts/generate_vapid_keys.py`) and store in Secrets Manager.
- [ ] Configure DNS for the chosen domain (`opennewsletter.example.com`, `api.opennewsletter.example.com`). If using Route53, note the hosted zone ID.

Subagents cannot execute this milestone — flag for the operator. Subagents proceed using placeholder ARNs for any blocking steps.

**Done when**: All ARNs/IDs are recorded in `infra/opennewsletter/config.py` (or a sibling `.env.local` for dev).

---

## Milestone 2 — Backend foundations: domain + persistence (1.5 days)

Read: [`data-model.md`](../../docs/design/data-model.md), [`overview.md` §6](../../docs/overview.md#6-cross-cutting-conventions).

**Deliverables**:
- `backend/` Cargo workspace with empty crates per [`overview.md` §4](../../docs/overview.md#4-repo-layout).
- `domain/` crate: every entity struct (`User`, `Group`, `GroupMembership`, `Invite`, `Newsletter`, `CandidateQuestion`, `LockedQuestion`, `Response`, `ImageMedia`, `Comment`, `Reaction`, `PushSubscription`, `NotificationPref`) as serde-friendly Rust types. `Role` and `Status`-style enums. `ApiError` enum with HTTP status mappings.
- `persistence/keys.rs`: every key-builder function from [`data-model.md`](../../docs/design/data-model.md). Unit tests for each.
- `persistence/`: `Repo` struct holding `aws_sdk_dynamodb::Client` and table name. One file per entity-family (`users.rs`, `groups.rs`, `invites.rs`, `newsletters.rs`, `questions.rs`, `responses.rs`, `engagement.rs`, `media.rs`, `push.rs`).
- For each access pattern AP1–AP21 in [`data-model.md` §3](../../docs/design/data-model.md#3-access-patterns--index-map): implement the corresponding query/get function. For each transaction in §4: implement using `TransactWriteItems`.
- `persistence/test_factories.rs` (cfg-test only).
- Integration tests against DDB-local using `testcontainers`. ≥1 test per access pattern, ≥1 per transaction.

**Done when**: `cargo test -p persistence` is green.

---

## Milestone 3 — Infrastructure baseline (1 day)

Read: [`infrastructure.md`](../../docs/design/infrastructure.md).

**Deliverables**:
- `infra/` CDK Python project with `cdk.json`, `requirements.txt`, `app.py`.
- `infra/opennewsletter/config.py` with `EnvConfig` dataclass and dev/prod loaders.
- `DataStack` — DynamoDB table + GSIs.
- `AuthStack` — Cognito user pool + 2 IdPs (Google, Facebook) + app clients + hosted UI domain (use placeholder secret ARNs from M1; refresh once the operator finalizes).
- `MediaPersistentStack` — S3 buckets for media originals and processed output.
- `MediaPipelineStack` — `lambda-image-process` (the Lambda code can be a stub at this point; replaced in M8) and its S3 event subscription.
- `FrontendStack` — ACM cert and DNS records.
- `MonitoringStack` skeleton (alarms wired empty for now).
- `infra/tests/test_stacks.py` per [`testing-and-ci.md` §3](../../docs/development/testing-and-ci.md#3-cdk-tests).

**Done when**: `cdk synth --context env=dev` succeeds; `pytest infra/tests/` is green; a manual `cdk deploy --context env=dev DataStack AuthStack MediaPersistentStack MediaPipelineStack FrontendStack` produces working resources (verifiable via console).

---

## Milestone 3.5 — Dev environment online (½ day)

Read: [`environments.md`](../../docs/development/environments.md).

**Goal**: Make the `dev` AWS environment usable for daily work. Every milestone after this is testable end-to-end against real AWS from the moment its code lands.

**Deliverables**:
- `Makefile` targets: `make deploy-dev`, `make seed`, `make redeploy-volatile`, `make reset-all`, `make deploy-lambda LAMBDA=...`, `make fe`.
- `scripts/seed_dev_data.py` — idempotent + destructive: scans + batch-deletes the DynamoDB table, empties S3 prefixes, writes the small fixture (one group, bootstrap admin as sole member, one `voting` cycle). Does NOT touch Cognito. Prints sign-in credentials. See [`environments.md` §4](../../docs/development/environments.md#4-reset-granularities).
- `scripts/write_frontend_env.py` — reads `cdk.out/dev-outputs.json`, writes `frontend/.env.dev` with API/media/Cognito values per [`environments.md` §7](../../docs/development/environments.md#7-frontend-pointing-at-dev).
- Dev-environment removal-policy overrides applied across all CDK stacks per [`infrastructure.md` §10.5](../../docs/design/infrastructure.md#105-dev-environment-resource-policy-overrides).
- `MediaStack` split into `MediaPersistentStack` + `MediaPipelineStack` per [`infrastructure.md` §5](../../docs/design/infrastructure.md#5-mediapersistentstack-and-mediapipelinestack).
- `POST /admin/dev/tick/{cycle|notify}` admin route stubs in `lambda-cycle-tick` / `lambda-notify-tick` (gated on `ENV=dev`) — full implementations landed in M5 (cycle) and M11 (notify); the routes exist now so manual testing has a fast-forward path.
- AWS Budgets alarm at $10/mo wired to `config.alarm_email`.

**Done when**: `make deploy-dev && make seed && make fe`, sign in via the `/admin/bootstrap-login` route as the seeded admin, and see the seeded group's home page. `make redeploy-volatile` succeeds end-to-end (proves the dev removal-policy overrides are correct).

---

## Milestone 4 — Auth + bootstrap (1 day)

Read: [`auth.md`](../../docs/design/auth.md).

**Deliverables**:
- `lambda-invites` crate compiled to `bootstrap` binary; supports both API routes (`POST /admin/invites`, `POST /invites/redeem`, `POST /admin/invites/{code}/revoke`, `GET /admin/groups/{g}/invites`) and a `PreSignUp` Cognito trigger (pass-through per §3.3).
- `lambda-groups` crate with `GET /me`, `PATCH /me`, `GET /config`, `GET /groups`, `GET /groups/{g}`, `PATCH /groups/{g}`, member-mgmt routes.
- **Create `ApiStack`** ([`infrastructure.md` §6](../../docs/design/infrastructure.md#6-apistack) — HTTP API, CORS, JWT authorizer, throttling, access logs; this stack does not exist before M4) and wire both Lambdas' routes into it. The dev-only tick routes stubbed in M3.5 get their API Gateway wiring in M5 when the tick Lambdas land.
- `scripts/bootstrap_admin.py` per [`auth.md` §9.1](../../docs/design/auth.md#91-scriptsbootstrap_adminpy--dev).
- The `admin-bootstrap` Cognito app client.

**Done when**: an operator can `python scripts/bootstrap_admin.py --env dev --admin-email me@example.com --group-name "Test"` and produce a working group + admin user in DynamoDB, AND that user can hit `GET /me` with a Cognito-issued token via curl.

---

## Milestone 5 — Lifecycle engine + cycle CRUD (1.5 days)

Read: [`lifecycle.md`](../../docs/design/lifecycle.md), [`api.md` §5](../../docs/design/api.md#5-newsletter-routes).

**Deliverables**:
- `lambda-newsletters` with `GET /groups/{g}/newsletters` and `GET /groups/{g}/newsletters/{c}` (handles all four status branches).
- `lambda-questions` with candidate question CRUD per [`api.md` §6](../../docs/design/api.md#6-candidate-question-routes-voting-pool). No admin curate/promote surface — voting is the sole source of truth (see [`api.md` §6.5](../../docs/design/api.md#65-admin-moderation-routes) and [`lifecycle.md` §7](../../docs/design/lifecycle.md#7-manual-admin-overrides)). The only admin mutation is `DELETE /admin/groups/{g}/candidate-questions/{q}` for abusive content.
- **Create `NotificationsStack`** ([`infrastructure.md` §7](../../docs/design/infrastructure.md#7-notificationsstack) — EventBridge schedules + VAPID secret reference; this stack does not exist before M5) with `lambda-cycle-tick` per [`lifecycle.md` §5](../../docs/design/lifecycle.md#5-lambda-cycle-tick) on the 5-minute schedule. `lambda-notify-tick` joins the stack in M11. Wire the dev-only `POST /admin/dev/tick/*` routes into `ApiStack` now.
- The "create-next-voting-cycle" logic that runs on group creation (in `bootstrap_admin.py` or in the group-creation transaction in `lambda-groups`).
- Lifecycle integration test suite per [`lifecycle.md` §11](../../docs/design/lifecycle.md#11-tests).
- **Establish `shared/openapi.yaml`** — see "OpenAPI contract" below.

### OpenAPI contract (deferred from M4, owned here)

`shared/openapi.yaml` is named as the contract's source of truth by [`coding-standards.md` §1.12](../../docs/development/coding-standards.md#112-dont-write-what-you-can-generate), [`api.md` §12](../../docs/design/api.md#12-openapi-generation), [`overview.md` §4](../../docs/overview.md#4-repo-layout), and the §8 pre-flight checklist — but it was never created, so M4's routes and DTOs were written from [`api.md`](../../docs/design/api.md) prose directly. M5 is where that debt gets paid, before the route count grows further and the backlog becomes a slog.

Deliverables:
- **`shared/openapi.yaml`** — OpenAPI 3.1, hand-maintained, covering every route shipped through M5: M4's 13 ([`api.md` §2](../../docs/design/api.md#2-bootstrap--metadata-routes)–§4) plus M5's newsletter and candidate-question routes (§5–§6). Include the `gradient` and `avatarColor` enums, which §2.3/§4.3 already designate this file as the canonical home for.
- **Consolidate the Rust wire types into `backend/crates/domain/src/api.rs`.** M4 put them in per-crate `dto.rs` files (`lambda-groups/src/dto.rs`, `lambda-invites/src/dto.rs`), which works but gives the contract test no single target. Move them, re-export per crate, and keep entities (`domain/entities.rs`) separate from wire shapes — they are allowed to differ, and already do (DynamoDB attributes are snake_case, the HTTP contract is camelCase).
- **`scripts/codegen_types.sh`** — runs `openapi-typescript` into `frontend/src/types/api.ts`. Referenced by `scripts/README.md` and [`overview.md` §4](../../docs/overview.md#4-repo-layout); also missing. The frontend consumes its output starting in M7, but the script belongs with the YAML.
- **The contract test from [`testing-and-ci.md` §2.3](../../docs/development/testing-and-ci.md#23-contract-test)** — loads the YAML and round-trips each route's example payloads through the corresponding `domain/api.rs` struct. This is what makes the YAML authoritative rather than decorative.

**Standing rule from M5 onward**: a milestone that adds or changes a route updates `shared/openapi.yaml` in the same PR. The §8 pre-flight already says this; it only becomes enforceable once the file and its contract test exist.

**Done when**: a manual flow produces:
1. A group is created with a `voting` cycle.
2. Members suggest candidate questions (via curl).
3. Tick is invoked manually with a fast-forwarded `responseOpenAt` → cycle transitions to `open`.
4. Tick is invoked again with fast-forwarded `responseCloseAt` → cycle transitions to `published`.
5. The next `voting` cycle was created automatically.

AND `shared/openapi.yaml` describes every route through M5, with the §2.3 contract test green in CI.

---

## Milestone 6 — Responses + drafts (1 day)

Read: [`api.md` §7](../../docs/design/api.md#7-response-routes-drafts--publish), [`frontend.md` §8](../../docs/design/frontend.md#8-drafts--detailed-flow).

**Deliverables**:
- `lambda-responses` with all routes from §7.
- Last-write-wins draft saves — the response row is overwritten unconditionally, with no version tokens or conflict errors. Only the cycle-open check applies. ([`api.md` §7.3](../../docs/design/api.md#73-put-groupsgroupidnewsletterscycleidquestionsquestionidmy-response), [`data-model.md` §5](../../docs/design/data-model.md#5-concurrency-for-drafts)).
- Validation: cycle status, image-id ownership / `ready` status / `purpose=response`, body length, image count.
- Integration tests for: save draft, interleaved saves resolve last-write-wins, publish, attempt to publish past deadline.

**Done when**: `cargo test -p lambda-responses` green AND a curl-based dance (save → save → publish) produces correct DDB state.

---

## Milestone 7 — Frontend skeleton + auth (1.5 days)

Read: [`frontend.md`](../../docs/design/frontend.md), [`auth.md`](../../docs/design/auth.md).

**Deliverables**:
- Vite project initialized, Tailwind configured, React Router set up with all routes from [`frontend.md` §3](../../docs/design/frontend.md#3-routing) as stubs.
- `auth/` module: OIDC client, callback page, token storage, `AuthProvider`, `useAuth`.
- `api/client.ts` with auth + correlation header.
- `types/api.ts` from `openapi-typescript` against `shared/openapi.yaml` (via `scripts/codegen_types.sh`; decided in M7 — [`coding-standards.md` §3.3](../../docs/development/coding-standards.md#33-types) already named this path).
- `pages/HomePage` rendering memberships.
- `pages/JoinPage` implementing the invite-redemption flow.
- `pages/SettingsPage` rendering basic profile.
- `AppShell`, `BottomNav`, `GroupSwitcher`.

**Done when**: Running `npm run dev` and pointing at the dev API:
- I can sign in via Google.
- An invited user can redeem and see their group.
- Logout works.

---

## Milestone 8 — Media pipeline (1.5 days)

Read: [`media.md`](../../docs/design/media.md).

**Deliverables**:
- `lambda-media` with all eight routes (`POST /uploads`, `GET`/`PATCH`/`DELETE /uploads/{id}`, `POST /uploads/{id}/complete`, and the three `/avatars` routes; [`api.md` §9](../../docs/design/api.md#9-media-routes)).
- `lambda-image-process` real implementation (replacing the M3 stub) — reads originals, generates display + thumb WebP/GIF, writes to processed bucket, marks DDB row ready.
- `frontend/src/components/responses/ImageUploader.tsx` with the drag-drop, presign, PUT, poll loop.
- Image-pipeline integration tests per [`media.md` §10](../../docs/design/media.md#10-tests).

**Done when**: I can upload a 5MB JPEG, see thumbnail render in the editor, and reload the page to see the image rendered via a presigned S3 URL.

---

## Milestone 9 — Newsletter UI (2 days)

Read: [`frontend.md` §7](../../docs/design/frontend.md#7-pages--behavior-detail), [`lifecycle.md`](../../docs/design/lifecycle.md).

**Deliverables**:
- `pages/CandidatesPage` with list + voting + suggestion link.
- `pages/SuggestPage` with the form (text + poll).
- `pages/NewsletterPage` rendering all three of `voting`/`open`/`published` layouts.
- `pages/RespondPage` with editor + autosave + Publish.
- `components/newsletter/PollWidget`, `AnswerCard`, `ImageGallery`, `QuestionPrompt`.
- Group-TZ-aware date display.

**Done when**: A simulated month round-trip (using fast-forward time) works end-to-end in the UI.

---

## Milestone 10 — Engagement (1 day)

Read: [`engagement.md`](../../docs/design/engagement.md), [`api.md` §8](../../docs/design/api.md#8-engagement-routes-comments--reactions).

**Deliverables**:
- `lambda-engagement` for comments + reactions.
- `frontend` `CommentList`, `ReactionBar` components.
- Markdown sanitizer applied consistently.
- Engagement integration tests per [`engagement.md` §8](../../docs/design/engagement.md#8-tests).

**Done when**: Two users can comment on each other's published answers and react with arbitrary emoji.

---

## Milestone 11 — Notifications (1 day)

Read: [`notifications.md`](../../docs/design/notifications.md).

**Deliverables**:
- `lambda-push` for subscribe/unsubscribe/list/test/preferences + cycle-open / publication / deadline fan-out functions.
- `lambda-notify-tick` and EventBridge schedule.
- Frontend service worker push handler + Settings UI toggle + per-group preferences.
- The `ensurePushSubscription` flow.

**Done when**: I can opt in on Settings, manually invoke the cycle-tick or notify-tick, and receive a real OS notification.

---

## Milestone 12 — Admin UI (½ day)

Read: [`frontend.md` §7.7](../../docs/design/frontend.md#77-groupadminpage), [`api.md` §3](../../docs/design/api.md#3-invite--membership-routes), §6.5.

**Deliverables**:
- `pages/GroupAdminPage` with the three tabs (Members, Invites, Settings). No Curate tab — admins do not override voting.

**Done when**: I can create an invite, copy the URL, share, and see the new member appear.

---

## Milestone 13 — Hardening (1 day)

**Deliverables**:
- All tests in [`testing-and-ci.md` §2](../../docs/development/testing-and-ci.md#2-rust-backend-tests)–§4 implemented (gaps from prior milestones filled).
- All six GitHub Actions workflows green on `main` (`frontend-ci`, `frontend-deploy`, `backend-ci`, `backend-deploy`, `infra-ci`, `e2e`; [`testing-and-ci.md` §6](../../docs/development/testing-and-ci.md#6-github-actions-workflows)). *Superseded in M13.5: `deploy.yml` replaced `frontend-deploy` and `backend-deploy`, leaving five workflows ([`testing-and-ci.md` §6.4](../../docs/development/testing-and-ci.md#64-deployyml--dev-auto-deploy--gated-prod-promotion)).*
- Monitoring alarms wired to a working SNS topic + email.
- CSP set on the frontend; verified by a Playwright test.
- Manual smoke runbook ([`smoke-test.md`](../../docs/operations/smoke-test.md)) executed and observed clean.
- 5-min E2E run against the deployed dev stack.

**Done when**: A new contributor can clone, follow `README.md`, run dev locally, and contribute a PR with passing CI.

---

## Milestone 13.5 — Plan-review follow-ups (2 days)

Read: the "Plan review (2026-10-07)" section of `PROGRESS.md`. Then read each plan section named below; every one is marked "decided 2026-10-07".

**Goal**: implement the owner's 2026-10-07 decisions before the first prod deploy. Several of them change prod-only setup (secrets, alarms, budget, bootstrap, deploy gate), which is far cheaper to do before prod exists.

**Deliverables** (split by directory for parallel agents, per `CLAUDE.md`):
- **Auth (D1, D5)**:
  - Frontend:
    - tokens move to `localStorage`
    - logout revokes the refresh token via `/oauth2/revoke` ([`frontend.md` §6](../../docs/design/frontend.md#6-auth-in-the-spa), [`auth.md` §8](../../docs/design/auth.md#8-logout))
  - Backend:
    - `Invite.bound_email` and optional `boundEmail` on `POST /admin/invites`
    - `POST /invites/redeem` checks `email` and `email_verified`; a mismatch is 403 `INVITE_EMAIL_MISMATCH` ([`data-model.md` §2.4](../../docs/design/data-model.md#24-invite), [`api.md` §3](../../docs/design/api.md#3-invite--membership-routes), [`auth.md` §4](../../docs/design/auth.md#4-invite-redemption-post-invitesredeem))
  - Infra:
    - Google `email_verified` attribute mapping
    - the `admin-bootstrap` client is dev-only, and the prod authorizer audience is the frontend client only
  - Script: `bootstrap_admin.py` prod mode (`BOOTSTRAP_ADMIN_EMAIL`, no Cognito user, an email-bound admin invite; [`auth.md` §9](../../docs/design/auth.md#9-bootstrap-admin--first-group))
- **Auto-publish (D2)**:
  - `lambda-cycle-tick` `publish(nl)` auto-publishes non-empty drafts before the status flip ([`lifecycle.md` §5.3](../../docs/design/lifecycle.md#53-publishnl)).
  - Reminder and publication counts include non-empty drafts ([`notifications.md` §5.1](../../docs/design/notifications.md#51-per-kind-copy), §7.2).
  - Frontend: the RespondPage auto-publish warning and DraftStatusBadge copy; a poll pick saves immediately as a draft ([`frontend.md` §7.3](../../docs/design/frontend.md#73-respondpage-ggncrespondq)).
- **Window and promotion (D3, D4)**:
  - `DEFAULT_RESPONSE_WINDOW_DAYS = 7`
  - promotion and `sort=top` order by votes desc, then oldest first ([`lifecycle.md` §5.2](../../docs/design/lifecycle.md#52-promote_and_opennl), [`api.md` §6.1](../../docs/design/api.md#61-get-groupsgroupidcandidate-questions))
- **Settings apply now (D10)**: `PATCH /groups/{g}` reschedules the current `voting` cycle regardless of candidates, and moves an `open` cycle's close when it's still in the future, else 422 ([`lifecycle.md` §6](../../docs/design/lifecycle.md#6-settings-changes-mid-cycle)). Update the admin Settings copy.
- **Real media deletion (D8)**:
  - Backend: a shared delete helper (S3 objects, then row) used by `DELETE /uploads`, comment image release, and avatar delete/replace ([`api.md` §9.4](../../docs/design/api.md#94-delete-uploadsimageid)/§9.7, [`media.md` §8](../../docs/design/media.md#8-cleanup-paths)).
  - Infra:
    - IAM for the media, engagement and groups Lambdas
    - no bucket versioning
  - Frontend: permanent-delete `ConfirmDialog`s ([`frontend.md` §7.3](../../docs/design/frontend.md#73-respondpage-ggncrespondq)/§7.6, [`engagement.md` §1](../../docs/design/engagement.md#1-comments)).
- **Cost (D7)**:
  - One Secrets Manager secret per env (`app_secret_arn`); migrate the four dev secrets, then delete them.
  - Prod-only alarms, at most 10 alarm metrics.
  - One account-wide $5 budget in `CiStack`.
  - DynamoDB on an AWS-owned key ([`infrastructure.md` §2](../../docs/design/infrastructure.md#2-configpy)/§3.1/§9/§12).
- **Deploy gate (D6)**:
  - One `deploy.yml`: `main` → dev automatically, then a `promote-prod` job behind the `production` environment that deploys the backend, then the frontend, from the same SHA ([`testing-and-ci.md` §6](../../docs/development/testing-and-ci.md#6-github-actions-workflows)).
  - The service worker activates updates promptly.
  - The expand → contract API rule goes in [`coding-standards.md` §8](../../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task).
- OpenAPI, `ROUTE_TABLE` and `frontend/src/types/api.ts` updated for `boundEmail` and the new error code.

**Done when**: all checks are green (fmt, clippy, `cargo test --workspace`, `pytest infra/tests`, ruff, mypy, `cdk synth` dev + prod, and the frontend checks), plus integration tests for:
- auto-publish (including a crash-and-rerun)
- the tie order
- bound-invite accept and reject
- an open-cycle window change
- media deletion removing the objects and the row

The deploy-shaped checks join the M14 gate.

---

## Estimated total effort

| Milestone | Days |
|---|---|
| 0 Skeleton | 0.5 |
| 1 Prerequisites | 0.5 (operator, parallel) |
| 2 Backend foundations | 1.5 |
| 3 Infra baseline | 1.0 |
| 4 Auth + bootstrap | 1.0 |
| 5 Lifecycle | 1.5 |
| 6 Responses | 1.0 |
| 7 Frontend skeleton | 1.5 |
| 8 Media | 1.5 |
| 9 Newsletter UI | 2.0 |
| 10 Engagement | 1.0 |
| 11 Notifications | 1.0 |
| 12 Admin UI | 0.5 |
| 13 Hardening | 1.0 |
| 13.5 Plan-review follow-ups | 2.0 |
| 14 Prod deploy | 0.5 |
| **Total** | **~18 days** |

This is "focused engineer time," not calendar time.
