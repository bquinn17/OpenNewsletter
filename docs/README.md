# OpenNewsletter documentation

OpenNewsletter is a multi-tenant PWA where small groups of friends put together a monthly newsletter. Members suggest and upvote questions, answer the winning ones during a response window (a week by default), and read and react to each other's answers once the edition publishes. It runs on a static frontend plus a serverless AWS backend built to sit idle and cost a dollar or two a month.

These pages describe the system as built: how each part works, why it works that way, and how to develop and operate it.

## Where to start

- **New to the project:** read [`overview.md`](overview.md) (architecture, stack, repo layout, glossary), then the design page for the area you care about.
- **Writing code:** set up your machine with [`environments.md` §0](development/environments.md#0-workstation-setup), then read [`coding-standards.md`](development/coding-standards.md) and [`testing-and-ci.md`](development/testing-and-ci.md). The standards end with a pre-flight checklist for every PR.
- **Deploying or operating:** [`operator-setup.md`](operations/operator-setup.md) covers the one-time setup, [`runbook.md`](operations/runbook.md) the deploys, and [`smoke-test.md`](operations/smoke-test.md) the check to run afterwards.

## All pages

### Design

How each subsystem works. Sections are numbered and cited from code.

| Page | Covers |
|---|---|
| [`infrastructure.md`](design/infrastructure.md) | Every AWS resource and CDK stack, IAM, monitoring and alarms, cost |
| [`data-model.md`](design/data-model.md) | The DynamoDB single table: entities, keys, access patterns, transactions |
| [`api.md`](design/api.md) | Every HTTP route: semantics, auth and errors. Wire shapes are in [`shared/openapi.yaml`](../shared/openapi.yaml) |
| [`frontend.md`](design/frontend.md) | The React PWA: routing, pages, drafts, data flow, service worker, build |
| [`auth.md`](design/auth.md) | Cognito sign-in, invite redemption, the bootstrap admin, tenant isolation |
| [`lifecycle.md`](design/lifecycle.md) | The cycle state machine and the tick that drives it |
| [`notifications.md`](design/notifications.md) | Web Push: subscriptions, preferences, fan-outs, deadline reminders |
| [`media.md`](design/media.md) | Image uploads, the processing pipeline, CDN signing, deletion, avatars |
| [`engagement.md`](design/engagement.md) | Comments, reactions and polls |
| [`archival.md`](design/archival.md) | Cold storage for old editions. Designed, not built |

### Development

| Page | Covers |
|---|---|
| [`environments.md`](development/environments.md) | Workstation setup (macOS and WSL2), the dev stack, local testing, debugging |
| [`testing-and-ci.md`](development/testing-and-ci.md) | The test layers, how to run them, and the GitHub Actions workflows |
| [`coding-standards.md`](development/coding-standards.md) | Style, errors, logging, tests, commits and PRs, and the pre-flight checklist |

### Operations

| Page | Covers |
|---|---|
| [`operator-setup.md`](operations/operator-setup.md) | One-time setup per environment: domain, OAuth apps, keys, the app secret |
| [`runbook.md`](operations/runbook.md) | Deploying dev and prod, bootstrapping the first admin, verifying a deploy, day-to-day operations |
| [`smoke-test.md`](operations/smoke-test.md) | The 15-minute manual check after a deploy |

### Reference

| Page | Covers |
|---|---|
| [`decisions.md`](decisions.md) | The owner's standing decisions (D1–D12 and others) and the reasoning behind them |
| [`future-ideas.md`](future-ideas.md) | Designs kept for later but not scheduled |

## Elsewhere in the repo

- [`shared/openapi.yaml`](../shared/openapi.yaml) is the API contract. Both sides generate code from it.
- [`backend/crates/shared/src/config.rs`](../backend/crates/shared/src/config.rs) holds every tunable number: windows, limits, debounces, image sizes.
- [`.github/workflows/README.md`](../.github/workflows/README.md) lists the CI/CD workflows and the repo variables they need.
- Each top-level directory and backend crate has a short `README.md`.
- [`plans/`](../plans/) is project tracking, not documentation: [`PROGRESS.md`](../plans/PROGRESS.md) (status, blockers, known gaps), [`build-order.md`](../plans/build-order.md) (milestones and their done-when gates) and [`archive/`](../plans/archive/) (milestone history and the original spec).

## Keeping these docs current

- **Change the docs with the code.** A change to behaviour updates the page that describes it, in the same PR. A new or changed route also updates `shared/openapi.yaml` and the contract test ([`coding-standards.md` §8](development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
- **Keep section numbers stable.** Code comments cite design sections as `docs/design/<page>.md` §N, and pages link to each other as [`lifecycle.md` §5.3](design/lifecycle.md#53-publishnl). Reword headings freely, but never renumber; new sections go at the end of a page.
- **Use the glossary's terms** ([`overview.md` §5](overview.md#5-glossary-use-these-terms-exactly)): Group, Member, Cycle/Newsletter, Candidate question, Locked question, Response/Answer, Comment, Reaction, Invite, Push subscription.
- **Record decisions.** When the owner settles a design question, write the outcome into the owning page and the reasoning into [`decisions.md`](decisions.md).
- **Surface contradictions.** If a page disagrees with the code or with another page, raise it rather than guessing which one is right.
