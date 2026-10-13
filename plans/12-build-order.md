# 12 — Build Order

The milestone sequence from empty repo to a deployed v1. Each milestone names its plan docs, deliverables and a "done when" gate. Before starting one, read [`00-overview.md`](00-overview.md) and [`coding-standards.md`](coding-standards.md); the §8 pre-flight checklist in the latter applies before any PR.

M0–M13.5 are code-complete. Their full definitions (deliverables lists, effort estimates) are archived in [`archive/build-order-m0-m13.5.md`](archive/build-order-m0-m13.5.md), and what each landed is in [`archive/progress-log.md`](archive/progress-log.md). Live status is in [`PROGRESS.md`](PROGRESS.md).

---

## Completed milestones and their gates

Every gate from M3 onward needs a deployed dev stack, so none has been run yet (blocker B5). Run them all against dev before M14, in order. `docs/RUNBOOK.md` has the commands for M3–M5, M7, M11, M13 and M14, and `docs/smoke.md` is the end-to-end checklist.

| Milestone | Read | Done when |
|---|---|---|
| **M0** Repo skeleton | `00` §4 | Layout matches `00` §4. ✅ |
| **M1** Operator prerequisites | `05` §10, `docs/milestone-1-operator-runbook.md` | All ARNs/IDs recorded in `config.py` / `.env.local` (dev) and in the `opennewsletter/{env}` secret. Operator-only; agents use placeholders. Partly done — see B5. |
| **M2** Domain + persistence | `02` | `cargo test -p persistence` green. ✅ |
| **M3** Infra baseline | `01` | `cdk synth` (dev) and `pytest infra/tests` green ✅; `cdk deploy` of the base stacks produces working resources. |
| **M3.5** Dev environment online | `13` | `make deploy-dev && make seed && make fe`, sign in at `/admin/bootstrap-login` as the seeded admin and see the seeded group; `make redeploy-volatile` succeeds. |
| **M4** Auth + bootstrap | `05` | `bootstrap_admin.py --env dev …` creates a working group + admin, and that user's Cognito token gets `GET /me` via curl. |
| **M5** Lifecycle + cycle CRUD + OpenAPI | `06`, `03` §5–§6 | Manual flow via `POST /admin/dev/tick/cycle` with `advanceCycleClosesBy`: group has a `voting` cycle → candidates suggested → tick → `open` → tick → `published` → next `voting` cycle exists. Contract test green ✅. |
| **M6** Responses + drafts | `03` §7, `04` §8 | `cargo test -p lambda-responses` green ✅; curl save → save → publish against `PUT …/my-response` gives the right DynamoDB state. |
| **M7** Frontend skeleton + auth | `04`, `05` | Against the dev API: sign in with Google, redeem an invite made by `POST /admin/invites` and see the group on Home, log out. Also try `/admin/bootstrap-login`. |
| **M8** Media pipeline | `08` | Upload a 5 MB JPEG through `ImageUploader`, see the thumbnail, reload, and see it via CloudFront. **Also:** in dev a group-B image URL with group-A signed params is a 403; run the CDN CORS check (`docs/smoke.md` §6) — images load with `crossorigin="use-credentials"`, so a wrong CORS policy breaks every image. Repeat the CORS check after the first prod deploy. |
| **M9** Newsletter UI | `04` §7, `06` | A simulated month in the UI: suggest and vote → tick to promote and open → draft with images, reload, publish text and poll answers → tick to publish → read the edition. |
| **M10** Engagement | `09`, `03` §8 | Two users comment on each other's published answers (one with an image), edit and delete a comment, react with a ZWJ emoji (👨‍👩‍👧‍👦) and a flag; an admin deletes the other user's comment. Check whether API Gateway passes the emoji path param decoded or encoded (the handler accepts both, `03` §8.6). |
| **M11** Notifications | `07` | Enable push on desktop Chrome and an installed iOS PWA; "Send test push" reaches both and "My devices" lists both. Tick to open → one cycle-open push (re-tick: no duplicate). Rewind into the reminder window and `POST /admin/dev/tick/notify` → one reminder, not a burst. Tick to publish → publication push opens the edition. A member with "Deadline reminders" off is skipped. `PushSent`/`PushFailed` EMF metrics appear in CloudWatch. |
| **M12** Admin UI | `04` §7.7, `03` §3 | As an admin: create a member invite, open it signed in as a second Google account → new member on the Members tab, invite shows "used by {name}". Promote, demote, then remove them. Rename the group and change its gradient → Home and the switcher update without a reload. |
| **M13** Hardening | `11` | A new contributor can clone, follow `README.md`, run dev locally and open a PR with passing CI. **Also:** the five workflows go green on GitHub (only checked locally and with `actionlint` so far — `backend-ci` is also the first Linux `cargo lambda build` of `image-process`/`libwebp-sys`); deploy `CiStack` and paste its role ARNs into the repo variables; run `docs/smoke.md` against dev; fill in the `e2e/dev/` Playwright bodies once a Cognito test user exists. |
| **M13.5** Plan-review follow-ups (D1–D12) | `PROGRESS.md` decisions | All checks green ✅. Deploy-shaped checks join M14: a bound-invite redeem via Google; a Facebook sign-up gets `email_verified=false`; an auto-publish at a real close; a media delete that 403s at the CDN afterwards; the `AutoPublishAbandoned` metric reaches CloudWatch and feeds the `CycleTick-Errors` alarm; the first `promote-prod`. |

---

## Milestone 14 — Production deploy (½ day)

Read: `docs/RUNBOOK.md` M14, `11` §6.4, `05` §9.1a. Mostly operator work: an agent can prepare and verify, but every `cdk deploy`, secret write and DNS change needs the owner.

**Deliverables**:
- `cdk deploy --context env=prod --all` runs clean.
- DNS records swung to prod.
- Bootstrap admin flow exercised in prod: `bootstrap_admin.py --env prod` with `BOOTSTRAP_ADMIN_EMAIL` prints an email-bound admin invite, and the owner redeems it by signing in with Google (`05` §9).
- First promotion through the `production` environment gate (`11` §6).
- Push notifications tested in prod. Prod alarms (10 alarm metrics) sit in `OK`; the SNS and budget email subscriptions are confirmed.
- README updated with prod URLs.

**Done when**: the owner can sign in to the prod domain and use the app for real.

---

## Cross-cutting checklist (revisit before each milestone PR)

- [ ] All public types in changed files have rustdoc / TSDoc when non-obvious.
- [ ] No new dependency added without justification in the PR description.
- [ ] No secrets, ARNs, or environment-specific URLs hard-coded outside `config.py` / `env.ts`.
- [ ] `cargo deny check` clean.
- [ ] OpenAPI YAML updated alongside any new/changed route; codegen re-run.
- [ ] Tests added for new access patterns / transactions.
- [ ] CloudWatch logs were inspected after the change deployed to dev.

When a plan doc is ambiguous or wrong, follow `CLAUDE.md` "When something is ambiguous" and `coding-standards.md` §7.
