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

The Rust `lambda-notify-tick` and `lambda-push` use `web-push` crate (which is the maintained Rust web push library). Cold-start fetch of the secret: `aws_sdk_secretsmanager::get_secret_value`, cached in `OnceCell` for the warm container's lifetime.

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
  "url": "https://opennewsletter.example.com/g/{groupId}/n/{cycleId}",
  "tag": "{groupId}:{cycleId}:{kind}",
  "groupName": "Trail Crew"
}
```

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

---

## 6. Cycle-open fan-out

Triggered from `lambda-cycle-tick` immediately after promoting a cycle to `open`. Implemented as an async Lambda invoke (fire-and-forget) of `lambda-push`'s internal handler `cycle_open_fanout`, with payload `{ groupId, cycleId }`.

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

Members are fanned out sequentially within the Lambda invocation. With ≤50 members per group and Web Push send taking ~200ms median, this finishes in < 15s.

If a group ever exceeds 200 members (we'd revisit the soft cap first), refactor to batch via SQS.

---

## 7. Deadline reminder fan-out

`lambda-notify-tick` runs **every 15 minutes** via EventBridge.

### 7.1 Algorithm

```
now = utc_now()
group_offsets_default = [96, 48, 24]   # hours

# Find any open cycle whose responseCloseAt is within max_offset (=96)h ahead
candidates = query GSI2
    gsi2pk = "NL_STATUS#open"
    gsi2sk between f"{now.isoformat()}#~~~" and f"{(now + 96h).isoformat()}#~~~"

for nl in candidates:
    group = get_group(nl.groupId)
    offsets = group.notificationSettings.offsetsHoursBeforeClose

    for offset in offsets:
        target = nl.responseCloseAt - offset hours
        if target <= now and not has_marker(nl, offset):
            fanout_deadline_reminder(nl, group, offset)
            put_marker(nl, offset)   # NOTIFIED#CLOSE#{offset}
```

The 15-minute cadence means the actual delivery may be ≤15 min late versus the configured offset — acceptable for human reminders.

### 7.2 Per-recipient body customization

For each member:
- Fetch member's drafts and published responses for this cycle (AP15 — single Query).
- `N = published.len()`, `total = nl.lockedQuestionIds.len()`, `remaining = total - N`.
- Inject into the body template per §5.1.

Members who already published every answer get a different soft message: "{N}/{total} answers in. You're done — but you can still edit." Optional: skip them entirely. **Default: skip them at the 24h reminder, send them the 96h/48h ones.** Configurable later.

---

## 8. Publication fan-out

When `publish(nl)` runs in `lambda-cycle-tick`, async-invoke `lambda-push`'s `publication_fanout` with `{ groupId, cycleId }`.

Idempotency marker: `NOTIFIED#PUBLISH`. Same algorithm as §6 but with the `publication` payload.

This was not on the original requirement list (the spec only mentioned cycle-open + deadline reminders) but is the natural counterpart: users want to know when there's a new edition to read. **Default ON.** A `notificationSettings.onPublication: bool` group setting can disable it.

---

## 9. Test push

`POST /push/test`:
- Sends a `kind: test` push to all of the caller's subscriptions.
- Returns per-subscription delivery results (success / 410 / other error) for the Settings UI to display: "✓ Chrome on Pixel 8" / "✗ Safari on iPhone (resubscribe)".

---

## 10. Web Push delivery details (Rust)

Using the `web-push` crate (or `web-push-native` if it's the maintained one — pick at implementation time).

```rust
async fn send_push(sub: &PushSubscription, payload: &Payload) -> SendResult {
    let info = SubscriptionInfo::new(&sub.endpoint, &sub.p256dh, &sub.auth);
    let body = serde_json::to_vec(payload).unwrap();

    let mut builder = WebPushMessageBuilder::new(&info);
    builder.set_payload(ContentEncoding::Aes128Gcm, &body);
    builder.set_ttl(86400);                      // 24h: don't deliver if app reopens later
    builder.set_urgency(Urgency::Normal);
    let msg = builder.build()?;

    let signer = VapidSignatureBuilder::from_pem(VAPID_PRIVATE_KEY_PEM, &info)?
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
| 413 Payload Too Large | Should never happen (we cap payload at 4KB); log + drop |
| 429 Too Many Requests | Increment failure, retry next tick |
| 5xx | Increment failure, retry next tick |
| Network error | Increment failure, retry next tick |

---

## 11. Frontend wiring

### 11.1 Settings page UI

A single toggle "Push notifications" with:
- An "Allow" CTA that requests permission and creates a subscription.
- A "Send test push" button (after enabled).
- A "My devices" list with each subscription's user-agent string + last-success timestamp + delete button.
- Per-group preference toggles ("Cycle open", "Deadline reminders", "Publication") with `PUT /push/preferences/{g}`.

### 11.2 Re-subscribe-on-load

In `App.tsx`, after auth:

```ts
useEffect(() => {
  if (!user) return;
  ensurePushSubscription(config.vapidPublicKey).catch(reportError);
}, [user]);
```

`ensurePushSubscription` is a no-op if permission was previously denied; only refreshes the server-side subscription record if one exists locally.

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
  PushLatencyMs (p50/p95/p99)
```

Alarm: PushFailed/(PushSent+PushFailed) > 20% over 30 min → SNS to operator.

---

## 14. Failure modes & UX

- **Browser doesn't support Web Push (Safari < 16.4 on iOS)**: detect via `'PushManager' in window` and `'Notification' in window`. Settings UI hides the toggle and shows "Push isn't supported in this browser. The PWA still works — install it for the best experience."
- **Permission denied previously**: instruct the user how to re-enable in browser settings; no programmatic recovery.
- **All subscriptions invalid for a user**: their experience degrades gracefully — they still get the in-app cycle indicator on Home, and an unread badge on the newsletter list. (Unread tracking is itself a future enhancement; for v1 the absence of push just means they may miss the deadline.)
