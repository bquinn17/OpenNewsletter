# Progress & Blockers

Snapshot as of 2026-10-26: M0–M13.5 code-complete plus the design-review fixes and presigned-S3 media serving (with its code-review fixes). Dev is deployed (2026-10-18), but runs that day's build, so it's behind `main`. Of the deploy-shaped gates, M3's deploy and M7 are confirmed. M14 next. This file is read at the start of every session, so it holds only the current state. How the system works is documented in [`docs/`](../docs/README.md). Milestone definitions and gates are in [`build-order.md`](build-order.md). What each milestone landed (bugs found, per-milestone decisions, test deltas) is in [`archive/progress-log.md`](archive/progress-log.md).

---

## Milestone status

| # | Milestone | Status |
|---|---|---|
| M0 | Repo skeleton | ✅ done |
| M1 | Operator prerequisites | 🟡 partial — see B5 |
| M2 | Domain + persistence | ✅ done |
| M3–M13 | Infra → Hardening | 🟡 code-complete; dev deployed. Confirmed against dev: M3 (deploy), M7. Every other deploy-shaped gate ([`build-order.md`](build-order.md)) is still to run |
| M13.5 | Plan-review follow-ups (D1–D12) | ✅ code-complete; deploy-shaped checks join M14 |
| — | Design review: performance and cost (2026-10-08) | ✅ code-complete; open questions below |
| — | Media serving via presigned S3 (2026-10-14; code-review fixes 2026-10-18) | ✅ code-complete; deploy checks join M14 (below) |
| M14 | Production deploy | ⬜ **next** |

**Test baselines (2026-10-29):** backend 496 (`cargo test --workspace`), CDK 153 (`pytest infra/tests`), Vitest 387, Playwright 14 (8 `mock`, 6 `demo`). fmt, clippy `-D warnings`, ruff, `mypy --strict` (`infra/`, plus `scripts/` run by hand, since CI doesn't cover it), typecheck, ESLint (0 problems), Prettier, `vite build`, `actionlint`, `cdk synth` (dev + prod) and `cargo deny check` are clean. For `cargo deny`, `lambda-push` moved to `p256` 0.14 and S3 dropped its unused `sigv4a` feature, the only two paths to the vulnerable `crypto-bigint` 0.5 (RUSTSEC-2026-0387).

---

## Active blockers

### B5 — M1 operator tasks outstanding

Done: domain `betterloop.site` (Namecheap DNS, no Route53 zone, so ACM validation CNAMEs are added at Namecheap by hand); Cognito domain prefix; `aws login` as IAM user `bquinn17` (not root); `cdk bootstrap`; VAPID keys; the combined `opennewsletter/dev` secret, whose Google credentials a real sign-in has proven; `ALARM_EMAIL`. The old per-credential dev secrets are deleted.

Still open:
- **Confirm Facebook sign-in** against dev: the app ID/secret in `opennewsletter/dev` and the redirect `https://<prefix>.auth.us-east-1.amazoncognito.com/oauth2/idpresponse` on the Facebook app. M13.5's Facebook `email_verified` check needs it anyway.
- **Fill in `opennewsletter/prod`** with the OAuth and VAPID keys: write the full key set with `aws secretsmanager put-secret-value` (M14, [`runbook.md`](../docs/operations/runbook.md)).
- **`.env.local` overrides environment variables for prod too** (`load_config`), so a workstation prod synth/deploy would pick up the dev `APP_SECRET_ARN`. Make it dev-only or override it before a local prod bring-up (swap `APP_SECRET_ARN`/`ROOT_DOMAIN`/`API_DOMAIN` with the `PROD_*`-prefixed lines next to them, then swap back). CI has no `.env.local`.
- `aws login` sessions expire. Check `aws sts get-caller-identity` before deploying.

**Decision (2026-10-13) — dev gets its own subdomain (`dev.betterloop.site`/`api.dev.betterloop.site`), not prod's apex.** `config.py` has no local dev/prod split for `ROOT_DOMAIN`/`API_DOMAIN`, and the ACM cert covers both names in every env, so dev pointed at the apex would put prod's hostnames on the dev cert. Set in `.env.local`; full writeup in [`docs/development/environments.md` §2](../docs/development/environments.md#2-environments).

These block M14 (and the Facebook checks), not code work or the other dev gates.

### B6 — Lambda release builds

- **macOS:** builds fine; `make deploy-dev` rebuilds every artifact. A manual `cdk deploy` needs `make build-lambdas` first: `ApiStack`/`NotificationsStack` silently fall back to a shell stub when a binary is missing (`make deploy-dev` and `deploy.yml` both run `scripts/check_lambda_binaries.sh` to catch that).
- **Windows/WSL2:** release builds abort in `aws-lc-sys` under gcc 9 (gcc bug 95189). Fix with `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain. Debug builds and tests are unaffected.
- `backend-ci`'s first run is also the first Linux build of `image-process` (`libwebp-sys`).

---

## Next steps (in order)

1. **Redeploy dev:** `make deploy-dev`. Dev's stacks were last deployed 2026-10-19 02:03 UTC, before `d2ef568` changed the backend, `ApiStack`'s routes and `MediaPipelineStack`'s IAM.
2. **Operator, GitHub** ([`.github/workflows/README.md`](../.github/workflows/README.md)). Done: the repo variables, both `AWS_*_ROLE_ARN`s, the `production` environment with a required reviewer and a `main`-only branch rule, and the old variables are gone. Still to do:
   - add the seven prod `VITE_*` secrets. None exist yet, and `promote-prod` stops before the frontend build without them ([`operator-setup.md` §6](../docs/operations/operator-setup.md#6-frontend-media-origins) has the two media ones).
   - `production` holds `APP_SECRET_ARN` and `DEV_APP_SECRET_ARN` as environment *secrets*. `deploy.yml` reads both as repo variables (`vars.*`), so they're unused, and the `DEV_` one doesn't belong in prod. Delete them unless they're meant for something.
   - set `DEPLOY_ENABLED=true` once CI is green.
3. **Run every pending gate in [`build-order.md`](build-order.md)** against dev: M3.5, M4–M6 and M8–M13.5 (M3's deploy and M7 are done). Push a branch so the CI workflows run for real. For media serving, first check what tests can't: an uploaded image's API URL loads (200, with `Access-Control-Allow-Origin` for the app origin); its host matches the CSP's `img-src` exactly; the same URL without its query string is a 403; the original is gone from the originals bucket once the row is `ready`; a reload serves images from the SW `media-images` cache and avatars from `media-avatars`; a deleted image's URL 403s ([`smoke-test.md`](../docs/operations/smoke-test.md)).
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
- `apiFetch` treats every 401 as an expired token: it renews the session and retries once. The server also returns 401 `UNAUTHENTICATED` for a valid token whose caller hasn't redeemed an invite (`require_user_id`), so those calls cost a pointless refresh-token round trip and log two 401s. Settings avoids it by enabling `useMe` only once `/config` has a `userId` ([`frontend.md` §7.6](../docs/design/frontend.md#76-settingspage)); any other user-scoped call made before an invite is redeemed still hits it. Fix by giving that case its own status or code, or by renewing only when the ID token has actually expired.
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
- `make seed` resets the bootstrap admin's password to a new random one on every run, so `E2E_USERNAME`/`E2E_PASSWORD` for the `dev` E2E project can't be the seeded admin. Use a separate Cognito user, or seed with `--admin-password` taken from the same secret.
- Mock-mode E2E can't cover reload-persistence of saved drafts (the mock transport is in-memory); that's a `dev`-project test.
- The comment composer's image upload and the `[deleted]` placeholder are only exercised in mock mode and component tests until the M10 gate runs.

---

## How to update this file

Keep it short (under ~200 lines). At the end of each milestone:
- update the status table, the snapshot date and the test baselines
- prune cleared blockers, gaps and next steps; add new ones
- add standing owner decisions to [`docs/decisions.md`](../docs/decisions.md), and write each into its owning design page
- append an "Mn detail — what landed" section to [`archive/progress-log.md`](archive/progress-log.md): what landed, bugs found in review, per-milestone decisions (also written into the owning design page), test deltas
