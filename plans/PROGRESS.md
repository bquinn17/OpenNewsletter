# Progress & Blockers

Snapshot as of 2026-10-18: M0–M13.5 code-complete plus the design-review fixes and presigned-S3 media serving (with its code-review fixes), nothing deployed, M14 next. This file is read at the start of every session, so it holds only the current state. How the system works is documented in [`docs/`](../docs/README.md). Milestone definitions and gates are in [`build-order.md`](build-order.md). What each milestone landed (bugs found, per-milestone decisions, test deltas) is in [`archive/progress-log.md`](archive/progress-log.md).

---

## Milestone status

| # | Milestone | Status |
|---|---|---|
| M0 | Repo skeleton | ✅ done |
| M1 | Operator prerequisites | 🟡 partial — see B5 |
| M2 | Domain + persistence | ✅ done |
| M3–M13 | Infra → Hardening | 🟡 code-complete; every deploy-shaped gate ([`build-order.md`](build-order.md)) waits on a dev deploy (B5) |
| M13.5 | Plan-review follow-ups (D1–D12) | ✅ code-complete; deploy-shaped checks join M14 |
| — | Design review: performance and cost (2026-10-08) | ✅ code-complete; open questions below |
| — | Media serving via presigned S3 (2026-10-14; code-review fixes 2026-10-18) | ✅ code-complete; deploy checks join M14 (below) |
| M14 | Production deploy | ⬜ **next** |

**Test baselines (2026-10-15):** backend 498 (`cargo test --workspace`, 2026-10-18), CDK 151 (`pytest infra/tests`), Vitest 346 (2026-10-18), Playwright 8 (mock project). fmt, clippy `-D warnings`, ruff, `mypy --strict`, typecheck, ESLint (0 problems), Prettier, `vite build`, `actionlint` and `cdk synth` (dev + prod) are clean. `cargo deny check` is clean (2026-10-18).

---

## Active blockers

### B5 — M1 operator tasks outstanding

Done: domain `betterloop.site` (Namecheap DNS, no Route53 zone, so ACM validation CNAMEs are added at Namecheap by hand); Cognito domain prefix; `aws login` as IAM user `bquinn17` (not root); `cdk bootstrap`; VAPID keys; the combined `opennewsletter/dev` secret; `ALARM_EMAIL`.

Still open:
- **Confirm the OAuth secrets hold real client IDs and secrets**, and that both IdP apps list the redirect `https://<prefix>.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`.
- **Fill in `opennewsletter/prod`** with the OAuth and VAPID keys: write the full key set with `aws secretsmanager put-secret-value` (M14, [`runbook.md`](../docs/operations/runbook.md)).
- **The old per-credential dev secrets are scheduled for deletion on 2026-10-18.** Deploy dev before then. If the new secret misbehaves, `aws secretsmanager restore-secret --secret-id <name>`.
- **`.env.local` overrides environment variables for prod too** (`load_config`), so a workstation prod synth/deploy would pick up the dev `APP_SECRET_ARN`. Make it dev-only or override it before a local prod bring-up (swap `APP_SECRET_ARN`/`ROOT_DOMAIN`/`API_DOMAIN` with the `PROD_*`-prefixed lines next to them, then swap back). CI has no `.env.local`.
- `aws login` sessions expire. Check `aws sts get-caller-identity` before deploying.

**Decision (2026-10-13) — dev gets its own subdomain (`dev.betterloop.site`/`api.dev.betterloop.site`), not prod's apex.** `config.py` has no local dev/prod split for `ROOT_DOMAIN`/`API_DOMAIN`, and the ACM cert covers both names in every env, so dev pointed at the apex would put prod's hostnames on the dev cert. Set in `.env.local`; full writeup in [`docs/development/environments.md` §2](../docs/development/environments.md#2-environments).

These block every deploy gate (M3–M14), not code work.

### B6 — Lambda release builds

- **macOS:** all 12 artifacts in `backend/target/lambda/` were built 2026-10-11/12, but they predate the last M13.5 fixes and the design review. Run `make build-lambdas` right before any deploy (`make deploy-dev` does). `ApiStack`/`NotificationsStack` silently fall back to a shell stub when a binary is missing; `deploy.yml` guards against that with `scripts/check_lambda_binaries.sh`.
- **Windows/WSL2:** release builds abort in `aws-lc-sys` under gcc 9 (gcc bug 95189). Fix with `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain. Debug builds and tests are unaffected.
- `backend-ci`'s first run is also the first Linux build of `image-process` (`libwebp-sys`).

---

## Next steps (in order)

1. **Operator:** clear B5's open items; set `BOOTSTRAP_ADMIN_EMAIL`; configure GitHub per `.github/workflows/README.md`:
   - repo variables `DEPLOY_ENABLED`, `DEV_APP_SECRET_ARN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, `FRONTEND_DOMAIN` (and optionally the `DEV_*` domain variables)
   - the `production` environment with a required reviewer **and a `main`-only deployment-branch rule** (its OIDC subject carries no branch)
   - delete the old `*_SECRET_ARN`, `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED` variables and the `VITE_COGNITO_BOOTSTRAP_CLIENT_ID` secret
   - the `VITE_MEDIA_BASE_URL`/`VITE_UPLOAD_ORIGINS` secrets ([`operator-setup.md` §6](../docs/operations/operator-setup.md#6-frontend-media-origins))
2. **Deploy dev before 2026-10-18:** `make deploy-dev && make seed && make fe` ([`runbook.md`](../docs/operations/runbook.md)).
3. **Run every pending gate in [`build-order.md`](build-order.md)**, M3 through M13.5, against dev. Push a branch so the CI workflows run for real. For media serving, first check what tests can't: an uploaded image's API URL loads (200, with `Access-Control-Allow-Origin` for the app origin); its host matches the CSP's `img-src` exactly; the same URL without its query string is a 403; the original is gone from the originals bucket once the row is `ready`; a reload serves images from the SW `media-images` cache and avatars from `media-avatars`; a deleted image's URL 403s ([`smoke-test.md`](../docs/operations/smoke-test.md)).
4. **M14 prep:** pick the real prod hostnames in place of the `opennewsletter.example.com` placeholders, check `config.py`'s prod values, fill in the prod secret, then deploy ([`build-order.md`](build-order.md) M14).

Workstation setup for either machine: [`environments.md` §0](../docs/development/environments.md#0-workstation-setup).

### Open owner questions (design review, 2026-10-08)

Not changed in code; each needs the owner's call before or soon after M14. (2 and 4 are settled; the numbers are kept so references stay valid. Originals are deleted once processed: [`decisions.md`](../docs/decisions.md#originals-deleted-on-success).)
1. **Merge the 8 API Lambdas into one?** Fewer cold starts, a shared membership cache, alarms that fit the ≤10 budget and one build artifact, against a broader IAM role (every function already has table read/write).
3. **Cap cost exposure.** Any Google/Facebook user can get a JWT. Reserved concurrency on the API function (easiest after 1) and/or lower per-route throttles.
5. **Raise the Lambda account concurrency quota.** It's 50, shared by dev and prod: fine for launch, too low before inviting many groups.

---

## Owner decisions

Standing decisions, including D1–D12 that code and docs cite by ID, are in [`docs/decisions.md`](../docs/decisions.md) with their reasoning. New ones go there (its last section says how), not here.

---

## Known gaps carried forward

Product and UX:
- A poll pick can't be cleared (`pollOptionId` is required), so a poll pick always auto-publishes at close. Decide whether polls need a "clear" action ([`frontend.md` §7.3](../docs/design/frontend.md#73-respondpage-ggncrespondq)).
- No markdown formatting toolbar in `RespondPage` (spec only requires Write/Preview); no "hype check" banner or recurring-question helper text ([`frontend.md` §14a](../docs/design/frontend.md#14a-client-derived-display-data-not-api-fields)).
- No invite pagination: `GET /admin/groups/{g}/invites` returns everything, including consumed/revoked invites until their TTL (expiry + 30 days).
- `VOTE_CAP_REACHED` lists the already-voted questions in free-text `detail`, not a structured field.
- PWA icons are generated placeholders (`frontend/scripts/generate-icons.mjs`).
- Fonts come from Google Fonts, so they aren't available offline and the CSP allows two Google origins ([`frontend.md` §12.1](../docs/design/frontend.md#121-content-security-policy)).

Frontend:
- `ResponseEditor`'s preview refreshes an expired image URL itself (`refreshPreviewImage`), duplicating the always-mounted `ImageUploader`'s own `refreshImage` for the same thumb. One expired thumb costs two `GET /uploads/{id}`, and the uploader's next `onChange` overwrites whatever the preview fetched. The fix is for `ImageUploader` to expose its refresh to the parent, which changes its props. Do it when the editor is next touched.

Backend and ops:
- The D10 open-cycle close move is best-effort: it runs after the group update, and a write error is logged rather than returned. The 422 check runs before any write.
- A crash mid-fan-out loses the rest of that fan-out (markers are claimed first, at-most-once, [`notifications.md` §7.1](../docs/design/notifications.md#71-algorithm)).
- Comment author names aren't cached across requests (one deduped `BatchGetItem` per request; [`engagement.md` §1.5](../docs/design/engagement.md#15-author-display-name-resolution)); `get_comment_by_id` walks the answer's `C#` range because there's no index on `comment_id`. Revisit if either gets hot.
- `lambda-cycle-tick` needs `#![recursion_limit = "256"]`. If it creeps up, box the dispatch futures.
- The account-level Lambda error alarm also counts dev's errors (one account, [`infrastructure.md` §9.2](../docs/design/infrastructure.md#92-alarms)).
- `CiStack` deploys with every `--all`, including `make deploy-dev`. It's account-wide and idempotent.
- `cargo deny` still ignores one advisory with no upstream fix (`tokio-tar`, test-only via testcontainers) and three unmaintained-crate notices (`rustls-pemfile`, `paste`, `unsafe-libyaml`). Re-check after dependency bumps.

Testing:
- Mock-mode E2E can't cover reload-persistence of saved drafts (the mock transport is in-memory); that's a `dev`-project test.
- The comment composer's image upload and the `[deleted]` placeholder are only exercised in mock mode and component tests until the M10 gate runs.

---

## How to update this file

Keep it short (under ~200 lines). At the end of each milestone:
- update the status table, the snapshot date and the test baselines
- prune cleared blockers, gaps and next steps; add new ones
- add standing owner decisions to [`docs/decisions.md`](../docs/decisions.md), and write each into its owning design page
- append an "Mn detail — what landed" section to [`archive/progress-log.md`](archive/progress-log.md): what landed, bugs found in review, per-milestone decisions (also written into the owning design page), test deltas
