# Progress & Blockers

Where the project stands, what's blocking it, and what's next. It's read at the start of every session, so it stays short and high level. Anything the repo, CI or the deployment already shows (test results, what's deployed, when something landed) is left out. How the system works is in [`docs/`](../docs/README.md); milestone definitions and their gates are in [`build-order.md`](build-order.md); what each finished milestone landed is in [`archive/progress-log.md`](archive/progress-log.md).

---

## Status

- **M0–M13.5 are code-complete**, along with the design-review fixes, presigned-S3 media serving, the configurable cycle cadence, the group leaderboard and the app's own sign-in pages.
- **Dev runs the latest `main`.** Every push to `main` deploys it ([D6](../docs/decisions.md#d6)).
- **Deploy-shaped gates:** the API side of every dev gate has passed; [`build-order.md`](build-order.md) marks each check. What's left needs a browser, a second Google account or a phone (next step 3).
- **M1** is partly done (B5).
- **M14, the production deploy, is next.**

---

## Active blockers

### B5 — M1 operator tasks outstanding

Done:
- the domain, `betterloop.site`, whose DNS is at Namecheap, so ACM validation records are added there by hand
- the Cognito domain prefix and `cdk bootstrap`
- VAPID keys and the dev secret, whose Google sign-in is proven
- `ALARM_EMAIL`

Still open:
- **Confirm Facebook sign-in** against dev: the app ID and secret in `opennewsletter/dev`, and the Cognito `idpresponse` redirect on the Facebook app. M13.5's Facebook `email_verified` check needs it.
- **Fill in `opennewsletter/prod`** with the OAuth and VAPID keys ([`runbook.md`](../docs/operations/runbook.md) M14).
- **`.env.local` also overrides prod config** (`load_config`), so a prod synth or deploy from a workstation picks up dev values. Before a local prod bring-up, swap in the `PROD_*` lines. CI has no `.env.local`.

These block M14 and the Facebook checks, not code work or the other dev gates.

### B6 — Lambda release builds on WSL2

Release builds abort in `aws-lc-sys` under gcc 9 (gcc bug 95189). Fix with `sudo apt install clang && export CC=clang`, or use `cargo-lambda`'s zig toolchain. Debug builds, tests and CI are unaffected. On any machine, run `make build-lambdas` before a manual `cdk deploy`: a missing binary silently deploys a stub (`make deploy-dev` and `deploy.yml` both run `scripts/check_lambda_binaries.sh` to catch it).

---

## Next steps (in order)

1. **Don't push `release` until M14 is ready.** A push to `release` deploys prod ([D6](../docs/decisions.md#d6)). Until then, nothing reaches prod.
2. **Operator setup on GitHub** ([`.github/workflows/README.md`](../.github/workflows/README.md)). `release.yml` stops early until it's done:
   - add the prod `VITE_*` secrets ([`operator-setup.md` §6](../docs/operations/operator-setup.md#6-frontend-media-origins) covers the media ones)
   - delete the `production` environment's `APP_SECRET_ARN` and `DEV_APP_SECRET_ARN` secrets. The workflows read those names as repo variables, so the secrets are unused.
   - wire up the prod Pages repo, [`bquinn17/betterloop-site`](https://github.com/bquinn17/betterloop-site) ([`operator-setup.md` §10](../docs/operations/operator-setup.md#10-prod-frontend-pages-repo) steps 2–5)
3. **Finish the unmarked gates in [`build-order.md`](build-order.md) against dev.** If `scripts/backfill_member_stats.py --env dev` hasn't been run since the leaderboard deployed, run it first, while the group is quiet: until it runs, the leaderboard and `editionsAnswered` count only what happened since that deploy ([`scripts/README.md`](../scripts/README.md)). The unmarked gates are the UI and device checks: uploading and the media CORS check in a browser (M8), the simulated month (M9), push on Chrome and an installed iOS PWA (M11), the rename showing without a reload (M12), `make redeploy-volatile` (M3.5), the full [`smoke-test.md`](../docs/operations/smoke-test.md), and the cadence settings pass in [`frontend-walkthrough.md`](../docs/operations/frontend-walkthrough.md) §16.2.
4. **M14:** check `config.py`'s prod values, fill in the prod secret, then deploy ([`build-order.md`](build-order.md) M14) and run `scripts/backfill_member_stats.py --env prod` once. The prod hostnames are already set as repo variables.
5. **Address the [frontend design review](../docs/frontend_design_review.md).** Fix its §1 defects before M14, then follow its §7 order. Each step updates [`frontend.md`](../docs/design/frontend.md) in the same change. The rest of the review doesn't block M14.

Workstation setup for either machine: [`environments.md` §0](../docs/development/environments.md#0-workstation-setup).

### Open owner questions

These came out of the performance and cost review ([archived](archive/design-review-performance-cost.md)). Each needs the owner's call before or soon after M14. Questions 2 and 4 are settled; the numbers are kept so references stay valid.
1. **Merge the API Lambdas into one?** That gives fewer cold starts, a shared membership cache, alarms that fit the ≤10 budget and a single build artifact. The cost is one broader IAM role, though every function already has table read/write.
3. **Cap cost exposure.** Any Google or Facebook user can get a JWT. Options are reserved concurrency on the API function (easiest after 1), lower per-route throttles, or both. Rejecting sign-ups without an invite, or deleting accounts that don't redeem one within 24 hours, would also limit it ([`future-ideas.md` §15](../docs/future-ideas.md#15-invite-only-accounts-enforced-at-sign-up)).
5. **Raise the Lambda account concurrency quota.** Dev and prod share it. It's enough for launch, but too low before inviting many groups.

---

## Owner decisions

Standing decisions, including D1–D12, are in [`docs/decisions.md`](../docs/decisions.md) with their reasoning. New ones go there, not here.

---

## Known gaps carried forward

Product and UX:
- A poll pick can't be cleared, so it always auto-publishes at close ([`frontend.md` §7.3](../docs/design/frontend.md#73-respondpage-ggncrespondq)).
- `RespondPage` has no markdown toolbar, "hype check" banner or recurring-question helper text ([`frontend.md` §14a](../docs/design/frontend.md#14a-client-derived-display-data-not-api-fields)).
- No invite pagination: `GET /admin/groups/{g}/invites` returns every invite, including used and revoked ones, until their TTL.
- `VOTE_CAP_REACHED` lists the already-voted questions in free-text `detail`, not in a structured field.
- The PWA icons are generated placeholders.
- Fonts come from Google Fonts, so they aren't available offline ([`frontend.md` §12.1](../docs/design/frontend.md#121-content-security-policy)).

Frontend:
- `apiFetch` treats every 401 as an expired token. A valid token from a caller who hasn't redeemed an invite also gets a 401, so those calls cost a pointless token refresh ([`frontend.md` §7.6](../docs/design/frontend.md#76-settingspage)).
- `ResponseEditor`'s preview and `ImageUploader` each refresh an expired image URL themselves, so one expired thumbnail costs two requests. Fix this the next time the editor is touched.

Backend and ops:
- A crash mid-fan-out loses the rest of that push fan-out, because markers are claimed first ([`notifications.md` §7.1](../docs/design/notifications.md#71-algorithm)).
- Comment author names aren't cached across requests, and `get_comment_by_id` has no index ([`engagement.md` §1.5](../docs/design/engagement.md#15-author-display-name-resolution)). Revisit either if it gets hot.
- `lambda-cycle-tick` needs a raised `recursion_limit`. If it creeps up, box the dispatch futures.
- The account-level Lambda error alarm also counts dev's errors ([`infrastructure.md` §9.2](../docs/design/infrastructure.md#92-alarms)).
- On dev, `seed_dev_data.py` and `bootstrap_admin.py` derive different user IDs for the same Cognito user. Running the bootstrap with the seeded admin's email moves that login to a new user that isn't in the seeded group. Use a different email for now ([`scripts/README.md`](../scripts/README.md)). The fix is for the bootstrap to reuse an existing user ID.

Testing:
- `make seed` resets the seeded admin's password on every run, so the `dev` E2E project needs its own Cognito user (`bootstrap_admin.py --env dev --password` makes one), or a seed with a fixed `--admin-password`. Its tests (`frontend/e2e/dev/`) are still `fixme`.
- Draft reload-persistence, the comment composer's image upload and the `[deleted]` placeholder are checked on dev through the API, but their UI is only covered in mock mode.

---

## How to update this file

Keep it short and high level. When a milestone finishes:
- update Status, and prune cleared blockers, gaps and next steps; add new ones
- add standing owner decisions to [`docs/decisions.md`](../docs/decisions.md), and write each into its owning design page
- append what landed (bugs found in review, per-milestone decisions) to [`archive/progress-log.md`](archive/progress-log.md)

Don't record what the repo or CI already shows: test counts, dates, commit IDs or what's deployed.
