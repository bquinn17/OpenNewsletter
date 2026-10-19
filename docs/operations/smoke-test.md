# Manual smoke runbook

Run this after every deploy to `dev` that touches more than one stack, and after every `prod` deploy ([`testing-and-ci.md` §13](../development/testing-and-ci.md#13-manual-smoke-runbook)). It takes about 15 minutes and two accounts. Tick off each step; if one fails, stop and look at CloudWatch Logs for the Lambda behind it before going further.

The `promote-prod` gate makes **dev first, then prod** the normal order ([D6](../decisions.md#d6)): run this checklist against `dev` before approving the promotion, then run it again against `prod` once that promotion ships. Set `ENV=dev` for the first pass and `ENV=prod` for the second.

Set these once per run (`ENV` is `dev` or `prod`):

```sh
ENV=dev
API=$(jq -r ".\"ApiStack-$ENV\".ApiEndpoint" infra/cdk.out/$ENV-outputs.json)
```

You need:
- **Account A**: a group admin. In `dev` this is the `bootstrap_admin.py` password user (or a Google account already in a group). In `prod` there is no password auth — **Account A is the email the prod admin invite is bound to** (`BOOTSTRAP_ADMIN_EMAIL`, [D5](../decisions.md#d5)), signed in with Google; see step 1 below.
- **Account B**: a second Google account that isn't in that group. Use it from a private window or a second browser.
- A browser that can receive Web Push. Desktop Chrome is the quickest.

---

## 1. Sign in

- [ ] Open the frontend (`make fe` → `http://localhost:5173` for dev, or the prod domain) and sign in as **A**. In `dev`, this can be the `bootstrap_admin.py` password user via `/admin/bootstrap-login`, or Google. In `prod` ([D5](../decisions.md#d5)), there's no password login: the first time, open the `/join?code=…` URL `scripts/bootstrap_admin.py --env prod` printed and sign in with **Google** using `BOOTSTRAP_ADMIN_EMAIL` exactly — the invite is bound to that email and only redeems for a matching, `email_verified` Google sign-in (see [`runbook.md`](runbook.md) "Bootstrap the production admin"). After that first redemption, A signs in with Google normally.
- [ ] Home lists A's group with its gradient. There are no console errors, including no `Content-Security-Policy` violations (prod build only).

## 2. Generate an invite

- [ ] Settings → Your groups → **Manage** → **Invites**. Create a *member* invite with a 7-day TTL.
- [ ] Copy the URL. It starts with the frontend origin, not `opennewsletter.example.com`.

## 3. Redeem it as a second account

- [ ] In the private window, open the invite URL and sign in as **B** with Google.
- [ ] B lands on Home and sees the group.
- [ ] Back as A, under Manage → **Members**, B is listed. Under **Invites**, the invite shows "used by {B's name}".

## 4. Suggest a candidate and vote on it

- [ ] As B, open the group → **Candidates** → **Suggest**. Submit "Smoke test: what did you fix this week?"
- [ ] It appears in the list with 0 votes. Upvote it, and the count goes to 1.
- [ ] As A, the candidate shows 1 vote. Upvote it too, and the count goes to 2.

## 5. Advance the cycle

**dev**: as A (an admin), get an ID token (see [`runbook.md`](runbook.md) "Verify with curl"), then fast-forward the voting cycle into `open`:

```sh
curl -sX POST "$API/admin/dev/tick/cycle" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d "{\"groupId\":\"$GROUP_ID\",\"advanceCycleClosesBy\":\"40d\"}" | jq
```

- [ ] The response lists a `voting → open` transition for the group.
- [ ] Reload the group. The newsletter is open, and the smoke-test candidate is one of its questions.
- [ ] (With push enabled, see step 7.) A cycle-open notification arrives once. Re-running the same curl doesn't send a second one.

**prod**: the dev tick routes return 404, so there's no fast-forward. Invoke the tick Lambda directly to prove it runs clean, then check transitions on the next real 1st of the month:

```sh
aws lambda invoke --function-name OpenNewsletter-CycleTick-$ENV \
  --payload '{}' --cli-binary-format raw-in-base64-out /dev/stdout
```

- [ ] It returns `StatusCode: 200` with no `FunctionError`, and its CloudWatch log shows one structured line per group scanned.

## 6. Draft, publish, comment, react (dev; on prod, during a real open cycle)

- [ ] As B, open the newsletter → answer a question. Type a few lines and attach an image. Wait for the thumbnail, then reload. The draft and the image are still there.
- [ ] Publish the answer, and answer any poll question.
- [ ] Fast-forward to publication (same curl as step 5). The response shows `open → published`, and a new `voting` cycle with `from: null`.
- [ ] Open the published edition. B's answer and image render, loaded from a presigned S3 URL ([`media.md` §4](../design/media.md#4-serving-presigned-s3-gets)).
- [ ] **Media serving** ([`infrastructure.md` §5.2](../design/infrastructure.md#52-image-serving)). `MediaImage` and `Avatar` load in CORS mode, so if the processed bucket's CORS policy is wrong, *every* image and avatar fails to load, not just offline caching. In DevTools → Network, pick the image request and check:
  - the URL's host is the processed bucket's S3 endpoint, and the request returns `200` with `Access-Control-Allow-Origin` set to this app's exact origin, never `*`, and no `Access-Control-Allow-Credentials` header (the signature is in the URL, so there are no credentials to send)
  - pasting the same URL with its query string (`?X-Amz-...`) stripped returns `403` — confirms the bucket really is private and the signature is doing the work
  - the Console shows no "blocked by CORS policy" errors

  Then, under Application → Cache Storage → `media-images`, the image has an entry (avatars go in `media-avatars`). A broken image here is a CORS failure. Check that the processed bucket's CORS rule includes the SPA's origin exactly (scheme, host, port, no trailing slash). The quickest way to get images back is a frontend-only rollback that removes `crossOrigin` from `MediaImage` and `Avatar`.
- [ ] **Original cleanup.** In the S3 console, the media-originals bucket's `uploads/` prefix no longer has an object for the image just uploaded — `lambda-image-process` deletes it once the row is `ready` ([`media.md` §5.1](../design/media.md#51-trigger-configuration)).
- [ ] **Delete 403s immediately.** Copy the image's `displayUrl`. Delete the image from the response editor (or `DELETE /uploads/{id}` directly), then request the copied URL again: it now returns `403` — there's no invalidation delay to wait out ([`media.md` §8.0](../design/media.md#80-explicit-deletion)).
- [ ] As A, comment on B's answer, then edit the comment. As B, reload: the edited comment shows.
- [ ] React with 👍. Toggle it off, then react with 👨‍👩‍👧‍👦. The counts update and survive a reload.

## 7. Push

- [ ] As A, open Settings → Notifications → **Enable**, and allow the browser prompt.
- [ ] **Send test push**: an OS notification arrives within a few seconds. **My devices** lists this browser.
- [ ] Click the notification. It opens the app.

## 8. Back-end health

- [ ] CloudWatch → Dashboards → `OpenNewsletter-$ENV`: the API, Lambda and DynamoDB widgets show the traffic from this run.
- [ ] CloudWatch → Alarms: nothing is in `ALARM`. (On first deploy, confirm the SNS email subscription for `ALARM_EMAIL` was accepted.)
- [ ] Metrics → `OpenNewsletter/$ENV` has `PushSent` data points from step 7.

## Clean up (dev only)

- [ ] `make seed` to reset fixture data, or leave the data if the next task needs it.
- [ ] As A, under Manage → Members, remove B so the next run starts from a fresh redeem.
