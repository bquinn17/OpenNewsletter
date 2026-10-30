# Frontend walkthrough

A full manual pass over every user flow in the SPA, for verification before a release or after a large frontend change. It takes two to three hours. It's the long form of [`smoke-test.md`](smoke-test.md): the smoke test checks that a deploy works end to end in 15 minutes, while this page checks that every screen, state and error path behaves as designed.

Run it against the `dev` stack. The steps follow the cycle in order (sign in, join, suggest, vote, open, answer, close, read, react, comment), so later sections rely on data that earlier ones create. Admin, settings and PWA checks come after the cycle. Each step is a checkbox. Where the expected result includes on-screen text, it's quoted exactly so you can match it. If a step fails, note the URL, the account, what you saw, and the `x-correlation-id` header of the failing request (DevTools → Network), then carry on where you can.

Section 21 lists behaviour that is expected in the current build, even though it may look like a bug. Read it before you start.

---

## 0. Before you start

### 0.1 What you need

- The dev stack deployed, with `frontend/.env.dev` written by `make deploy-dev` ([`runbook.md`](runbook.md)).
- AWS credentials for the dev account. `aws sts get-caller-identity` should succeed.
- Node 24 (`nvm use` in `frontend/`), `jq` and `curl`.
- Desktop Chrome, with **three Chrome profiles** (Chrome menu → Profiles → Add): one each for accounts A, B and C. Each profile keeps its own tokens, service worker and push subscription. Don't use Incognito for an account you'll test push with, because Chrome blocks notifications there.
- Optional: an iPhone and an Android phone for the PWA checks in §18.
- Test files: a small JPEG, a PNG, a WebP, an animated GIF, a JPEG over 15 MB, a JPEG between 5 and 15 MB, and a non-image such as a PDF.
- macOS: allow Chrome to show notifications (System Settings → Notifications → Google Chrome), and turn Focus off.

### 0.2 Accounts

| Account | Who | How to sign in |
|---|---|---|
| **A** | The seeded bootstrap admin, "Dev Admin" | `/admin/bootstrap-login` with the email and password `make seed` prints |
| **B** | A second member | A Google account, at least once per pass, because that's the only way to exercise the invite surviving the OAuth redirect (§2.2). A password user also works for the other steps. |
| **C** | A third account, used for invite edge cases | A Cognito password user (below) |

To create a password user for B or C:

```sh
POOL=$(jq -r '."AuthStack-dev".UserPoolId' infra/cdk.out/dev-outputs.json)
EMAIL=tester-c@example.com
aws cognito-idp admin-create-user --user-pool-id "$POOL" --username "$EMAIL" \
  --user-attributes Name=email,Value="$EMAIL" Name=email_verified,Value=true Name=name,Value="Tester C" \
  --message-action SUPPRESS
aws cognito-idp admin-set-user-password --user-pool-id "$POOL" --username "$EMAIL" \
  --password '<choose one>' --permanent
```

Password users sign in at `http://localhost:5173/admin/bootstrap-login`. The Cognito hosted UI only offers Google and Facebook.

### 0.3 Reset the data

```sh
make seed
```

This wipes the dev table and media buckets, keeps Cognito users, and writes one group:

- "Dev Group", timezone `America/New_York`, gradient `grape-sky`
- 5 questions per cycle, 3 votes per member, a 7-day response window
- reminders at 96, 48 and 24 hours before close, cycle-open notifications on
- a member soft cap of 50
- A as its only member and admin
- one `voting` cycle that opens in about 27 days

`make seed` resets A's password to a new random one on every run and prints it. To keep a fixed password:

```sh
. infra/.venv/bin/activate && python scripts/seed_dev_data.py --env dev --admin-password '<yours>'
```

### 0.4 Serve a production build on port 5173

Run the walkthrough against a production build, not `make fe`. The Vite dev server doesn't register the service worker, so push, the install prompt, media caching and the CSP can't be tested there. The push toggle also never finishes under the dev server. The build must be served on `localhost:5173`, because that's the only local origin the dev stack's API CORS, Cognito callback URLs and media bucket CORS allow.

```sh
cd frontend
npm run build -- --mode dev            # reads frontend/.env.dev
npx vite preview --port 5173 --strictPort
```

Stop `make fe` first if it's running. To pick up a code change, rebuild and restart the preview. The new service worker activates straight away, and the page uses the new build on its next reload.

### 0.5 Fast-forwarding the cycle

The dev stack has admin-only routes that run the cycle tick and the notify tick on demand ([`api.md` §11a](../design/api.md#11a-dev-only-endpoints)). Set up a terminal once per pass:

```sh
API=$(jq -r '."ApiStack-dev".ApiEndpoint' infra/cdk.out/dev-outputs.json)
GROUP_ID=<the id in the URL after /g/>
TOKEN=<A's ID token, see below>

advance() {   # rewind the group's active cycle by $1 (e.g. 40d, 2d, 6h), then run the cycle tick
  curl -s -X POST "$API/admin/dev/tick/cycle" -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"groupId\":\"$GROUP_ID\",\"advanceCycleClosesBy\":\"$1\"}" | jq
}
notify() {    # run the deadline-reminder tick
  curl -s -X POST "$API/admin/dev/tick/notify" -H "Authorization: Bearer $TOKEN" | jq
}
```

To get A's ID token, sign in as A on `localhost:5173`, open the DevTools console and run:

```js
copy(JSON.parse(localStorage.getItem(Object.keys(localStorage).find((k) => k.startsWith("oidc.user:")))).id_token)
```

The token is now on your clipboard. It expires after an hour; copy a fresh one when `curl` returns 401.

`advance` rewinds the **open** cycle's close if the group has one, otherwise the **voting** cycle's open time. The response's `transitions` array lists each state change, for example `{"from": "voting", "to": "open"}`. A newly created cycle shows `"from": null`. An empty array means the rewind wasn't far enough, so run it again with a larger duration.

### 0.6 Things to keep in mind

- **Reload to see another account's changes.** The app doesn't refetch on window focus, and caches group data for up to 5 minutes. When a step says "as A, check …" after B did something, reload A's page first.
- **Dates on group pages are in the group's timezone**, not your browser's. Dev Group is `America/New_York`.
- Keep DevTools open on the Console for the whole pass. Any red error, React warning or `Content-Security-Policy` violation is worth noting.

---

## 1. Signed-out pages and sign-in

- [ ] As a signed-out visitor (a fresh profile, or after signing out), open `/`. It shows "Your group's newsletter, together.", "Sign in to see your group's newsletters.", a **Sign in** button, and a card "Have an invite code?" with an invite code field. The header shows the "O" logo and a **Sign in** button.
- [ ] Open `/settings` while signed out. You're sent straight to the Cognito hosted UI, which offers Google (and Facebook, if configured). Type `localhost:5173` in the address bar to leave it. §2.2 checks that sign-in returns you to the page you asked for.
- [ ] Open `/admin/bootstrap-login`. Submit it empty: "Username is required" and "Password is required". Submit a wrong password: Cognito's error message appears under the fields. Submit A's real credentials: the button shows "Signing in…", then Home loads with "Hey Dev Admin 👋".
- [ ] In B's profile, sign in with Google (B isn't in any group yet). `/auth/callback` shows a spinner, then Home says "You haven't joined a group yet — paste your invite code below." with the join form. Open `/settings`: you're redirected back to Home. The avatar menu still offers **Sign out**. Sign B out.
- [ ] Optional, if the Facebook IdP is configured: sign in once with Facebook the same way.
- [ ] Open `/auth/callback` directly with no query string. It shows "Something went wrong finishing sign-in." and a **Try signing in again** button.
- [ ] Reload any page while signed in: you stay signed in. Close the tab, open the app again: still signed in.
- [ ] Optional, needs an hour: leave a signed-in tab idle for more than an hour, then do something that calls the API (open an edition). It works without a sign-in prompt, because the refresh token renews the session.
- [ ] Avatar menu (top right) → **Sign out**. A "Signing out…" overlay shows, then Cognito's logout, then the signed-out Home. In DevTools → Application → Local Storage, no `oidc.user:` key remains.
- [ ] Sign back in as A, then use the **Sign out** button at the bottom of Settings. Same result.

## 2. Invites and joining

### 2.1 Creating invites (A)

- [ ] As A: avatar menu → **Settings** → Your groups → **Manage** on Dev Group. The admin page opens with eyebrow "Admin", title "Dev Group", and tabs Members, Invites and Settings.
- [ ] **Invites** tab. "No invites yet." The form has Role (Member, Admin) and Expires in (1, 3, 7, 14, 30, 90 days), with Member and 7 days selected.
- [ ] **Create invite** (shows "Creating…"). An "Invite created" panel shows the link `http://localhost:5173/join?code=…`. Clicking the field selects it. **Copy** shows the toast "Invite link copied." and the clipboard holds the URL.
- [ ] The list shows the code in monospace, "member · expires in 7 days", and **Copy** and **Revoke** actions. **Dismiss** hides the panel; the row stays.
- [ ] Create a second member invite and **Revoke** it. Toast "Invite revoked."; the row greys out and reads "revoked". Keep the code for §2.4.
- [ ] Create four more invites for later: a Member invite each for B and C, a spare Member invite, and an Admin invite. Active invites list first, newest first.

### 2.2 Joining through the OAuth redirect (B, Google)

- [ ] In B's profile, signed out, open B's invite URL. You're sent straight to the Cognito hosted UI.
- [ ] Sign in with Google. You come back through `/auth/callback` and `/join?code=…`, then see the toast "Welcome to Dev Group" and land on `/g/<GROUP_ID>/upcoming`. The header's group switcher shows "Dev Group" with its gradient.
- [ ] As A, reload Admin → **Members**: B is listed with "joined … ago · pending first answer", and the count reads "2 of 50 members". Under **Invites**, B's code reads "used by {B's name} · … ago".
- [ ] Sign B out, then open `/g/<GROUP_ID>/upcoming`. You're sent to the hosted UI, and after signing in with Google you land back on Upcoming, not Home.

### 2.3 Joining while signed in (C), and the member cap

- [ ] As C, sign in at `/admin/bootstrap-login`. Home shows the join form. Submit it empty: "Enter an invite code".
- [ ] As A, set Admin → Settings → **Member soft cap** to 2 (the current count) and save.
- [ ] As C, type C's code in lowercase and press **Join**. The URL shows the code upper-cased, and the page says "Couldn't join" and "That group is full."
- [ ] Enter the same code and press **Join** again: a spinner, then "That group is full." again. It retried rather than doing nothing.
- [ ] As A, set the cap back to 50. As C, submit the same code once more: "Welcome to Dev Group" and the Upcoming page.

### 2.4 Invite errors

Use C (signed in) for each, pasting into Settings → Your groups → **Join with an invite code** (`/join`, "Join a group"), or opening `/join?code=…` directly.

- [ ] A made-up code such as `ABCD-EFGH`: "Couldn't join" and "That invite code isn't valid." The join form is shown again below.
- [ ] The revoked code from §2.1: "That invite code isn't valid."
- [ ] B's code, already used: "That invite has already been used."
- [ ] The spare Member invite, redeemed by B, who is already a member: still "Welcome to Dev Group" and the Upcoming page. As A, that invite is still active (an existing member doesn't consume it).
- [ ] Optional: an expired invite. Create a 1-day invite and come back after 24 hours. A's list shows it greyed out as "expired". Redeeming it shows "That invite has expired — ask for a new one."

## 3. App shell and navigation

- [ ] The "O" logo returns to Home from any page.
- [ ] With one group, the group switcher shows the group's name and gradient avatar with no chevron, and clicking it does nothing.
- [ ] To test more than one group, create a second group with a **different** admin email: `. infra/.venv/bin/activate && python scripts/bootstrap_admin.py --env dev --admin-email second-admin@example.com --group-name "Second Group"`. Don't reuse A's email, because the script would write a second user record for A's Cognito account. Sign in as that admin, invite A, and redeem it as A. Then:
  - [ ] the switcher shows a chevron, and its menu lists both groups with their gradients
  - [ ] picking a group navigates Home and shows that group's editions
  - [ ] opening a `/g/<other group>/…` URL directly makes that group current in the switcher
  - [ ] the choice survives a reload
  - [ ] the menu closes on an outside click and on Escape
- [ ] The avatar menu holds **Settings** and **Sign out**, and closes on an outside click and on Escape.
- [ ] `/no-such-page` shows "Lost in the fog" and a **Take me home** button.
- [ ] `/g/00000000-0000-0000-0000-000000000000/upcoming` (a group you aren't in) redirects to Home.
- [ ] As B (a member, not an admin), `/g/<GROUP_ID>/admin` redirects to the group, which then redirects to its newest open or published edition, or to Upcoming if there's only a voting cycle.
- [ ] `/g/<GROUP_ID>` alone redirects the same way.
- [ ] `/g/<GROUP_ID>/n/209901` shows "This edition doesn't exist."
- [ ] Every page header's ← button goes to that page's parent (Home, the edition, Upcoming or Settings).

## 4. Profile

Do this early, so names, colours and photos are set when later sections show them.

- [ ] As B: Settings. The page title is "You". The profile card shows B's colour avatar with an initial, the display name, the email, and **Edit profile** and **Upload photo**.
- [ ] **Edit profile**. Clear the name: "Name can't be empty." The field stops at 40 characters. Pick each of the 8 colour swatches; the preview avatar changes. **Cancel** discards the changes.
- [ ] Change the name and colour, then **Save** ("Saving…"). Toast "Profile updated." The Home greeting uses the new name.
- [ ] **Upload photo** with the GIF: "That file type isn't supported." With the 5–15 MB JPEG: "That image is larger than 5 MB."
- [ ] **Upload photo** with a small JPEG. The button reads "Uploading N%", then "Processing…" with a spinner over the avatar, then the toast "Profile photo updated." The photo replaces the colour avatar, and **Remove photo** appears.
- [ ] **Upload photo** again. A dialog asks "Replace your photo?", "This permanently deletes the image. It can't be recovered.", with **Choose a new photo**. Escape, a click outside the dialog, and **Cancel** each close it without changes. Confirm and pick a PNG: the new photo shows.
- [ ] **Remove photo** → "Remove your photo?" → **Remove photo**. The colour avatar returns. Upload a photo once more so B has one for later sections.
- [ ] As A, set a different colour but no photo.

## 5. Push notifications setup

Subscribe A and B now, so the pushes sent by later sections arrive. Run this in Chrome on the production build (§0.4).

- [ ] As A: Settings → **Notifications**. It shows a **Push notifications** toggle (off), a disabled **Send test push**, "My devices" with "No devices subscribed yet.", a "Per-group notifications" row for Dev Group with **Cycle open** and **Deadline reminders** both ticked, and "New-edition notifications are always on."
- [ ] Turn the toggle on and allow the browser prompt. The toggle turns on, and My devices lists "Chrome · macOS" (or your OS) with a "This device" pill.
- [ ] **Send test push** ("Sending…"). A result line shows each device with its outcome and HTTP status. An OS notification arrives: "Test push from OpenNewsletter", "If you got this, your device is wired up." Click it: the app opens or focuses on `/settings`.
- [ ] Untick **Deadline reminders** for Dev Group, reload, and check it stays unticked. Tick it again.
- [ ] Turn the toggle off: the device disappears from My devices. Turn it back on.
- [ ] **Remove** next to this device: the row goes and the toggle turns off. Turn it back on.
- [ ] Repeat the toggle-on and test push as B.
- [ ] Denied permission. In C's profile, click the padlock in the address bar → Notifications → **Block**, then reload Settings. It says "Blocked for this browser — enable them in your browser settings, then reload." and the toggle is disabled. Reset the permission afterwards.
- [ ] In C's profile, with permission reset, turn the toggle on and dismiss or deny the browser prompt. Toast "Notifications were blocked — enable them in your browser settings."

## 6. Suggesting and voting

The edition will have 5 slots. This section creates six candidate questions with a known vote spread, so §7 can check which five get promoted, and in what order.

### 6.1 Empty state

- [ ] As A, open Home. The "Taking suggestions" section has one row: the month and year on a purple tile, a "Voting" label, and "Suggest and vote on questions for next month". Click it.
- [ ] The Upcoming page: eyebrow "Dev Group · upcoming", title "Suggested questions", a **＋ Idea** button, and a banner "Vote on the questions" with "{N} days until the {Month} newsletter opens". The list shows "No one's suggested a question yet." with a **Suggest a question** button. The votes line reads "You've used 0 of 3 votes".

### 6.2 The suggest form

- [ ] **＋ Idea** opens "Suggest a question" (eyebrow "For next month"). The privacy card reads "Asked by you" and "Your friends see “You asked: …” next to this question." Type defaults to **Open**. **Submit** is disabled.
- [ ] Type 3 characters: "Add a bit more (at least 5 characters)". The counter reads "3 / 500".
- [ ] Each of the four "sparks" chips fills the question field.
- [ ] Toggle anonymous on: the card turns purple and reads "Anonymous to the group" and "Admins can still see who asked — your friends won't." The button changes to **Submit anonymously**. Toggle it off again.
- [ ] Choose **Poll**. Two empty option fields appear. **＋ Add option** adds fields up to 6, then disappears. The × buttons remove fields down to 2, then disappear.
- [ ] Leave an option empty: "Required". Enter "Tea" and "tea": `"tea" is already an option`. Paste 81 characters into an option: "80 characters max".
- [ ] Press the header's ← to go back without submitting.

### 6.3 Create the candidates

Submit these in this order. After each one, you return to Upcoming with the toast "Question submitted." (or "Question submitted anonymously.").

| # | As | Type | Prompt | Notes |
|---|---|---|---|---|
| Q1 | B | Open | "What was the best thing you ate this month?" | |
| Q2 | A | Open | "Show us a photo from a walk." | |
| Q3 | B | Open | "What's something you changed your mind about?" | **anonymous** |
| Q4 | A | Poll | "Best season?" | options Spring, Summer, Autumn |
| Q5 | B | Poll | "Coffee or tea?" | options Coffee, Tea |
| Q6 | A | Open | "Filler: this one should not be promoted." | submit last, never vote for it |

- [ ] Each card shows the asker's avatar and "{name} asked", the prompt, and "0 votes · submitted {date}". Poll cards also show "Poll · N options" and an option chip for each option.
- [ ] As B, Q3 reads "Asked anonymously" and its meta line ends with "· anonymous". As A, an admin, Q3 shows B's name and avatar.

### 6.4 Voting

- [ ] As A, press "vote ↑" on Q1. The button turns coral and shows the count, "1". The card's meta reads "1 vote". The votes line reads "You've used 1 of 3 votes" and a **Manage votes** button appears.
- [ ] Press Q1's button again: the vote is removed. Vote for Q1 again.
- [ ] Vote for Q3 and Q4 too. The line reads "You've used 3 of 3 votes", and every card you haven't voted for is dimmed and can't be pressed.
- [ ] **Manage votes** opens "Your votes", listing Q1, Q3 and Q4, each with **Remove vote**. Remove Q4: the other cards are enabled again. Vote for Q4 again. Press **Manage votes** again to close the panel.
- [ ] Cap race: open Upcoming as A in a second tab. In tab 1, remove the Q4 vote, then reload tab 2 so both tabs show "2 of 3". In tab 2, vote for Q4 again. In tab 1, without reloading, vote for Q2. Toast "You've used all 3 votes this cycle. Remove one to vote here." and the list refreshes. A ends with votes on Q1, Q3 and Q4.
- [ ] As B, vote for Q1, Q2 and Q5.
- [ ] As A, reload. Q1 shows 2 votes; Q2 to Q5 show 1 each; Q6 shows 0.
- [ ] **Top** order: Q1 first, then Q2, Q3, Q4, Q5 (ties go to the oldest), then Q6. **Recent** order: newest first, so Q6, Q5, Q4, Q3, Q2, Q1.

## 7. Opening the edition

- [ ] Run `advance 40d`. The response lists `"from": "voting", "to": "open"` for Dev Group, and may also list the next month's cycle with `"from": null, "to": "voting"`.
- [ ] As A and B (with push on, §5): an OS notification "Dev Group: time to write" with "5 questions are waiting. You have … days to respond." Clicking it opens the new edition. Don't run `advance` again to check for duplicates: with the edition now open, it would rewind the edition's close instead and could publish it.
- [ ] Reload Home. "Awaiting responses" lists the new edition on a coral tile, with "{countdown} left · you've published 0 of 5". If the next voting cycle was created, "Taking suggestions" lists it too.
- [ ] Open the edition. The header is coral: eyebrow "Dev Group · editions", title "{Month Year}", and a countdown pill such as "31d 04h". A card reads "Open for responses", "Closes {date, time and zone}", "Your progress" and "You've answered 0 of 5", with five empty progress bars.
- [ ] The five questions are Q1 to Q5, in that order, each marked "Not started" with an empty circle. Q4 and Q5 are labelled "Poll". Q3 reads "Asked anonymously" for B and shows B's name for A. Q6 isn't there.
- [ ] Open Upcoming. A link reads "The {Month} edition is open — answer now". If a new voting cycle exists, its list is empty again. If not, the page reads "Voting for the next edition hasn't opened yet."

## 8. Answering text questions

### 8.1 Draft and autosave (B)

- [ ] As B, open Q1. Eyebrow "Question · question", the prompt as the title, the countdown pill, a coral "Closes …" banner, the prompt card, and an editor with a **Preview** toggle, the placeholder "Tell us about it…", a dropzone ("Drag photos here, or choose files", "0/10 images · JPEG, PNG, WebP or GIF · up to 15 MB each") and "0 words". The bottom bar has a save badge, **Save & exit**, and a disabled **Publish**.
- [ ] Press **Save & exit** without typing. Q1 is still "Not started": visiting a question doesn't create a draft.
- [ ] Open Q1 again and type a sentence. The badge reads "Unsaved changes", then "● Saving…" about 1.5 seconds after you stop, then "Draft — publishes automatically at the deadline". A yellow note appears: "Drafts are published automatically when responses close on {date}. Clear your answer if you don't want it included." The word count updates.
- [ ] Type, then click outside the textarea at once: it saves straight away. Type, then switch to another tab at once: it saves before you come back.
- [ ] **Save & exit**. On the edition, Q1 reads "Draft" with an ellipsis icon and a preview of your text. Its progress segment turns a lighter coral. "You've answered 0 of 5", because drafts don't count until published.
- [ ] Reload Q1: the text is still there.

### 8.2 Markdown and sanitizing

- [ ] Enter a heading, **bold**, *italic*, a bulleted list, a numbered list, a link, ~~strikethrough~~, a `- [ ]` task list and a small GFM table. **Preview** renders all of them, and links open in a new tab. **Edit** returns to the textarea.
- [ ] Enter `<script>alert(1)</script>`, `<img src=x onerror=alert(1)>` and `[click](javascript:alert(1))`. In Preview, no alert fires, no raw HTML renders, and the `javascript:` link does nothing.
- [ ] Enter `![cat](https://example.com/cat.jpg)`. Preview shows a link labelled "cat", not an image. Remove these test lines.

### 8.3 Images

- [ ] Click the dropzone and choose the JPEG. A tile shows "Uploading N%", then "Processing…", then the thumbnail with a "Describe this image" field. `![](image:<id>)` is inserted into the text at the cursor. **Publish** is disabled while the upload is in progress.
- [ ] Drag the PNG and the WebP onto the dropzone together: it highlights, and both upload. Add the animated GIF too.
- [ ] Add one more image and reload while its tile reads "Processing…". After the reload the tile carries on and reaches its thumbnail.
- [ ] **Preview** shows the images inline, where their tokens are.
- [ ] Select the PDF and the 15 MB+ JPEG together. Errors list each file by name: "{name}: That file type isn't supported." and "{name}: That image is larger than 15 MB."
- [ ] Keep adding images, reusing the same files, until "10/10". The next file shows "You can attach up to 10 images."
- [ ] Press × on a ready tile. A dialog "Delete this image?" with "This permanently deletes the image. It can't be recovered." appears. **Cancel** keeps the image. **Delete image** removes the tile and its token from the text, and the draft saves. Delete images until four remain.
- [ ] Delete one image's token line from the text by hand, but leave its tile. That image will appear in the answer's image grid instead of inline (§12).
- [ ] Reload: the text and the four images are still there.

### 8.4 Publish and edit

- [ ] **Publish**. Toast "Published — friends can see this when the edition publishes." The page switches to a read-only view of your answer with an **Edit** link. The bottom button reads **Save changes**.
- [ ] Back on the edition, Q1 reads "Published" with a ✓, and "You've answered 1 of 5".
- [ ] Open Q1 and press **Edit**. A note reads "Changes save automatically and update your published answer." Edit the text: it autosaves and stays published.
- [ ] As B, answer Q2 with an image only: add one image, then delete everything in the textarea, including the image's token, but leave the image tile. **Publish** is still enabled. Publish it.
- [ ] As B, type a few words into Q3 and leave it unpublished. Don't publish this one: it checks auto-publish in §11.
- [ ] As A, type into Q1, wait for the save, then delete all the text. The yellow note disappears and **Publish** is disabled. Leave it empty: an emptied draft must not publish at close.
- [ ] As A, answer and publish Q2.

### 8.5 Network failures

- [ ] As B, DevTools → Network → **Offline**. Type into Q3. The badge reads "You're offline". Close the tab. Go back online, open the same question: your unsaved text is restored from local storage, the badge reads "Unsaved changes", and it then saves.
- [ ] Block the API host instead (Network → right-click a request to it → **Block request domain**). Type. The badge reads "Saving failed — retrying". Unblock it: the save succeeds within about 30 seconds, without a reload.

## 9. Answering poll questions

- [ ] As B, open Q4. The eyebrow reads "Question · poll". The options show as rows with radio circles. The badge reads "Pick one" and **Publish** is disabled.
- [ ] Pick Summer. It saves at once ("● Saving…", then "Draft — publishes automatically at the deadline"). The note reads "Drafts are published automatically when responses close on {date}.", without the sentence about clearing. Pick Autumn: it saves again.
- [ ] **Publish**. Toast "Published — friends can see this when the edition publishes."; the badge reads "Saved" and the button **Save changes**. Pick Spring: it saves at once and stays published.
- [ ] On the edition, Q4 reads "Published" with "Spring" as its preview.
- [ ] As B, pick Coffee on Q5 and don't publish (checks poll auto-publish).
- [ ] As A, pick and publish an option on Q4. Leave Q5 unanswered.
- [ ] There's no way to clear a poll pick once made (§21).

## 10. Deadline reminders

Push must be on for A and B (§5). Read the edition's countdown pill before each step, and rewind just far enough to put the close inside the next reminder's window. A reminder is due once the time left is at or under its offset (96, 48 or 24 hours). Its title names the offset, not the exact time left.

At this point B has answered all five questions (published, or a non-empty draft, which counts because it auto-publishes), and A has answered two (an emptied draft doesn't count). Members who have answered everything get different copy, so A and B see different bodies.

- [ ] Rewind until about 3 days remain. For example, if the pill reads "31d 04h", run `advance 28d`. The pill now reads about "3d 04h", the "Closes" date has moved, and Home's row agrees.
- [ ] Run `notify`. The response lists a `deadline_reminder` with `"offsetHours": 96`. A gets "Dev Group: 4 days left" with "Don't forget — write your answers before {weekday, month day}." B gets the same title with "5/5 answers in. You're done — but you can still edit."
- [ ] Run `notify` again: nothing new is sent.
- [ ] Rewind until under 48 hours remain (from "3d 04h", `advance 30h`), then `notify`. A gets "Dev Group: 2 days left" with "2 of 5 answers written. 3 to go." B gets the "You're done" body again.
- [ ] Rewind until under 24 hours remain (`advance 1d`), then `notify`. A gets "Dev Group: due tomorrow" with "Last call — answers lock at {time}." B gets nothing: members who have answered everything are skipped at the last reminder.
- [ ] Before one of the steps above, untick **Deadline reminders** for A. A gets no reminder for that step. Tick it again.

## 11. Closing the edition

- [ ] As B, open Q3 (the unpublished draft) and add a sentence. Wait for the badge to settle. Also open Q5 in a second tab.
- [ ] Run `advance` by a little more than the pill shows (for example `1d` when it reads "22h 10m"). The response lists `"from": "open", "to": "published"`.
- [ ] Notification for A and B: "Dev Group: this month's edition is out", "4 answers, 2 polls — go read it." The 4 are B's answers to Q1, Q2 and Q3 and A's to Q2. This notification has no opt-out.
- [ ] In B's Q3 tab, type a few more words. The save is refused and the tab moves to the published edition. The "This edition has closed — your last saved version stands." note may flash first. In §12, Q3 shows the sentence saved before the close, without the words typed after it.
- [ ] In B's Q5 tab, pick Tea. Toast "This edition has closed — your last saved version stands."; the pick returns to Coffee, the same note appears, and **Publish** is gone.
- [ ] Open any `/respond/…` URL of this edition: it redirects to the edition page.

## 12. Reading the published edition

- [ ] Home lists the edition under "This month's edition" (or "Past editions") on a green tile, with "5 questions · published {date}".
- [ ] The edition header is green: title "{Month Year}" and a "Published" pill. Below it: "Published {date}".
- [ ] Every question is expanded. Its − button collapses it and + expands it again.
- [ ] **Q1**: B's answer only. A's emptied draft isn't shown. The answer card shows B's photo, B's name and "published {date}", then the markdown and the inline images. The image whose token you deleted appears in a grid below the text. Clicking a grid thumbnail opens the full image in a new tab.
- [ ] **Q2**: B's image-only answer and A's answer.
- [ ] **Q3**: B's unpublished draft, auto-published at close, as last saved before the close (§11). The byline reads "Asked anonymously" for B and B's name for A.
- [ ] **Q4**: "{N} votes", each option with "count · percent" and a bar. Your own pick has a purple bar, a ring and a "Your vote" badge. There's nothing to click.
- [ ] **Q5**: B's unpublished Coffee pick counts as a vote, because it auto-published.
- [ ] If a text question got no answers, it reads "Nobody answered this one. Maybe next month." To see this, leave a text question unanswered in a later cycle.
- [ ] The footer reads "That's a wrap on {Month} 🎈" and its **Suggest a question** link goes to Upcoming.
- [ ] As A: Admin → Members. B's line reads "1 edition answered".
- [ ] Media serving: in DevTools → Network, pick an image request. It's a presigned S3 URL that returns 200 with `Access-Control-Allow-Origin: http://localhost:5173`. Open the same URL without its query string in a new tab: 403. The full media checks are in [`smoke-test.md`](smoke-test.md) §6.

## 13. Reactions

Reactions and comments appear only on published text answers, never on polls.

- [ ] Each answer card shows dashed quick-pick buttons 🔥 🤣 ❤️ 😍 👍 🎉 and a **＋** button.
- [ ] As A, tap 👍 on B's Q1 answer. It becomes a solid pill "👍 1", highlighted as yours, and leaves the dashed row. The header shows "1 reaction".
- [ ] Tap "👍 1" again: the reaction is removed. Tap 👍 again.
- [ ] **＋** opens an emoji panel: a grid and a "Type an emoji…" field with **Add**. Pick 🚀: it's added, the panel closes, and focus returns to **＋**.
- [ ] Open the panel and press Escape: it closes. Open it and click outside it: it closes.
- [ ] Type "abc" into the field: **Add** stays disabled. Type 🦄 and press Enter: it's added. Add 👨‍👩‍👧‍👦 and 👍🏽: each is a single reaction.
- [ ] As B, react 👍 to the same answer. As A, reload: "👍 2", highlighted for A. As B, it's highlighted for B too.
- [ ] Reload: reactions and the header total remain.

## 14. Comments

- [ ] Each answer card ends with an "Add a comment…" box, a one-image dropzone ("0/1 image") and a **Post** button, disabled while the box is empty.
- [ ] As A, comment on B's Q1 answer with markdown (`**nice**` and a link). It appears with A's avatar, A's name and a relative time. Hovering the time shows the full date in the group's timezone. The bold and the link render. An `![](image:…)` token in a comment renders nothing, and a remote image becomes a link.
- [ ] Type 1,801 characters: a counter "1801/2000" appears. The box stops at 2,000.
- [ ] Attach an image and wait for its thumbnail. **Post** is disabled while it uploads. Post an image-only comment: the thumbnail shows, and clicking it opens the full image.
- [ ] Post enough comments to reach five. Only the last three show, with a "Show all 5 comments" link above them. Clicking it shows all five.
- [ ] **Edit** one of A's comments. **Cancel** restores it. Edit and **Save**: the text updates and "(edited)" appears. As B, reload: the edit shows.
- [ ] Edit A's image-only comment, clear nothing, and press **Remove image**: the uploader appears. **Save**: "Add some text or an image.", because the comment would be empty. Type some text and **Save** again: a dialog "Delete the attached image?" with "This permanently deletes the image. It can't be recovered." and **Delete image and save**. Confirm: the image is gone and the text shows.
- [ ] Edit that comment again, attach a new image in the uploader and **Save**. The new image shows.
- [ ] **Delete** one of A's comments: a dialog "Delete this comment?" with "This can't be undone." (it also mentions the image if there is one). Confirm: "[deleted]" stays in its place, with no time and no actions.
- [ ] As B, comment on A's Q2 answer. B sees **Edit** and **Delete** on their own comment and nothing on A's.
- [ ] As A, an admin, B's comments show **Delete** but not **Edit**. Delete one: it becomes "[deleted]" for B too.

## 15. Group admin: members

- [ ] As A: Admin → Members. "3 of 50 members". Each row shows the avatar, the name, an "admin" pill where it applies, and "N editions answered" or "joined … ago · pending first answer". A's own row says "(you)" and has no ⋯ menu.
- [ ] ⋯ on B's row: **Make admin** and **Remove from group**. **Make admin**: toast "{B} is now an admin." and the admin pill appears.
- [ ] As B, reload Settings. Your groups shows **Manage** on Dev Group, and it opens the admin page.
- [ ] As A, ⋯ on B → **Make member**: toast "{B} is now a member."
- [ ] As A, ⋯ on C → **Remove from group**. A dialog "Remove {C} from the group?" with "They'll lose access to this group's newsletters and need a new invite to rejoin." **Cancel** keeps C. Confirm: toast "Removed {C}." and the count drops to 2.
- [ ] As C, without reloading, go from Home into the edition. The API now refuses C, so the page shows an error state with **Try again** (content C viewed in the last 30 seconds may show from cache instead). Reload: Dev Group is gone from Settings, Home shows the join form, and `/g/<GROUP_ID>/upcoming` redirects to Home.
- [ ] As A, under Invites, C's consumed invite now reads "used by a former member".
- [ ] As C, redeem the Admin invite from §2.1. C rejoins as an admin: Settings → Your groups shows **Manage**, and A's Members tab shows C with the "admin" pill.

## 16. Group admin: settings

### 16.1 The form

- [ ] Admin → Settings shows the group name, 7 gradient swatches, a timezone list, Questions per cycle (5), Votes per member per cycle (3), Response window (7), reminder hours (96, 48, 24), "Notify members when a cycle opens" (on) and Member soft cap. The lines "Timezone changes affect the current voting cycle's open time.", "Moves the current voting cycle's close, and an open cycle's close if it's still in the future." and "Changes apply to the current cycle where possible." are shown.
- [ ] **Save** is disabled until something changes, and disabled again if you change a field back.
- [ ] Validation, one field at a time, pressing **Save** each time:
  - [ ] empty name: "Name can't be empty."
  - [ ] questions per cycle 0 or 21: "At least 1." or "20 max."; 2.5: "Whole numbers only."
  - [ ] votes per member above questions per cycle: "Can't exceed questions per cycle (N)."
  - [ ] response window 0 or 29: "At least 1 day." or "28 days max."
  - [ ] a reminder of 0 or 169: "At least 1 hour." or "168 hours max."; two equal reminders: "Duplicate reminder."
  - [ ] member soft cap below the member count: "Can't be below the current N members."
- [ ] **＋ Add reminder** adds a 24-hour row. × removes rows down to one, then disappears.
- [ ] Change the name and the gradient, then **Save**. Toast "Group settings saved." The switcher and the admin header update without a reload. Change them back, since later steps quote "Dev Group".
- [ ] Change the timezone to `Europe/London` and save. Dates on the group's pages now show in London time, and Upcoming's "N days until …" banner may shift by a day. Set it back.

### 16.2 Settings that change the next cycle

This runs a second cycle.

- [ ] Set Questions per cycle to 2 and Votes per member to 2 (votes can't exceed questions), and turn **Notify members when a cycle opens** off. Save.
- [ ] Suggest three candidates in the new voting cycle, from A and B. As B, vote for two of them. As A, set Votes per member to 1 and save. As B, reload: both votes stay, the line reads "You've used 2 of 1 vote", and the other card can't be voted for until B removes a vote.
- [ ] Run `advance 50d`. The new edition opens with exactly 2 questions, and no "time to write" notification arrives.
- [ ] Set Response window to 1 day and save. Inline under the field: "Would close the open cycle in the past." Nothing is saved.
- [ ] Set Response window to 28 and save. It's accepted, and the open edition's "Closes" date changes to 28 days after its rewound open time.
- [ ] Restore 5 questions, 3 votes and cycle-open notifications. Leave the window at 28 for now: while this edition is open, setting 7 may be refused with the same "in the past" error. `make seed` resets it.

## 17. Leaving a group

- [ ] As C (an admin since §15): Settings → Your groups → **Leave** on Dev Group. A dialog "Leave Dev Group?" with "You'll lose access to its newsletters and need a new invite to rejoin." Confirm: toast "Left Dev Group." and Home shows the join form. As A, Members no longer lists C.
- [ ] As A, now the only admin again, **Leave** Dev Group and confirm. Toast "You're the only admin — promote someone else first." A is still a member.
- [ ] Settings → Your groups → **View** goes to the group. **Join with an invite code** opens `/join`.

## 18. PWA install, caching and offline

On the production build (§0.4), in Chrome:

- [ ] DevTools → Application → **Manifest** shows the name "OpenNewsletter", the short name "Newsletter", three icons (one maskable) and display "standalone".
- [ ] **Service workers**: `sw.js` is activated and running.
- [ ] **Cache Storage**: the precache holds the build's JS, CSS and HTML. After you've viewed images, `media-images` holds image entries and `media-avatars` holds avatar entries, keyed by URL **without** the `X-Amz-…` query string. No API response is cached anywhere.
- [ ] In the page's `<head>` there's a `<meta http-equiv="Content-Security-Policy">` tag. The Console shows no CSP violations after the whole walkthrough.
- [ ] Home and Settings show "Install OpenNewsletter for the best experience." with **Install** (it can take a reload before Chrome offers it). Install: the app opens in its own window, and the install banner isn't shown there.
- [ ] In the installed window, open Settings and send a test push. Clicking the notification brings the installed window to the front.
- [ ] On a published edition, go offline (DevTools → Network → Offline) without reloading. Collapse and expand a question: its images still load, from the `media-images` cache.
- [ ] Still offline, reload. The app shell loads from the precache, but API responses are never cached, so there's no data. Home shows "Couldn't load your groups." with **Try again**, never the join form. A group page redirects to Home, and Settings shows a spinner (§21). Go back online and press **Try again**: Home loads.
- [ ] Optional: leave a published edition open for more than an hour, until its presigned URLs expire, then collapse and expand a question. The images load again: an expired URL is refreshed once. An image that still fails shows a blank placeholder.
A phone can't reach `localhost:5173` on your computer, and that's the only local origin the dev stack allows. On Android, use Chrome's port forwarding: connect the phone by USB, open `chrome://inspect/#devices` on the computer, and forward port 5173 to `localhost:5173`. The phone's `localhost:5173` then reaches your preview build. iOS has no equivalent, and iOS push needs an installed app served over HTTPS, so run the iPhone checks against prod.

- [ ] iPhone, Safari, not installed: Home and Settings show "Add OpenNewsletter to your Home Screen: tap ↑ Share, then "Add to Home Screen"." with ✕. Dismissing it is remembered after a reload. Settings → Notifications reads "Add OpenNewsletter to your Home Screen to get notifications." After adding it to the Home Screen and opening it from there, the full notification controls appear and push works.
- [ ] Android, Chrome: the Install button appears and installs the app. Push works in the installed app.

## 19. Layout, keyboard and accessibility

- [ ] At 375 px wide (DevTools device toolbar): content fills the width with side padding, nothing scrolls sideways, and the respond page's fixed bottom bar doesn't cover the end of the editor or the image tiles.
- [ ] On a wide desktop window, content sits in a narrow centred column.
- [ ] Keyboard only, from page load: Tab reaches the logo, the switcher, the avatar menu, every vote button, the sort buttons, the dropzone (Enter or Space opens the file picker), reaction pills and **＋**, admin tabs and every dialog button. Focus rings are visible. Opening a menu moves focus into it, and Escape returns focus to its button.
- [ ] Confirm dialogs close on Escape and on a click outside, but not while their action is in progress.
- [ ] With a screen reader (VoiceOver: Cmd+F5), form errors on the suggest form, the join form and the profile form are announced when they appear. Vote buttons announce "Upvote …" or "Remove your vote for …" and whether they're pressed.

## 20. Mock mode (optional, no AWS)

For layout and UI checks without the dev stack:

```sh
cd frontend && npm run dev      # http://localhost:5173, VITE_USE_MOCKS=true
```

- [ ] You're signed in as "Quinn", in three groups: Trail Crew (admin), Game Night Gang (member) and Meeple Mailbox (member). Between them the groups have voting, open and published editions, which makes this the quickest way to check the group switcher and the Home sections.
- [ ] The invite code `DEMO-JOIN-CODE` joins a demo group. Any other code is "That invite code isn't valid."
- [ ] All data resets on reload. There's no real sign-in, no cycle ticks, and no service worker, so push and caching can't be tested here.

## 21. Known gaps and expected behaviour

These are how the current build behaves. Don't log them as failures unless you think the behaviour itself should change.

- **A poll pick can't be cleared**, so any poll pick auto-publishes at close ([`frontend.md` §7.3](../design/frontend.md#73-respondpage-ggncrespondq); open question in [`PROGRESS.md`](../../plans/PROGRESS.md)).
- **Image alt text isn't saved.** The "Describe this image" field under each uploaded image only lives in the page. Reload, and it's empty again.
- **No UI to delete a candidate question or to create a group.** Groups come from `scripts/bootstrap_admin.py`.
- **The header avatar** shows an initial taken from the sign-in token. It doesn't show the profile photo or colour, or a display name changed in Settings.
- **An untouched question** shows "Saved just now" (or "Saved {time} ago") in the badge before you've typed anything.
- **Offline, group pages redirect to Home**, because their membership check can't load `/config`, and Settings shows a spinner.
- **An edition with no questions** (nobody suggested any) shows "You've answered 0 of 0" and an empty list while open, and only its header and footer once published. [`lifecycle.md` §9](../design/lifecycle.md#9-empty-cycle-edge-cases) describes an empty-state message that the UI doesn't render.
- **Dark mode and reduced motion aren't implemented**, although [`frontend.md`](../design/frontend.md) §4.2 and §11 describe them. The design page and the code disagree here.
- **Month labels on fast-forwarded cycles** come from the rewound open time, so after several `advance` runs two editions can show the same month. That's an artifact of the dev tick, not of real schedules.
- **Invites** list consumed and revoked invites until they expire, with no pagination.
- **Cross-account changes need a reload** (§0.6).

## 22. Clean up

- [ ] `make seed` to reset the data. It also resets A's password, unless you pass `--admin-password` (§0.3).
- [ ] Delete test Cognito users you no longer need: `aws cognito-idp admin-delete-user --user-pool-id "$POOL" --username <email>`.
- [ ] In each Chrome profile, turn push off in Settings before signing out, or remove the site's data under DevTools → Application → **Clear site data**.
