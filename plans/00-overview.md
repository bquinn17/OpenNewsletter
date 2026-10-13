# 00 — Overview & System Map

Entry point for the plan set: every other plan assumes the architecture, glossary and repo layout described here. Also required before writing code: [`coding-standards.md`](coding-standards.md).

---

## 1. What we are building

OpenNewsletter is a multi-tenant PWA where small groups of friends collaboratively produce a monthly newsletter:

1. Continuously between cycles, members suggest and upvote questions for the next edition.
2. On the 1st of each month (in the group's timezone), the top-voted questions auto-promote into the active newsletter and a 7-day response window opens.
3. During the response window, members write long-form text answers (with up to 10 image/GIF attachments per answer) or vote in poll questions. Drafts autosave; only the author can see their draft.
4. At the close of the response window, any non-empty draft answer (text with a body or images, or a poll pick) is auto-published, then the newsletter as a whole auto-publishes and comments/emoji reactions become available on every published text answer. Empty drafts stay drafts and are excluded.

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
| PWA | `vite-plugin-pwa` (`injectManifest`) service worker, Web Push (VAPID) for notifications |
| Auth | Amazon Cognito User Pool with Google / Facebook IdPs; JWT validated by API Gateway |
| API | Amazon API Gateway HTTP API with Cognito JWT authorizer |
| Compute | AWS Lambda, Rust (`provided.al2023` runtime via `cargo-lambda`) |
| Data | Amazon DynamoDB single-table design, on-demand billing |
| Media | Amazon S3 (originals + processed) behind CloudFront with signed cookies |
| Notifications | Web Push sent directly from Lambda (own RFC 8291/8292 implementation, `07` §10); VAPID keys in the per-env Secrets Manager secret |
| Scheduling | Amazon EventBridge scheduler rules (cycle tick + notification tick Lambdas) |
| IaC | AWS CDK in Python |
| CI/CD | GitHub Actions: a CI workflow per area, one `deploy.yml` (dev auto-deploy, gated prod promotion), nightly E2E (`11` §6) |

---

## 4. Repo layout

```
OpenNewsletter/
├── frontend/                       # React + TS + Tailwind PWA — layout in 04 §2
│
├── backend/                        # Rust workspace
│   ├── Cargo.toml                  # workspace root
│   ├── crates/
│   │   ├── domain/                 # entities, errors, ports (no IO)
│   │   ├── persistence/            # DynamoDB single-table adapter
│   │   ├── shared/                 # config constants, HTTP/claims helpers, telemetry, cycle time math
│   │   ├── media-delete/           # S3 + CloudFront + row deletion shared by media callers
│   │   ├── lambda-invites/         # invite create/list/revoke/redeem + PreSignUp trigger
│   │   ├── lambda-groups/          # /config, /me, groups, members
│   │   ├── lambda-newsletters/     # newsletter list + detail
│   │   ├── lambda-questions/       # candidate questions: suggest, vote, list, admin delete
│   │   ├── lambda-responses/       # drafts, publish, poll votes
│   │   ├── lambda-engagement/      # comments + reactions
│   │   ├── lambda-media/           # uploads (presign), media cookie, avatars
│   │   ├── lambda-push/            # push routes + cycle-open/publication fan-out
│   │   ├── lambda-image-process/   # S3 ObjectCreated trigger
│   │   ├── lambda-cycle-tick/      # EventBridge: open/close/publish cycles
│   │   └── lambda-notify-tick/     # EventBridge: deadline reminders
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
│   │   ├── media_persistent_stack.py  # S3 buckets + CloudFront (stable)
│   │   ├── media_pipeline_stack.py    # image-process Lambda + S3 events (volatile)
│   │   ├── api_stack.py            # HTTP API, all request Lambdas, routes
│   │   ├── lambda_assets.py        # locates cargo-lambda build output for CDK assets
│   │   ├── notifications_stack.py  # EventBridge schedules + tick Lambdas
│   │   ├── frontend_stack.py       # ACM cert + DNS records (Route53 if applicable)
│   │   ├── monitoring_stack.py     # CloudWatch dashboard + prod alarms
│   │   ├── ci_stack.py             # account-wide GitHub OIDC roles + budget
│   │   └── ssm_refs.py             # SSM parameter names shared between stacks
│   └── tests/
│
├── shared/
│   └── openapi.yaml                # source of truth for API; both sides codegen from it
│
├── scripts/
│   ├── bootstrap_admin.py          # creates first group + admin (see 05-auth-flow.md)
│   ├── generate_vapid_keys.py
│   ├── seed_dev_data.py
│   ├── write_frontend_env.py       # CDK outputs → frontend/.env.{env}
│   ├── check_lambda_binaries.sh    # refuses to deploy shell-stub Lambdas
│   └── codegen_types.sh            # openapi-typescript → frontend/src/types/api.ts
│
├── plans/                          # this directory (history in plans/archive/)
├── docs/                           # operator runbooks + smoke checklist
├── Makefile                        # dev-stack shortcuts (13 §8)
│
├── .github/workflows/
│   ├── frontend-ci.yml             # typecheck, lint, test, build
│   ├── backend-ci.yml              # fmt, clippy, cargo deny, cargo test, Lambda cross-build
│   ├── infra-ci.yml                # ruff, mypy, pytest, cdk synth; cdk diff on PR
│   ├── deploy.yml                  # main → dev; gated promote-prod → prod backend + gh-pages
│   └── e2e.yml                     # nightly Playwright
│
├── CLAUDE.md                       # runtime guidance for AI assistants in-repo
├── OpenNewsletter_concept.md       # original concept (read-only reference)
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
- **Errors.** Backend returns RFC-7807 Problem Details JSON. Error catalog in `03-api-contract.md`.
- **Logging.** Structured JSON logs via `tracing` + `tracing-subscriber` with a `correlation_id` taken from `x-correlation-id` request header (frontend generates ULIDs).
- **Auth claims.** Lambdas read `sub` (Cognito user ID) and `email` from the validated JWT context injected by API Gateway. Group memberships are NOT in the JWT — they are resolved on every request via DynamoDB (cached in-memory per cold-start for ≤60s).
- **Question authorship & anonymity.** Questions are attributed to their submitter by default — candidate questions and locked questions return `submittedBy` (userId + display name) so the UI can render "Kari asked: …" style attribution. A submitter may opt in to anonymous display by setting `isAnonymous: true` on submission; in that case the server omits `submittedBy` for non-admin callers (admins always see it for moderation). The submitter's `userId` is always recorded server-side regardless.

---

## 7. Environments

Two environments managed by CDK context:

- `dev` — stack suffix `-dev`. **No custom domains**: raw AWS endpoints (`*.execute-api…`, `*.cloudfront.net`), per `13` §2. Seeded via `make seed`.
- `prod` — stack suffix `-prod`, custom domains, the only environment with a hosted frontend and alarms.

Both live in one personal AWS account; stronger account-level isolation is out of scope for v1.

CDK reads `--context env=dev|prod` and merges from `infra/opennewsletter/config.py`.

---

## 8. How to use this plan set

Each document is self-contained enough to pick up on its own; numbers are cited as e.g. "`06` §5.3".

- `coding-standards.md` — **required for any code-writing task**
- `01-infrastructure-cdk.md` — every AWS resource, IAM policy and CDK stack
- `02-data-model-dynamodb.md` — single-table schema, entities, access patterns, transactions
- `03-api-contract.md` — every HTTP route's semantics and auth (wire shapes: `shared/openapi.yaml`)
- `04-frontend-architecture.md` — pages, routing, state, PWA, build
- `05-auth-flow.md` — Cognito, OAuth, invite redemption, bootstrap admin
- `06-newsletter-lifecycle.md` — cycle state machine and the tick
- `07-notifications.md` — Web Push, reminders, subscriptions
- `08-media-uploads.md` — uploads, image pipeline, CDN signing, deletion
- `09-engagement.md` — comments, reactions, polls
- `10-archival.md` — future cold-storage design (deferred)
- `11-testing-ci-cd.md` — test strategy and GitHub Actions
- `12-build-order.md` — milestones and their done-when gates
- `13-dev-environments.md` — workstation setup, the dev stack, local testing
- `14-future-ideas.md` — kept but unscheduled ideas
- `PROGRESS.md` — current status, blockers, owner decisions; `archive/` holds the milestone history
- `OpenNewsletter_spec.md` — the original spec (read-only reference; superseded where the plans differ)
