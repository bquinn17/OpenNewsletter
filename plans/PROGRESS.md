# Progress & Blockers

Snapshot as of 2026-10-11: M0–M13.5 code-complete, nothing deployed, M14 next. This file is read at the start of every session, so it holds only the current state. Milestone definitions and gates are in [`12-build-order.md`](12-build-order.md). What each milestone landed (bugs found, per-milestone decisions, test deltas) is in [`archive/progress-log.md`](archive/progress-log.md).

---

## Milestone status

| # | Milestone | Status |
|---|---|---|
| M0 | Repo skeleton | ✅ done |
| M1 | Operator prerequisites | 🟡 partial — see B5 |
| M2 | Domain + persistence | ✅ done |
| M3–M13 | Infra → Hardening | 🟡 code-complete; every deploy-shaped gate (`12`) waits on a dev deploy (B5) |
| M13.5 | Plan-review follow-ups (D1–D12) | ✅ code-complete; deploy-shaped checks join M14 |
| M14 | Production deploy | ⬜ **next** |

**Test baselines (2026-10-12):** backend 498 (`cargo test --workspace -- --list`; 498 last full green run), CDK 157 (`pytest infra/tests`), Vitest 334, Playwright 8 (mock project). fmt, clippy `-D warnings`, `cargo deny`, ruff, `mypy --strict`, typecheck, ESLint (0 problems), Prettier, `vite build`, `actionlint` and `cdk synth` (dev + prod) are clean.

---

## Active blockers

### B5 — M1 operator tasks outstanding

Done: domain `betterloop.site` (Namecheap DNS, no Route53 zone, so ACM validation CNAMEs are added at Namecheap by hand); Cognito domain prefix; `aws login` as IAM user `bquinn17` (not root); `cdk bootstrap`; VAPID keys; a separate RSA-2048 CloudFront pair per env (public halves committed in `infra/keys/`); the combined `opennewsletter/dev` secret (all 8 keys); `ALARM_EMAIL`.

Still open:
- **Confirm the OAuth secrets hold real client IDs and secrets**, and that both IdP apps list the redirect `https://<prefix>.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`.
- **Fill in `opennewsletter/prod`.** It holds only `cdn_signing_private_key`; *merge* the OAuth and VAPID keys into it (M14).
- **The four old dev secrets are scheduled for deletion on 2026-10-18.** Deploy dev before then. If the new secret misbehaves, `aws secretsmanager restore-secret --secret-id <name>`.
- **`.env.local` overrides environment variables for prod too** (`load_config`), so a workstation prod synth/deploy would pick up the dev `APP_SECRET_ARN`. Make it dev-only or override it before a local prod bring-up. CI has no `.env.local`.
- `aws login` sessions expire. Check `aws sts get-caller-identity` before deploying.

These block every deploy gate (M3–M14), not code work.

### B6 — Lambda release builds

- **macOS:** artifacts in `backend/target/lambda/` date from 2026-10-04/06 and predate M13 and later fixes; `engagement-api` has never been built. Run `make build-lambdas` right before any deploy (`make deploy-dev` does). `ApiStack`/`NotificationsStack` silently fall back to a shell stub when a binary is missing; `deploy.yml` guards against that with `scripts/check_lambda_binaries.sh`.
- **Windows/WSL2:** release builds abort in `aws-lc-sys` under gcc 9 (gcc bug 95189). Fix with `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain. Debug builds and tests are unaffected.
- `backend-ci`'s first run is also the first Linux build of `image-process` (`libwebp-sys`).

---

## Next steps (in order)

1. **Operator:** clear B5's open items; set `BOOTSTRAP_ADMIN_EMAIL`; configure GitHub per `.github/workflows/README.md`:
   - repo variables `DEPLOY_ENABLED`, `DEV_APP_SECRET_ARN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, `FRONTEND_DOMAIN` (and optionally the `DEV_*` domain variables)
   - the `production` environment with a required reviewer **and a `main`-only deployment-branch rule** (its OIDC subject carries no branch)
   - delete the old `*_SECRET_ARN`, `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED` variables and the `VITE_COGNITO_BOOTSTRAP_CLIENT_ID` secret
2. **Deploy dev before 2026-10-18:** `make deploy-dev && make seed && make fe` (`docs/RUNBOOK.md`).
3. **Run every pending gate in `12`**, M3 through M13.5, against dev. Push a branch so the CI workflows run for real.
4. **M14 prep:** pick the real prod hostnames in place of the `opennewsletter.example.com` placeholders, check `config.py`'s prod values, merge the prod secret, then deploy (`12` M14).

Workstation setup for either machine: [`13-dev-environments.md`](13-dev-environments.md) §0.

---

## Owner decisions

Standing decisions that plan docs and code refer to by ID. Each is written into the owning plan doc as current design; the reasoning is in [`archive/progress-log.md`](archive/progress-log.md).

**2026-10-07 plan review (D1–D12, landed in M13.5):**
- **D1** Tokens in `localStorage` so an installed PWA stays signed in; logout revokes the refresh token (`04` §6, `05` §8).
- **D2** Non-empty drafts auto-publish at close, before the status flip; a poll pick saves as a draft immediately (`06` §5.3, `07` §7.2, `04` §7.3).
- **D3** Default response window is 7 days; all `[96, 48, 24]` reminders fire (`06` §4.2).
- **D4** Promotion order: votes desc, then oldest suggestion; 0-vote candidates still fill slots (`06` §5.2, `03` §6.1).
- **D5** The prod bootstrap admin is an email-bound admin invite redeemed via Google; `admin-bootstrap` is dev-only, so prod has no password auth (`05` §9).
- **D6** One `deploy.yml`: `main` → dev, then a gated `promote-prod` deploys backend then frontend from the same SHA; API changes are expand → contract; the service worker activates promptly (`11` §6).
- **D7** Cost target ≈ $1–2/mo: one secret per env, prod-only alarms within the free 10, one $5 budget in `CiStack`, DynamoDB on an AWS-owned key (`01` §12).
- **D8** Media deletes are real (S3 objects, exact-path CloudFront invalidation, row); no bucket versioning; permanent-delete warnings in the UI (`08` §8.0, `03` §9.4).
- **D9** Shared-device privacy is accepted as-is.
- **D10** Settings changes apply to the current cycle where possible: the voting cycle is rescheduled; an open cycle's close moves only if still in the future, else 422 `CLOSES_IN_PAST` (`06` §6).
- **D11** Flat comments, Web Push and the admin UI are confirmed; threaded comments and poll engagement go to `14`.
- **D12** Per-PR `ci-pr-*` stacks go to `14` §3.

**Other standing decisions:**
- **Apple IdP dropped** (2026-10-05, M1): Sign In with Apple needs a $99/yr Apple Developer membership. Google and Facebook only. Re-adding it means the IdP construct, its config fields and the operator-runbook step.
- **Broken drafts are retried, then skipped** at close: after `AUTO_PUBLISH_MAX_FAILED_SWEEPS` failing ticks the cycle publishes without them and emits `AutoPublishAbandoned` (2026-10-09, `06` §5.3).
- **Field errors carry one name per field, with the reason in `fieldErrors[].code`** (`FieldErrorCode`, `03` §1.1) (2026-10-09).
- **CI hardening** (2026-10-11): the PR `cdk diff` role assumes no bootstrap role (stack templates only, `11` §6); every action is SHA-pinned, with Dependabot bumping them weekly.
- **CDN images load with credentialed CORS** so the service worker can cache them offline (2026-10-11, `01` §5.2, `04` §5.2).
- **Up to 60 s of stale access** after removal or demotion (the per-container membership cache) is accepted (2026-10-11).

---

## Known gaps carried forward

Product and UX:
- The fixed `BottomNav` covers RespondPage's bottom bar ("Save & exit", "Publish") at 700px; both are `fixed bottom-3 z-40`. The e2e test works around it with `goBack()`.
- A poll pick can't be cleared (`pollOptionId` is required), so a poll pick always auto-publishes at close. Decide whether polls need a "clear" action (`04` §7.3).
- No markdown formatting toolbar in `RespondPage` (spec only requires Write/Preview); no "hype check" banner or recurring-question helper text (`04` §14a).
- No invite pagination: `GET /admin/groups/{g}/invites` returns everything, including consumed/revoked invites until their TTL (expiry + 30 days).
- `VOTE_CAP_REACHED` lists the already-voted questions in free-text `detail`, not a structured field.
- PWA icons are generated placeholders (`frontend/scripts/generate-icons.mjs`).
- Fonts come from Google Fonts, so they aren't available offline and the CSP allows two Google origins (`04` §12.1).

Backend and ops:
- The D10 open-cycle close move is best-effort: it runs after the group update, and a write error is logged rather than returned. The 422 check runs before any write.
- A crash mid-fan-out loses the rest of that fan-out (markers are claimed first, at-most-once, `07` §7.1).
- Comment author names aren't cached across requests (one deduped `BatchGetItem` per request; `09` §1.5); `get_comment_by_id` walks the answer's `C#` range because there's no index on `comment_id`. Revisit if either gets hot.
- `lambda-cycle-tick` needs `#![recursion_limit = "256"]`. If it creeps up, box the dispatch futures.
- The account-level Lambda error alarm also counts dev's errors (one account, `01` §9.2).
- `CiStack` deploys with every `--all`, including `make deploy-dev`. It's account-wide and idempotent.
- `rsa` timing advisory (RUSTSEC-2023-0071) is mitigated by blinded signing, not fixed; five more `cargo deny` advisory ignores have no upstream fix (three `rustls-webpki` 0.101 via the AWS SDK's legacy rustls, two test-only). Re-check after AWS SDK bumps.

Testing:
- Mock-mode E2E can't cover reload-persistence of saved drafts (the mock transport is in-memory); that's a `dev`-project test.
- The comment composer's image upload and the `[deleted]` placeholder are only exercised in mock mode and component tests until the M10 gate runs.

---

## How to update this file

Keep it short (under ~200 lines). At the end of each milestone:
- update the status table, the snapshot date and the test baselines
- prune cleared blockers, gaps and next steps; add new ones
- add standing owner decisions above (one line each, pointing at the owning plan section)
- append an "Mn detail — what landed" section to [`archive/progress-log.md`](archive/progress-log.md): what landed, bugs found in review, per-milestone decisions (also written into the owning plan doc), test deltas
