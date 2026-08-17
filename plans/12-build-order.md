# 12 — Build Order

This is the recommended sequence to take OpenNewsletter from empty repo to a deployable v1. Each milestone names the plan documents to consult, the deliverables, and an explicit "done when" gate. A subagent should be able to pick up at any milestone and execute against it without re-reading earlier ones (although the Overview is always assumed).

**Before starting any milestone, read these two files in full:**
1. [`00-overview.md`](00-overview.md) — system map, glossary, repo layout
2. [`coding-standards.md`](coding-standards.md) — style, linting, testing, error handling, naming, comments, commit/PR conventions

These are non-negotiable preamble for every milestone below. The pre-flight checklist at the bottom of `coding-standards.md` (§8) applies before opening any PR.

---

## Milestone 0 — Repo skeleton (½ day)

**Goal**: Empty but well-shaped repo. Nothing runs, but every directory and config file exists so subsequent milestones can drop code into the right place.

**Deliverables**:
- Repo layout per `00-overview.md` §4 created with empty placeholder files (`README.md` in each dir explaining its purpose).
- Top-level `README.md` describing how to run dev locally (filling in detail later).
- `CLAUDE.md` summarizing repo conventions for AI assistants (use the same language as `00-overview.md`).
- `.gitignore` covering `node_modules/`, `target/`, `cdk.out/`, `.env*`, `*.pem`, `dist/`.
- `.editorconfig` (2-space JS/Py/Yaml, 4-space Rust, LF line endings).
- `.nvmrc` (20), `rust-toolchain.toml` (1.79+ stable), `infra/runtime.txt` (3.12).

**Done when**: `tree -L 2 .` matches the layout in `00-overview.md` §4.

---

## Milestone 1 — Operator prerequisites (½ day, parallelizable with M0)

Read: `05-auth-flow.md` §10.

A human operator (the user) completes the out-of-band setup:
- [ ] Register Google, Apple, Facebook OAuth apps with the Cognito redirect URI (need Cognito domain set up first, OR placeholder updated after Milestone 3).
- [ ] Generate VAPID keys (`scripts/generate_vapid_keys.py`) and store in Secrets Manager.
- [ ] Generate CloudFront signing keypair; store private in Secrets Manager, commit public key file to `infra/keys/cf-signing.pub.pem`.
- [ ] Configure DNS for the chosen domain (`opennewsletter.example.com`, `cdn.opennewsletter.example.com`, `api.opennewsletter.example.com`). If using Route53, note the hosted zone ID.

Subagents cannot execute this milestone — flag for the operator. Subagents proceed using placeholder ARNs for any blocking steps.

**Done when**: All ARNs/IDs are recorded in `infra/opennewsletter/config.py` (or a sibling `.env.local` for dev).

---

## Milestone 2 — Backend foundations: domain + persistence (1.5 days)

Read: `02-data-model-dynamodb.md`, `00-overview.md` §6.

**Deliverables**:
- `backend/` Cargo workspace with empty crates per `00-overview.md` §4.
- `domain/` crate: every entity struct (`User`, `Group`, `GroupMembership`, `Invite`, `Newsletter`, `CandidateQuestion`, `LockedQuestion`, `Response`, `ImageMedia`, `Comment`, `Reaction`, `PushSubscription`, `NotificationPref`) as serde-friendly Rust types. `Role` and `Status`-style enums. `ApiError` enum with HTTP status mappings.
- `persistence/keys.rs`: every key-builder function from `02-data-model-dynamodb.md`. Unit tests for each.
- `persistence/`: `Repo` struct holding `aws_sdk_dynamodb::Client` and table name. One file per entity-family (`users.rs`, `groups.rs`, `invites.rs`, `newsletters.rs`, `questions.rs`, `responses.rs`, `engagement.rs`, `media.rs`, `push.rs`).
- For each access pattern AP1–AP21 in `02-data-model-dynamodb.md` §3: implement the corresponding query/get function. For each transaction in §4: implement using `TransactWriteItems`.
- `persistence/test_factories.rs` (cfg-test only).
- Integration tests against DDB-local using `testcontainers`. ≥1 test per access pattern, ≥1 per transaction.

**Done when**: `cargo test -p persistence` is green.

---

## Milestone 3 — Infrastructure baseline (1 day)

Read: `01-infrastructure-cdk.md`.

**Deliverables**:
- `infra/` CDK Python project with `cdk.json`, `requirements.txt`, `app.py`.
- `infra/opennewsletter/config.py` with `EnvConfig` dataclass and dev/prod loaders.
- `DataStack` — DynamoDB table + GSIs.
- `AuthStack` — Cognito user pool + 3 IdPs + app client + hosted UI domain (use placeholder secret ARNs from M1; refresh once the operator finalizes).
- `MediaPersistentStack` — S3 buckets + CloudFront + KeyGroup.
- `MediaPipelineStack` — `lambda-image-process` (the Lambda code can be a stub at this point; replaced in M8) and its S3 event subscription.
- `FrontendStack` — ACM cert and DNS records.
- `MonitoringStack` skeleton (alarms wired empty for now).
- `infra/tests/test_stacks.py` per `11-testing-ci-cd.md` §3.

**Done when**: `cdk synth --context env=dev` succeeds; `pytest infra/tests/` is green; a manual `cdk deploy --context env=dev DataStack AuthStack MediaPersistentStack MediaPipelineStack FrontendStack` produces working resources (verifiable via console).

---

## Milestone 3.5 — Dev environment online (½ day)

Read: [`13-dev-environments.md`](13-dev-environments.md).

**Goal**: Make the `dev` AWS environment usable for daily work. Every milestone after this is testable end-to-end against real AWS from the moment its code lands.

**Deliverables**:
- `Makefile` targets: `make deploy-dev`, `make seed`, `make redeploy-volatile`, `make reset-all`, `make deploy-lambda LAMBDA=...`, `make fe`.
- `scripts/seed_dev_data.py` — idempotent + destructive: scans + batch-deletes the DynamoDB table, empties S3 prefixes, writes the small fixture (one group, bootstrap admin as sole member, one `voting` cycle). Does NOT touch Cognito. Prints sign-in credentials. See [`13-dev-environments.md` §4](13-dev-environments.md).
- `scripts/write_frontend_env.py` — reads `cdk.out/dev-outputs.json`, writes `frontend/.env.dev` with API/CDN/Cognito values per [`13-dev-environments.md` §7](13-dev-environments.md).
- Dev-environment removal-policy overrides applied across all CDK stacks per [`01-infrastructure-cdk.md` §10.5](01-infrastructure-cdk.md).
- `MediaStack` split into `MediaPersistentStack` + `MediaPipelineStack` per [`01-infrastructure-cdk.md` §5](01-infrastructure-cdk.md).
- `POST /admin/dev/tick/{cycle|notify}` admin route stubs in `lambda-cycle-tick` / `lambda-notify-tick` (gated on `ENV=dev`) — full implementations land in M5/M9; the routes exist now so manual testing has a fast-forward path.
- AWS Budgets alarm at $10/mo wired to `config.alarm_email`.

**Done when**: `make deploy-dev && make seed && make fe`, sign in via the `/admin/bootstrap-login` route as the seeded admin, and see the seeded group's home page. `make redeploy-volatile` succeeds end-to-end (proves the dev removal-policy overrides are correct).

---

## Milestone 4 — Auth + bootstrap (1 day)

Read: `05-auth-flow.md`.

**Deliverables**:
- `lambda-invites` crate compiled to `bootstrap` binary; supports both API routes (`POST /admin/invites`, `POST /invites/redeem`, `POST /admin/invites/{code}/revoke`, `GET /admin/groups/{g}/invites`) and a `PreSignUp` Cognito trigger (pass-through per §3.3).
- `lambda-groups` crate with `GET /me`, `PATCH /me`, `GET /config`, `GET /groups`, `GET /groups/{g}`, `PATCH /groups/{g}`, member-mgmt routes.
- **Create `ApiStack`** (`01-infrastructure-cdk.md` §6 — HTTP API, CORS, JWT authorizer, throttling, access logs; this stack does not exist before M4) and wire both Lambdas' routes into it. The dev-only tick routes stubbed in M3.5 get their API Gateway wiring in M5 when the tick Lambdas land.
- `scripts/bootstrap_admin.py` per `05-auth-flow.md` §9.1.
- The `admin-bootstrap` Cognito app client.

**Done when**: an operator can `python scripts/bootstrap_admin.py --env dev --admin-email me@example.com --group-name "Test"` and produce a working group + admin user in DynamoDB, AND that user can hit `GET /me` with a Cognito-issued token via curl.

---

## Milestone 5 — Lifecycle engine + cycle CRUD (1.5 days)

Read: `06-newsletter-lifecycle.md`, `03-api-contract.md` §5.

**Deliverables**:
- `lambda-newsletters` with `GET /groups/{g}/newsletters` and `GET /groups/{g}/newsletters/{c}` (handles all four status branches).
- `lambda-questions` with candidate question CRUD per `03-api-contract.md` §6. No admin curate/promote surface — voting is the sole source of truth (see `03-api-contract.md` §6.5 and `06-newsletter-lifecycle.md` §7). The only admin mutation is `DELETE /admin/groups/{g}/candidate-questions/{q}` for abusive content.
- **Create `NotificationsStack`** (`01-infrastructure-cdk.md` §7 — EventBridge schedules + VAPID secret reference; this stack does not exist before M5) with `lambda-cycle-tick` per `06-newsletter-lifecycle.md` §5 on the 5-minute schedule. `lambda-notify-tick` joins the stack in M11. Wire the dev-only `POST /admin/dev/tick/*` routes into `ApiStack` now.
- The "create-next-voting-cycle" logic that runs on group creation (in `bootstrap_admin.py` or in the group-creation transaction in `lambda-groups`).
- Lifecycle integration test suite per `06-newsletter-lifecycle.md` §11.
- **Establish `shared/openapi.yaml`** — see "OpenAPI contract" below.

### OpenAPI contract (deferred from M4, owned here)

`shared/openapi.yaml` is named as the contract's source of truth by `coding-standards.md` §1.12, `03-api-contract.md` §12, `00-overview.md` §4, and the §8 pre-flight checklist — but it was never created, so M4's routes and DTOs were written from `03-api-contract.md` prose directly. M5 is where that debt gets paid, before the route count grows further and the backlog becomes a slog.

Deliverables:
- **`shared/openapi.yaml`** — OpenAPI 3.1, hand-maintained, covering every route shipped through M5: M4's 13 (`03-api-contract.md` §2–§4) plus M5's newsletter and candidate-question routes (§5–§6). Include the `gradient` and `avatarColor` enums, which §2.3/§4.3 already designate this file as the canonical home for.
- **Consolidate the Rust wire types into `backend/crates/domain/src/api.rs`.** M4 put them in per-crate `dto.rs` files (`lambda-groups/src/dto.rs`, `lambda-invites/src/dto.rs`), which works but gives the contract test no single target. Move them, re-export per crate, and keep entities (`domain/entities.rs`) separate from wire shapes — they are allowed to differ, and already do (DynamoDB attributes are snake_case, the HTTP contract is camelCase).
- **`scripts/codegen_types.sh`** — runs `openapi-typescript` into `frontend/src/types/api.ts`. Referenced by `scripts/README.md` and `00-overview.md` §4; also missing. The frontend consumes its output starting in M7, but the script belongs with the YAML.
- **The contract test from `11-testing-ci-cd.md` §2.3** — loads the YAML and round-trips each route's example payloads through the corresponding `domain/api.rs` struct. This is what makes the YAML authoritative rather than decorative.

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

Read: `03-api-contract.md` §7, `04-frontend-architecture.md` §8.

**Deliverables**:
- `lambda-responses` with all routes from §7.
- Last-write-wins draft saves — unconditional overwrite, no version tokens or conflict errors (`03-api-contract.md` §7.3, `02-data-model-dynamodb.md` §5).
- Validation: cycle status, image-id ownership / `ready` status / `purpose=response`, body length, image count.
- Integration tests for: save draft, interleaved saves resolve last-write-wins, publish, attempt to publish past deadline.

**Done when**: `cargo test -p lambda-responses` green AND a curl-based dance (save → save → publish) produces correct DDB state.

---

## Milestone 7 — Frontend skeleton + auth (1.5 days)

Read: `04-frontend-architecture.md`, `05-auth-flow.md`.

**Deliverables**:
- Vite project initialized, Tailwind configured, React Router set up with all routes from `04-frontend-architecture.md` §3 as stubs.
- `auth/` module: OIDC client, callback page, token storage, `AuthProvider`, `useAuth`.
- `api/client.ts` with auth + correlation header.
- `api/generated.ts` from `openapi-typescript` against `shared/openapi.yaml`.
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

Read: `08-media-uploads.md`.

**Deliverables**:
- `lambda-media` with all four routes (`/uploads`, `/uploads/{id}/complete`, `/uploads/{id}`, `/uploads/{id}` DELETE, `/media-cookie`).
- `lambda-image-process` real implementation (replacing the M3 stub) — reads originals, generates display + thumb WebP/GIF, writes to processed bucket, marks DDB row ready.
- `frontend/src/components/responses/ImageUploader.tsx` with the drag-drop, presign, PUT, poll loop.
- CloudFront signed-cookie issuance integrated end-to-end (frontend `ensureCookie`, sets `credentials: include`).
- Image-pipeline integration tests per `08-media-uploads.md` §10.

**Done when**: I can upload a 5MB JPEG, see thumbnail render in the editor, and reload the page to see the image rendered via CloudFront.

---

## Milestone 9 — Newsletter UI (2 days)

Read: `04-frontend-architecture.md` §7, `06-newsletter-lifecycle.md`.

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

Read: `09-engagement.md`, `03-api-contract.md` §8.

**Deliverables**:
- `lambda-engagement` for comments + reactions.
- `frontend` `CommentList`, `ReactionBar` components.
- Markdown sanitizer applied consistently.
- Engagement integration tests per `09-engagement.md` §8.

**Done when**: Two users can comment on each other's published answers and react with arbitrary emoji.

---

## Milestone 11 — Notifications (1 day)

Read: `07-notifications.md`.

**Deliverables**:
- `lambda-push` for subscribe/unsubscribe/list/test/preferences + cycle-open / publication / deadline fan-out functions.
- `lambda-notify-tick` and EventBridge schedule.
- Frontend service worker push handler + Settings UI toggle + per-group preferences.
- The `ensurePushSubscription` flow.

**Done when**: I can opt in on Settings, manually invoke the cycle-tick or notify-tick, and receive a real OS notification.

---

## Milestone 12 — Admin UI (½ day)

Read: `04-frontend-architecture.md` §7.7, `03-api-contract.md` §3, §6.5.

**Deliverables**:
- `pages/GroupAdminPage` with the three tabs (Members, Invites, Settings). No Curate tab — admins do not override voting.

**Done when**: I can create an invite, copy the URL, share, and see the new member appear.

---

## Milestone 13 — Hardening (1 day)

**Deliverables**:
- All tests in `11-testing-ci-cd.md` §2–§4 implemented (gaps from prior milestones filled).
- All four GitHub Actions workflows green on `main`.
- Monitoring alarms wired to a working SNS topic + email.
- CSP set on the frontend; verified by a Playwright test.
- Manual smoke runbook (`docs/smoke.md`) executed and observed clean.
- 5-min E2E run against the deployed dev stack.

**Done when**: A new contributor can clone, follow `README.md`, run dev locally, and contribute a PR with passing CI.

---

## Milestone 14 — Production deploy (½ day)

**Deliverables**:
- `cdk deploy --context env=prod --all` runs clean.
- DNS records swung to prod.
- Bootstrap admin + first invite flow exercised in prod.
- Push notifications tested in prod.
- README updated with prod URLs.

**Done when**: The user can sign in to `https://opennewsletter.example.com` and use the app for real.

---

## Cross-cutting checklist (revisit before each milestone PR)

- [ ] All public types in changed files have rustdoc / TSDoc when non-obvious.
- [ ] No new dependency added without justification in PR description.
- [ ] No secrets, ARNs, or environment-specific URLs hard-coded outside `config.py` / `env.ts`.
- [ ] `cargo deny check` clean.
- [ ] OpenAPI YAML updated alongside any new/changed route; codegen re-run.
- [ ] Tests added for new access patterns / transactions.
- [ ] CloudWatch logs were inspected after the change deployed to dev.

---

## When something is ambiguous

If a plan document seems wrong or contradicts another, **stop and surface it** rather than guessing. The plan documents are intended to be edited as we learn — keeping them accurate is more valuable than racing to ship a milestone that diverges from them. Open a thread with the user, propose the change, update the doc, then continue.

If you find a gap (a behavior not specified anywhere): pick the simplest, most boring default, document it as a new section in the appropriate plan file in the same PR, and continue. The bias is "decide and document," not "stop and ask," for low-stakes details.

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
| 14 Prod deploy | 0.5 |
| **Total** | **~16 days** |

This is "focused engineer time," not calendar time.
