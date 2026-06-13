# 00 — Overview & System Map

This is the entry point for the OpenNewsletter plan set. Every other plan document in this directory assumes the architecture, glossary, and repo layout described here. A subagent picking up any single plan should also read this file.

> **Required reading before writing or editing any code:** [`coding-standards.md`](coding-standards.md). It defines the style, linting, testing, error-handling, and PR conventions every contributor — human or agent — is expected to follow. A milestone or task is not "done" until its work conforms to those standards.

---

## 1. What we are building

OpenNewsletter is a multi-tenant PWA where small groups of friends collaboratively produce a monthly newsletter:

1. Continuously between cycles, members suggest and upvote questions for the next edition.
2. On the 1st of each month (in the group's timezone), the top-voted questions auto-promote into the active newsletter and a 4-day response window opens.
3. During the response window, members write long-form text answers (with up to 10 image/GIF attachments per answer) or vote in poll questions. Drafts autosave; only the author can see their draft.
4. At the close of the response window the newsletter auto-publishes. Comments and emoji reactions become available on every answer.

All values above (cycle length, response window, questions-per-cycle, timezone) are configured per-group and editable.

---

## 2. Architectural principles

- **Idle backend.** Unauthenticated traffic never reaches Lambda. The static frontend is served by GitHub Pages on a custom domain; only after Cognito issues a JWT does the frontend call API Gateway.
- **Single repo, IaC-first.** The frontend, backend, infrastructure, and operational scripts all live in one repo. The CDK app is the source of truth for AWS resources.
- **Cheap by default.** DynamoDB on-demand, Lambda + HTTP API, CloudFront cache in front of S3, no always-on compute, no NAT gateway. See `01-infrastructure-cdk.md` for the cost model.
- **Single-table DynamoDB.** All durable application state lives in one table (`OpenNewsletter`) with two GSIs. Multi-tenancy is enforced through partition-key prefixes — see `02-data-model-dynamodb.md`.
- **Strict tenant isolation in code.** Every Lambda handler resolves the caller's group memberships from the JWT claims and the membership table, then refuses any request whose path/body group ID is not in that set.
- **Best-judgment defaults.** Anywhere this plan picks a number (autosave debounce, image dimensions, vote counts, etc.), the value is captured as a constant in `backend/crates/shared/src/config.rs` so it can change in one place.

---

## 3. Stack summary

| Layer | Choice |
|---|---|
| Frontend | React 18 + TypeScript + Vite + Tailwind, deployed as static assets to GitHub Pages on a custom domain |
| PWA | Workbox-generated service worker, Web Push (VAPID) for notifications |
| Auth | Amazon Cognito User Pool with Google / Apple / Facebook IdPs; JWT validated by API Gateway |
| API | Amazon API Gateway HTTP API with Cognito JWT authorizer |
| Compute | AWS Lambda, Rust (`provided.al2023` runtime via `cargo-lambda`) |
| Data | Amazon DynamoDB single-table design, on-demand billing |
| Media | Amazon S3 (originals + processed) behind CloudFront with signed cookies |
| Notifications | Web Push directly from Lambda using the `web-push` crate; VAPID keys stored in Secrets Manager |
| Scheduling | Amazon EventBridge scheduler rules (cycle tick + notification tick Lambdas) |
| IaC | AWS CDK in Python |
| CI/CD | GitHub Actions: separate workflows for frontend, backend, and infra |

---

## 4. Repo layout

```
OpenNewsletter/
├── frontend/                       # React + TS + Tailwind PWA
│   ├── src/
│   │   ├── api/                    # generated client + thin wrappers
│   │   ├── auth/                   # Cognito hosted-UI integration
│   │   ├── components/
│   │   ├── pages/
│   │   ├── pwa/                    # service worker registration, push
│   │   ├── state/                  # Zustand stores
│   │   ├── styles/
│   │   ├── types/                  # generated from backend OpenAPI
│   │   └── main.tsx
│   ├── public/
│   ├── index.html
│   ├── vite.config.ts
│   ├── tailwind.config.ts
│   ├── tsconfig.json
│   └── package.json
│
├── backend/                        # Rust workspace
│   ├── Cargo.toml                  # workspace root
│   ├── crates/
│   │   ├── domain/                 # entities, errors, ports (no IO)
│   │   ├── persistence/            # DynamoDB single-table adapter
│   │   ├── shared/                 # config, JWT verify, telemetry, S3 helpers
│   │   ├── lambda-invites/         # POST /invites, POST /invites/redeem
│   │   ├── lambda-groups/          # GET /groups, GET /groups/{id}
│   │   ├── lambda-newsletters/     # GET /groups/{g}/newsletters, GET /groups/{g}/newsletters/{id}
│   │   ├── lambda-questions/       # candidate questions: suggest, vote, list
│   │   ├── lambda-responses/       # drafts, publish, poll votes
│   │   ├── lambda-engagement/      # comments + reactions
│   │   ├── lambda-media/           # POST /uploads (presign), GET /media/{id}
│   │   ├── lambda-push/            # subscribe/unsubscribe/test
│   │   ├── lambda-image-process/   # S3 ObjectCreated trigger
│   │   ├── lambda-cycle-tick/      # EventBridge: open/close/publish cycles
│   │   └── lambda-notify-tick/     # EventBridge: 4d/48h/24h reminders
│   └── README.md
│
├── infra/                          # CDK Python
│   ├── app.py
│   ├── cdk.json
│   ├── requirements.txt
│   ├── opennewsletter/
│   │   ├── __init__.py
│   │   ├── config.py               # env-specific settings (dev/prod)
│   │   ├── data_stack.py           # DynamoDB
│   │   ├── auth_stack.py           # Cognito user pool + IdPs + domain
│   │   ├── media_stack.py          # S3 + CloudFront + image-process Lambda
│   │   ├── api_stack.py            # HTTP API, all request Lambdas, routes
│   │   ├── notifications_stack.py  # EventBridge schedules + tick Lambdas + Secrets
│   │   ├── frontend_stack.py       # ACM cert + DNS records (Route53 if applicable)
│   │   └── monitoring_stack.py     # CloudWatch dashboards + alarms + log groups
│   └── tests/
│
├── shared/
│   └── openapi.yaml                # source of truth for API; both sides codegen from it
│
├── scripts/
│   ├── bootstrap_admin.py          # creates first group + admin (see 05-auth-flow.md)
│   ├── generate_vapid_keys.py
│   ├── seed_dev_data.py
│   └── codegen_types.sh            # openapi-typescript + openapi-generator for Rust
│
├── plans/                          # this directory
│
├── .github/workflows/
│   ├── frontend-ci.yml             # build + test PRs
│   ├── frontend-deploy.yml         # push to gh-pages branch on main
│   ├── backend-ci.yml              # cargo test + clippy
│   ├── backend-deploy.yml          # cargo lambda build + cdk deploy
│   └── infra-ci.yml                # cdk synth + cdk diff on PR
│
├── CLAUDE.md                       # runtime guidance for AI assistants in-repo
├── OpenNewsletter_concept.md       # original concept (read-only reference)
├── OpenNewsletter_spec.md          # original spec (read-only reference)
└── README.md
```

---

## 5. Glossary (use these terms exactly)

| Term | Meaning |
|---|---|
| **Group** | A tenant. A collection of users that share newsletters. Has its own admin(s), settings, and timezone. |
| **Member** | A user who belongs to a group. Roles: `admin` or `member`. |
| **Cycle / Newsletter** | One monthly edition for one group. Identified by `groupId + yyyymm`. Has a status: `voting | open | published | archived`. |
| **Candidate question** | A user-suggested question in the pool for the next cycle, eligible for upvotes. Authored by the submitter by default; the submitter may opt in to anonymous display. |
| **Locked question** | A candidate that has been promoted into a specific cycle and is now visible as a prompt. May be `kind: text` or `kind: poll`. |
| **Response / Answer** | A user's reply to one locked question in one cycle. Has a `status: draft | published`. May contain text + up to 10 image attachments, OR a poll vote. |
| **Comment** | A flat, post-publication reply to a specific answer. Author can edit/delete. Admins can delete any. |
| **Reaction** | Arbitrary emoji applied to an answer by a user. Last-write-wins per `(answerOwner, reactor, emoji)`. |
| **Invite** | A single-use, time-limited code generated by an admin that grants membership in exactly one group on signup. |
| **Push subscription** | A browser PushSubscription object keyed by endpoint, owned by one user, used to deliver Web Push messages. |

---

## 6. Cross-cutting conventions

- **IDs.** All IDs are UUIDv7 strings except group cycle IDs which are `yyyymm` (e.g. `202605`). UUIDv7 sorts lexicographically by creation time, which the data layer relies on. `userId` is a UUIDv7 we generate when an account first redeems an invite — it is NOT the Cognito `sub`. A small `pk=COGNITO_SUB#{sub}, sk=USER_ID` lookup row maps the JWT's `sub` → `userId` on every request; see `02-data-model-dynamodb.md` §2.1.
- **Timestamps.** All timestamps stored as ISO-8601 UTC strings. Group-local rendering happens client-side using the group's `timezone` field.
- **Money.** Not applicable — this is a no-cost-to-user app.
- **Errors.** Backend returns RFC-7807 Problem Details JSON. Error catalog in `03-api-contract.md`.
- **Logging.** Structured JSON logs via `tracing` + `tracing-subscriber` with a `correlation_id` taken from `x-correlation-id` request header (frontend generates ULIDs).
- **Auth claims.** Lambdas read `sub` (Cognito user ID) and `email` from the validated JWT context injected by API Gateway. Group memberships are NOT in the JWT — they are resolved on every request via DynamoDB (cached in-memory per cold-start for ≤60s).
- **Question authorship & anonymity.** Questions are attributed to their submitter by default — candidate questions and locked questions return `submittedBy` (userId + display name) so the UI can render "Kari asked: …" style attribution. A submitter may opt in to anonymous display by setting `isAnonymous: true` on submission; in that case the server omits `submittedBy` for non-admin callers (admins always see it for moderation). The submitter's `userId` is always recorded server-side regardless.

---

## 7. Environments

Two environments managed by CDK context:

- `dev` — single shared AWS account with stack suffix `-dev`. Uses `dev.opennewsletter.example.com` and `api-dev.opennewsletter.example.com`. Pre-seeded test users and one test group.
- `prod` — same account, stack suffix `-prod`.

Both environments live in one personal AWS account (the account may host unrelated personal projects later; IAM users will remain few). Stronger account-level isolation is out of scope for v1; revisit if the threat model changes.

CDK reads `--context env=dev|prod` and merges from `infra/opennewsletter/config.py`.

---

## 8. How to use this plan set

The remaining documents are designed to be read in order during initial implementation, but each is self-contained enough to be picked up independently:

- `coding-standards.md` — **required reading for any code-writing task**; style, linting, testing, error handling, naming, comments, commits, PR hygiene
- `01-infrastructure-cdk.md` — every AWS resource, IAM policy, and CDK stack in detail
- `02-data-model-dynamodb.md` — the single-table schema, every entity, every access pattern
- `03-api-contract.md` — every HTTP endpoint with request/response shapes and auth rules
- `04-frontend-architecture.md` — pages, routing, state, PWA, build
- `05-auth-flow.md` — Cognito setup, OAuth, invite redemption, bootstrap script
- `06-newsletter-lifecycle.md` — cycle state machine + EventBridge schedules
- `07-notifications.md` — Web Push (VAPID), tick Lambda, subscription lifecycle
- `08-media-uploads.md` — pre-signed uploads, validation, image processing pipeline
- `09-engagement.md` — comments, reactions, polls
- `10-archival.md` — future cold-storage plan (deferred but designed)
- `11-testing-ci-cd.md` — test strategy and GitHub Actions workflows
- `12-build-order.md` — sequenced milestones a subagent can execute end-to-end

`12-build-order.md` is the recommended starting point for an executor: it references all other documents in the right order.
