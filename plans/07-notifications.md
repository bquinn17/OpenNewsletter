# 07 — Notifications (Web Push)

OpenNewsletter sends notifications for two events: a new cycle opens for responses, and reminders before the response deadline (default 96h, 48h, 24h before close). All notifications are delivered as **Web Push** to PWA clients using VAPID. Email is out of scope for v1.

---

## 1. Components involved

| Component | Role |
|---|---|
| Frontend service worker | Registers a `PushSubscription`; receives push events; renders OS notification |
| `lambda-push` | Subscribe / unsubscribe / list / test endpoints; also called by tick Lambdas to deliver |
| `lambda-cycle-tick` | Triggers cycle-open notifications when transitioning a cycle to `open` |
| `lambda-notify-tick` | Runs every 15 min; finds open cycles with deadlines within configured offsets and fans out reminder pushes |
| Secrets Manager `opennewsletter/vapid/{env}` | Holds VAPID public key, private key, and subject |
| DynamoDB | Stores `PushSubscription` rows and idempotency markers |

---

## 2. VAPID keys

Generated once per environment, out of band:

```bash
python scripts/generate_vapid_keys.py
# prints JSON
# {"publicKey": "BPV...", "privateKey": "Vmt...", "subject": "mailto:admin@opennewsletter.example.com"}
```

The script uses the `py_vapid` library. The output is stored in Secrets Manager under `opennewsletter/vapid/{env}`. Public key is exposed via `GET /config`.

The Rust `lambda-notify-tick` and `lambda-push` share `lambda-push`'s library (`push`), which implements Web Push itself (§10 — **not** the `web-push` crate, decided M11). Cold-start fetch of the secret: `aws_sdk_secretsmanager::get_secret_value` on `VAPID_SECRET_ARN`, cached in `OnceCell` for the warm container's lifetime. The `privateKey` value is the raw 32-byte P-256 scalar, URL-safe base64 unpadded (the format `scripts/generate_vapid_keys.py` writes); the Rust side loads it directly — **no PEM conversion step exists anywhere**.

---

## 3. Subscription lifecycle

### 3.1 Create

The frontend's `pushSetup.ts`:

1. Calls `Notification.requestPermission()` (only on a user gesture in Settings).
2. `serviceWorker.ready` → `pushManager.subscribe({ userVisibleOnly: true, applicationServerKey })`
3. POSTs `subscription.toJSON()` to `POST /push/subscribe`.

`lambda-push` upserts the row keyed by `(userId, sha256(endpoint))`. If a row already exists for this `(user, endpoint)`, it is overwritten — handles re-subscriptions after the browser rotates the endpoint.

### 3.2 Pruning failures

When the tick Lambdas attempt delivery and receive a `410 Gone` or `404 Not Found` from the push service, the subscription is deleted immediately (it's permanently invalid).

For other errors (5xx, timeouts), `failureCount` is incremented. After 5 consecutive failures, the row is deleted.

Successful deliveries reset `failureCount` to 0 and update `lastSuccessAt`.

### 3.3 Unsubscribe

`POST /push/unsubscribe` with `{ endpoint }` → row deleted. Frontend calls this on:
- Settings toggle off
- Logout

The browser's local subscription is also unsubscribed via `subscription.unsubscribe()` so the OS-level permission state reflects reality.

---

## 4. Notification preferences

`NotificationPref` rows (per user, per group):
- `cycleOpen: bool` — receive a push when a new cycle opens
- `deadlineReminders: bool` — receive 96/48/24h reminders

Default: both `true`. Read at fan-out time.

If a user has no subscriptions OR has both prefs `false` for a given group, no fan-out happens for them.

---

## 5. Push payload schema

Every push is a JSON document:

```json
{
  "kind": "cycle_open" | "deadline_reminder" | "publication" | "test",
  "title": "...",
  "body": "...",
  "url": "/g/{groupId}/n/{cycleId}",
  "tag": "{groupId}:{cycleId}:{kind}",
  "groupName": "Trail Crew"
}
```

`url` is root-relative (decided M11; was absolute): the service worker resolves it against its own origin, so the backend needs no frontend-origin setting and dev/prod differ only by where the SPA is served. The test push uses `/settings` and tag `test`.

The service worker reads this, calls `showNotification`, and uses `tag` to coalesce re-deliveries (the OS shows only the latest per tag). On click, the SW navigates to `url`.

### 5.1 Per-kind copy

| Kind | Title | Body |
|---|---|---|
| `cycle_open` | "{groupName}: time to write" | "5 questions are waiting. You have 4 days to respond." |
| `deadline_reminder` (96h) | "{groupName}: 4 days left" | "Don't forget — write your answers before {dueDate}." |
| `deadline_reminder` (48h) | "{groupName}: 2 days left" | "{N} of {total} answers written. {remaining} to go." |
| `deadline_reminder` (24h) | "{groupName}: due tomorrow" | "Last call — answers lock at {time}." |
| `publication` | "{groupName}: this month's edition is out" | "{N} answers, {polls} polls — go read it." |
| `test` | "Test push from OpenNewsletter" | "If you got this, your device is wired up." |

`{N}`, `{remaining}`, and `{dueDate}` are computed per-recipient using their own draft state and the group's TZ.

**Generalised for any configured offset (decided M11).** Offsets are admin-configurable up to 168h, so the table above is the 96/48/24 instance of these rules, keyed on the offset `h` being sent:
- Title: `h ≥ 48` → "{groupName}: {round(h/24)} days left"; `24 ≤ h < 48` → "due tomorrow"; `h < 24` → "{h} hours left".
- Body: `h ≥ 72` → the `{dueDate}` copy; `36 ≤ h < 72` → the `{N} of {total}` copy; `h < 36` → the "Last call" copy.
- `cycle_open` body uses the real counts: "{total} questions are waiting. You have {window} days to respond.", window = the response window in whole days. `publication` omits the polls clause when there are none. Singular forms throughout.
- `{dueDate}` is `%A, %b %-d` and `{time}` is `%-I:%M %p %Z`, in the group's timezone.

---

## 6. Cycle-open fan-out

Triggered from `lambda-cycle-tick` immediately after promoting a cycle to `open`. Implemented as an async Lambda invoke (fire-and-forget) of `lambda-push`'s internal handler `cycle_open_fanout`. The function name comes from env `PUSH_FUNCTION_NAME`; an invoke failure is logged and never fails the tick. Skipped entirely when the group's `notificationSettings.onCycleOpen` is `false` (decided M11 — that group switch already existed in `02` §2.3 but nothing read it).

`lambda-push` serves API Gateway routes and these direct invokes from a single binary, dispatching on **event shape**: if the incoming JSON has a `requestContext` field it's an API Gateway event and routes by method+path; if it has an `internal` field it's a fan-out invoke. `requestContext` is checked first: API Gateway always wraps the client's body, so an HTTP caller can't forge an `internal` event. The direct-invoke envelope is:

```json
{ "internal": "cycle_open_fanout" | "publication_fanout", "groupId": "01H...", "cycleId": "202605" }
```

Anything matching neither shape is logged at ERROR and dropped.

### 6.1 `cycle_open_fanout` algorithm

```
# Idempotency — write a marker first
try:
    PutItem(pk=GROUP#{g}#NL#{c}, sk=NOTIFIED#OPEN, condition: attribute_not_exists(pk))
except ConditionalCheckFailed:
    return  # someone else already fanned out

group = get_group(g)
members = query GSI1 gsi1pk=GROUP#{g} begins_with(gsi1sk, MEMBER#)

for member in members:
    pref = get_notification_pref(member.userId, g)
    if pref and not pref.cycleOpen: continue
    subs = query pk=USER#{member.userId} begins_with(sk, PUSH#)
    payload = build_payload("cycle_open", group, cycle, member)
    for sub in subs:
        send_push(sub, payload)
```

`send_push` returns delivery success/failure → updates `failureCount`/`lastSuccessAt` or deletes per §3.2.

### 6.2 Concurrency

Every fan-out (cycle-open, deadline reminder, publication, test push) first gathers its recipients' subscriptions, then sends via `delivery::send_all`: up to `PUSH_SEND_CONCURRENCY` (16) requests in flight at once, each with the `PUSH_SEND_TIMEOUT_SECS` (10s) timeout. A dead endpoint costs one slot for 10s instead of delaying everyone queued behind it. Sending one at a time was dropped after the M11 review: with each send able to take up to 10s, 6 hung endpoints were enough to exhaust the 60s `push-api` timeout.

**Send cutoffs (decided post-M11 review).** No *new* send starts after a cutoff measured from the start of the work. Sends already in flight always finish, so their §3.2 bookkeeping lands. Each cutoff leaves the 10s send timeout plus ~5s headroom under the hard limit it protects:

| Work | Hard limit | Cutoff (`shared::config`) |
|---|---|---|
| Cycle-open / publication fan-out | `push-api` Lambda, 60s | `PUSH_FANOUT_SEND_CUTOFF_SECS` = 45 |
| `POST /push/test` | API Gateway integration, 30s | `PUSH_TEST_SEND_CUTOFF_SECS` = 15 |
| Deadline-reminder sends in one notify tick | `notify-tick` Lambda, 120s | `NOTIFY_TICK_SEND_CUTOFF_SECS` = 105 |

Markers are still claimed before sending (at-most-once), so a delivery skipped at the cutoff is never retried. It is, however, never silent: `send_all` logs it at ERROR and it counts toward the `PushSkipped` metric (§13). A test-push device that is skipped reports `failed` with no status code. In addition, a notify tick claims no further cycles after `NOTIFY_TICK_CYCLE_CUTOFF_SECS` (60s); those cycles' markers stay unclaimed, so the next tick (15 min later) sends their reminders instead of claiming them and then running out of time.

If a group ever exceeds 200 members (we'd revisit the soft cap first), refactor to batch via SQS.

---

## 7. Deadline reminder fan-out

`lambda-notify-tick` runs **every 15 minutes** via EventBridge.

### 7.1 Algorithm

```
now = utc_now()
group_offsets_default = [96, 48, 24]   # hours
MAX_REMINDER_OFFSET_HOURS = 168        # constant in shared/src/config.rs;
                                       # PATCH /groups validation caps every offset at this
                                       # (03-api-contract.md §4.3), so the query window below
                                       # is guaranteed to cover all configured offsets.

# Find any open cycle whose responseCloseAt is within the maximum allowed offset
candidates = query GSI2
    gsi2pk = "NL_STATUS#open"
    gsi2sk between f"{now.isoformat()}#~~~"
              and f"{(now + MAX_REMINDER_OFFSET_HOURS hours).isoformat()}#~~~"

for nl in candidates:
    group = get_group(nl.groupId)
    offsets = group.notificationSettings.offsetsHoursBeforeClose

    moot = [o for o in offsets if nl.responseCloseAt - o hours <= nl.responseOpenAt]
    due  = [o for o in offsets if o not in moot and nl.responseCloseAt - o hours <= now < nl.responseCloseAt]

    claim_markers(nl, moot)                      # never sent
    claimed = claim_markers(nl, due)             # NOTIFIED#CLOSE#{o}, attribute_not_exists
    if claimed:
        fanout_deadline_reminder(nl, group, min(claimed))
```

**Decided M11** (the original pseudo-code put each marker *after* its fan-out, and sent every overdue offset):
- **Claim first.** Markers are conditional puts written *before* sending, so two overlapping ticks can't both send. A crash mid-fan-out drops the rest of that reminder rather than risk duplicates.
- **Moot offsets are never sent.** An offset that lands at or before `responseOpenAt` (e.g. the default 96h against a 4-day window) would arrive minutes after the cycle-open push. Its marker is written and nothing is sent.
- **Only the most urgent due offset is sent.** If several are due at once (a late tick, or `advanceCycleClosesBy` in dev), the smallest is sent and all of them are marked, so members don't get a burst.

The 15-minute cadence means the actual delivery may be ≤15 min late versus the configured offset — acceptable for human reminders.

### 7.2 Per-recipient body customization

For each member:
- Fetch member's drafts and published responses for this cycle (AP15 — single Query).
- `N = published.len()`, `total = nl.lockedQuestionIds.len()`, `remaining = total - N`.
- Inject into the body template per §5.1.

Members who already published every answer get a different soft message: "{N}/{total} answers in. You're done — but you can still edit." Optional: skip them entirely. **Default: skip them at the 24h reminder, send them the 96h/48h ones.** Configurable later. Generalised in M11: they're skipped at the group's *smallest* configured offset. `N` counts published responses of any question kind.

---

## 8. Publication fan-out

When `publish(nl)` runs in `lambda-cycle-tick`, async-invoke `lambda-push`'s `publication_fanout` with `{ groupId, cycleId }`.

Idempotency marker: `NOTIFIED#PUBLISH` (claimed first, like §6.1). `{N}` counts published answers to text questions, `{polls}` the cycle's poll questions. Same algorithm as §6 but with the `publication` payload.

This was not on the original requirement list (the spec only mentioned cycle-open + deadline reminders) but is the natural counterpart: users want to know when there's a new edition to read. **Always on; no user-facing toggle.** The only kill-switch is the OS-/browser-level push permission. `NotificationPref` carries no `publication` field; `Group.notificationSettings` carries no `onPublication`. The fan-out still respects "user has zero subscriptions" — those users simply receive nothing.

---

## 9. Test push

`POST /push/test`:
- Sends a `kind: test` push to all of the caller's subscriptions.
- Returns per-subscription delivery results (success / 410 / other error) for the Settings UI to display: "✓ Chrome on Pixel 8" / "✗ Safari on iPhone (resubscribe)".

---

## 10. Web Push delivery details (Rust)

**Decided M11: implemented directly, not with the `web-push` crate.** `web-push` 0.10 pulls `ece` (OpenSSL backend) and `isahc` (libcurl), two C builds that put the arm64 `cargo lambda` cross-compile at risk (PROGRESS.md B6). `lambda-push`'s `webpush` module instead does RFC 8291 `aes128gcm` encryption (`p256` ECDH, `hkdf`, `aes-gcm`; tested against the RFC's Appendix A vector) and an RFC 8292 VAPID ES256 JWT (`aud` = endpoint origin, `exp` = now + 12h, `sub` from the secret), sent with `reqwest` over rustls with a 10 s timeout. Headers: `TTL: 86400`, `Urgency: normal`, `Content-Encoding: aes128gcm`, `Authorization: vapid t=…, k=…`. The sketch below is the original plan, kept for the behaviour it describes.

```rust
async fn send_push(sub: &PushSubscription, payload: &Payload) -> SendResult {
    let info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);
    let body = serde_json::to_vec(payload).unwrap();

    let mut builder = WebPushMessageBuilder::new(&info);
    builder.set_payload(ContentEncoding::Aes128Gcm, &body);
    builder.set_ttl(86400);                      // 24h: don't deliver if app reopens later
    builder.set_urgency(Urgency::Normal);
    let msg = builder.build()?;

    // Private key is stored as URL-safe base64 raw bytes (py_vapid's output format).
    let signer = VapidSignatureBuilder::from_base64(VAPID_PRIVATE_KEY_B64, URL_SAFE_NO_PAD, &info)?
        .add_claim("sub", VAPID_SUBJECT)
        .build()?;

    let client = IsahcWebPushClient::new()?;
    client.send_with_signer(msg, &signer).await
}
```

Error mapping:

| Web push response | Action |
|---|---|
| 200/201 | Mark success |
| 410 Gone | Delete subscription (endpoint expired) |
| 404 Not Found | Delete subscription |
| 413 Payload Too Large | Should never happen (payload capped at `MAX_PUSH_PAYLOAD_BYTES`, 3000, before encryption); counted as a failure |
| 429 Too Many Requests | Increment failure, retry next tick |
| 5xx | Increment failure, retry next tick |
| Network error | Increment failure, retry next tick |
| Other 4xx (e.g. 403 VAPID key mismatch) | Increment failure (deleted at 5) |

Bookkeeping writes are conditioned on the row still existing, so a send racing an unsubscribe never resurrects it. Note "retry next tick" means a later fan-out reaches the subscription; a reminder that failed isn't re-sent, since its marker is already claimed.

---

## 11. Frontend wiring

### 11.1 Settings page UI

A single toggle "Push notifications" (on = permission granted AND this browser holds a subscription) with:
- An "Allow" CTA that requests permission and creates a subscription.
- A "Send test push" button (after enabled).
- A "My devices" list with each subscription's user-agent string + last-success timestamp + delete button.
- Per-group preference toggles ("Cycle open", "Deadline reminders") with `PUT /push/preferences/{g}`. Publication push is always-on and has no toggle.

### 11.2 Re-subscribe-on-load

In `App.tsx`, after auth:

```ts
useEffect(() => {
  if (!user) return;
  ensurePushSubscription(config.vapidPublicKey).catch(reportError);
}, [user]);
```

`ensurePushSubscription` is a no-op if permission was previously denied; only refreshes the server-side subscription record if one exists locally. It never prompts on load (`{ prompt: false }`); Settings calls it with `{ prompt: true }`. If the local subscription was made with a different VAPID key it is replaced. Logout calls `disablePush()` best-effort before tokens are cleared.

### 11.3 PWA install prompt

Listen for `beforeinstallprompt`, stash the event, and surface an "Install OpenNewsletter" button on the home page (dismissible) and on Settings. iOS Safari can't be triggered programmatically; show a banner with instructions on iOS user-agent.

---

## 12. Iconography

Notification icon and badge served from the static frontend. Both 192px PNGs precached by the SW. Match Apple/Google guidelines for monochrome badges (the `badge` is rendered as a tinted glyph on Android).

---

## 13. Observability

Custom EMF metrics emitted from `lambda-notify-tick` and `lambda-push`:

```
Namespace: OpenNewsletter/{env}
Dimensions: { kind: "cycle_open"|"deadline_reminder"|"publication"|"test" }
Metrics:
  PushSent (count)
  PushFailed (count)
  PushExpired (count)        # 410/404 → subscription deleted
  PushSkipped (count)        # not attempted: send cutoff reached (§6.2)
  PushLatencyMs (p50/p95/p99)
```

One EMF line per fan-out (or test push), with dimension sets `[["kind"], []]` so the alarm can use the dimensionless series.

Alarm: PushFailed/(FILL(PushSent,0)+PushFailed) > 20% over 30 min → SNS to operator. The `FILL` matters: in a total outage `PushSent` has no datapoints, and without it the expression is missing (= not breaching) at a 100% failure rate.

---

## 14. Failure modes & UX

- **Browser doesn't support Web Push (Safari < 16.4 on iOS)**: detect via `'PushManager' in window` and `'Notification' in window`. Settings UI hides the toggle and shows "Push isn't supported in this browser. The PWA still works — install it for the best experience."
- **Permission denied previously**: instruct the user how to re-enable in browser settings; no programmatic recovery.
- **All subscriptions invalid for a user**: their experience degrades gracefully — they still get the in-app cycle indicator on Home, and an unread badge on the newsletter list. (Unread tracking is itself a future enhancement; for v1 the absence of push just means they may miss the deadline.)
