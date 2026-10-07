# Manual smoke runbook

Run this after every deploy to `dev` that touches more than one stack, and after every `prod` deploy (`plans/11-testing-ci-cd.md` §13). It takes about 15 minutes and two accounts. Tick off each step; if one fails, stop and look at CloudWatch Logs for the Lambda behind it before going further.

Set these once per run (`ENV` is `dev` or `prod`):

```sh
ENV=dev
API=$(jq -r ".\"ApiStack-$ENV\".ApiEndpoint" infra/cdk.out/$ENV-outputs.json)
```

You need:
- **Account A**: a group admin, either the `bootstrap_admin.py` user or a Google account already in a group.
- **Account B**: a second Google account that isn't in that group. Use it from a private window or a second browser.
- A browser that can receive Web Push. Desktop Chrome is the quickest.

---

## 1. Sign in

- [ ] Open the frontend (`make fe` → `http://localhost:5173` for dev, or the prod domain) and sign in as **A** with Google.
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

**dev**: as A (an admin), get an ID token (`docs/RUNBOOK.md` §M4 "Verify with curl"), then fast-forward the voting cycle into `open`:

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
- [ ] Open the published edition. B's answer and image render, and the image loads from the CDN origin.
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
