# Progress & Blockers

Snapshot as of 2026-10-12: M0–M13.5 code-complete plus the design-review fixes, nothing deployed, M14 next. This file is read at the start of every session, so it holds only the current state. How the system works is documented in [`docs/`](../docs/README.md). Milestone definitions and gates are in [`build-order.md`](build-order.md). What each milestone landed (bugs found, per-milestone decisions, test deltas) is in [`archive/progress-log.md`](archive/progress-log.md).

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
| M14 | Production deploy | ⬜ **next** |

**Test baselines (2026-10-12):** backend 505 (`cargo test --workspace`, last full green run), CDK 158 (`pytest infra/tests`), Vitest 334, Playwright 8 (mock project). fmt, clippy `-D warnings`, `cargo deny`, ruff, `mypy --strict`, typecheck, ESLint (0 problems), Prettier, `vite build`, `actionlint` and `cdk synth` (dev + prod) are clean.

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

- **macOS:** all 12 artifacts in `backend/target/lambda/` were built 2026-10-11/12, but they predate the last M13.5 fixes and the design review. Run `make build-lambdas` right before any deploy (`make deploy-dev` does). `ApiStack`/`NotificationsStack` silently fall back to a shell stub when a binary is missing; `deploy.yml` guards against that with `scripts/check_lambda_binaries.sh`.
- **Windows/WSL2:** release builds abort in `aws-lc-sys` under gcc 9 (gcc bug 95189). Fix with `sudo apt install clang && export CC=clang`, or `cargo-lambda`'s zig toolchain. Debug builds and tests are unaffected.
- `backend-ci`'s first run is also the first Linux build of `image-process` (`libwebp-sys`).

---

## Next steps (in order)

1. **Operator:** clear B5's open items; set `BOOTSTRAP_ADMIN_EMAIL`; configure GitHub per `.github/workflows/README.md`:
   - repo variables `DEPLOY_ENABLED`, `DEV_APP_SECRET_ARN`, `APP_SECRET_ARN`, `BOOTSTRAP_ADMIN_EMAIL`, `FRONTEND_DOMAIN` (and optionally the `DEV_*` domain variables)
   - the `production` environment with a required reviewer **and a `main`-only deployment-branch rule** (its OIDC subject carries no branch)
   - delete the old `*_SECRET_ARN`, `FRONTEND_DEPLOY_ENABLED`/`BACKEND_DEPLOY_ENABLED` variables and the `VITE_COGNITO_BOOTSTRAP_CLIENT_ID` secret
2. **Deploy dev before 2026-10-18:** `make deploy-dev && make seed && make fe` ([`runbook.md`](../docs/operations/runbook.md)).
3. **Run every pending gate in [`build-order.md`](build-order.md)**, M3 through M13.5, against dev. Push a branch so the CI workflows run for real.
4. **M14 prep:** pick the real prod hostnames in place of the `opennewsletter.example.com` placeholders, check `config.py`'s prod values, merge the prod secret, then deploy ([`build-order.md`](build-order.md) M14).

Workstation setup for either machine: [`environments.md` §0](../docs/development/environments.md#0-workstation-setup).

### Open owner questions (design review, 2026-10-08)

Not changed in code; each needs the owner's call before or soon after M14.
1. **Merge the 8 API Lambdas into one?** Fewer cold starts, a shared membership cache, alarms that fit the ≤10 budget and one build artifact, against a broader IAM role (every function already has table read/write).
2. **Keep originals after processing?** They're the storage that keeps growing and they retain EXIF/GPS. Options: delete on success, or expire after N days, unless reprocessing at new sizes is wanted ([`infrastructure.md` §12](../docs/design/infrastructure.md#12-cost-expectations)).
3. **Cap cost exposure.** Any Google/Facebook user can get a JWT. Reserved concurrency on the API function (easiest after 1) and/or lower per-route throttles.
4. **Omit `policy`/`signature` from the prod `/media-cookie` body.** The HttpOnly cookie is moot while the body carries them; a contract change, so expand → contract.
5. **Raise the Lambda account concurrency quota.** It's 50, shared by dev and prod: fine for launch, too low before inviting many groups.

---

## Owner decisions

Standing decisions, including D1–D12 that code and docs cite by ID, are in [`docs/decisions.md`](../docs/decisions.md) with their reasoning. New ones go there (its last section says how), not here.

---

## Known gaps carried forward

Product and UX:
- The fixed `BottomNav` covers RespondPage's bottom bar ("Save & exit", "Publish") at 700px; both are `fixed bottom-3 z-40`. The e2e test works around it with `goBack()`.
- A poll pick can't be cleared (`pollOptionId` is required), so a poll pick always auto-publishes at close. Decide whether polls need a "clear" action ([`frontend.md` §7.3](../docs/design/frontend.md#73-respondpage-ggncrespondq)).
- No markdown formatting toolbar in `RespondPage` (spec only requires Write/Preview); no "hype check" banner or recurring-question helper text ([`frontend.md` §14a](../docs/design/frontend.md#14a-client-derived-display-data-not-api-fields)).
- No invite pagination: `GET /admin/groups/{g}/invites` returns everything, including consumed/revoked invites until their TTL (expiry + 30 days).
- `VOTE_CAP_REACHED` lists the already-voted questions in free-text `detail`, not a structured field.
- PWA icons are generated placeholders (`frontend/scripts/generate-icons.mjs`).
- Fonts come from Google Fonts, so they aren't available offline and the CSP allows two Google origins ([`frontend.md` §12.1](../docs/design/frontend.md#121-content-security-policy)).

Backend and ops:
- The D10 open-cycle close move is best-effort: it runs after the group update, and a write error is logged rather than returned. The 422 check runs before any write.
- A crash mid-fan-out loses the rest of that fan-out (markers are claimed first, at-most-once, [`notifications.md` §7.1](../docs/design/notifications.md#71-algorithm)).
- Comment author names aren't cached across requests (one deduped `BatchGetItem` per request; [`engagement.md` §1.5](../docs/design/engagement.md#15-author-display-name-resolution)); `get_comment_by_id` walks the answer's `C#` range because there's no index on `comment_id`. Revisit if either gets hot.
- `lambda-cycle-tick` needs `#![recursion_limit = "256"]`. If it creeps up, box the dispatch futures.
- The account-level Lambda error alarm also counts dev's errors (one account, [`infrastructure.md` §9.2](../docs/design/infrastructure.md#92-alarms)).
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
- add standing owner decisions to [`docs/decisions.md`](../docs/decisions.md), and write each into its owning design page
- append an "Mn detail — what landed" section to [`archive/progress-log.md`](archive/progress-log.md): what landed, bugs found in review, per-milestone decisions (also written into the owning design page), test deltas
