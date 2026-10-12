# 06 — Newsletter Lifecycle

This document is the source of truth for how a Newsletter (cycle) progresses from "voting on questions" to "published" to "archived". It defines the state machine, the timestamps, the EventBridge schedules, and the algorithm `lambda-cycle-tick` runs on each invocation.

---

## 1. States

| State | Meaning | Visible UI |
|---|---|---|
| `voting` | Group members are suggesting and upvoting candidate questions. No locked questions yet. | `/g/:g/upcoming` |
| `open` | Locked questions are set; response window is active. Members write/edit drafts; only their own drafts are visible. | `/g/:g/n/:c` (open layout) |
| `published` | Response window closed. Answers are visible to everyone. Comments and reactions enabled. | `/g/:g/n/:c` (published layout) |
| `archived` | Long-past edition moved out of hot storage. Read-only via archive blob. (Deferred — see `10-archival.md`.) | 410 `NEWSLETTER_ARCHIVED`; the SPA shows an "archived" state |

---

## 2. Timestamps and transitions

Every Newsletter row carries six timestamps:

| Field | Set when | Meaning |
|---|---|---|
| `voteWindowOpenAt` | created | Earliest time candidates can be suggested for this cycle |
| `voteWindowCloseAt` | created, or recomputed by a `timezone` settings change on the current `voting` cycle (§6, D10) | == `responseOpenAt`. Voting freezes at this instant. |
| `responseOpenAt` | created, or recomputed by a `timezone` settings change on the current `voting` cycle (§6, D10) | When the cycle transitions `voting → open` |
| `responseCloseAt` | created, or recomputed by a `responseWindowDays`/`timezone` settings change on the current `voting` or `open` cycle (§6, D10) | When the cycle transitions `open → published` |
| `publishedAt` | on publish | Actual transition timestamp (≈ `responseCloseAt`) |
| `nextTransitionAt` | on every state change, or recomputed alongside the fields above (§6, D10) | The next instant any automatic action is due. Drives GSI2 query. |

`nextTransitionAt` always equals one of:
- in `voting` → `responseOpenAt`
- in `open` → `responseCloseAt`
- in `published` → `null` (or, in the future, the archival cutoff)

GSI2 keys are recomputed on every transition:
- `gsi2pk = NL_STATUS#{status}`
- `gsi2sk = {nextTransitionAt}#{groupId}#{cycleId}` (or a sentinel future date when terminal)

---

## 3. State machine

```
                ┌──────────┐    promote candidates,         ┌──────┐
                │  voting  │───►lock questions,             │ open │
                │          │    set status=open             │      │
                └─────┬────┘                                └───┬──┘
                      │                                         │
                      │ admin-only: cancel cycle (deferred)     │ deadline reached,
                      │                                         │ auto-publish
                      ▼                                         ▼
                                                          ┌────────────┐
                                                          │ published  │
                                                          │            │
                                                          └─────┬──────┘
                                                                │
                                                                │ archival job (deferred,
                                                                │ ~12 months later)
                                                                ▼
                                                          ┌────────────┐
                                                          │  archived  │
                                                          └────────────┘
```

Transitions are exclusively forward; there is no "unpublish."

---

## 4. Cycle creation and the calendar

For each group, exactly one `voting` Newsletter exists at any time (the one accepting candidates and votes). When that cycle transitions to `open`, the cycle-tick Lambda creates the *next* `voting` cycle so users can immediately start suggesting questions for the month after.

### 4.1 Cycle ID derivation

`cycleId = yyyymm` based on the **`responseOpenAt`** date in the group's local timezone.

E.g., a cycle whose `responseOpenAt` is 2026-06-01T00:00 in `America/New_York` has `cycleId = 202606`, regardless of UTC offset.

### 4.2 Default schedule per group

Given group settings `responseWindowDays` (default **7**, `DEFAULT_RESPONSE_WINDOW_DAYS` in `shared/src/config.rs`; decided 2026-10-07, lands in M13.5 — was 4) and `timezone` (default `America/New_York`):

```
voteWindowOpenAt   = the instant the cycle row is created
                     (i.e. when the previous cycle transitions to open;
                      "now" for the very first cycle)
responseOpenAt     = first day of next month at 00:00 group-local
                     converted to UTC
voteWindowCloseAt  = responseOpenAt
responseCloseAt    = responseOpenAt + responseWindowDays days
nextTransitionAt   = responseOpenAt   (because we're in voting)
```

`voteWindowOpenAt` is **informational only** — nothing enforces it. Enforcement is purely status-based: a cycle in `voting` accepts suggestions and votes from the moment its row exists (`CYCLE_NOT_VOTING` otherwise). Members can therefore suggest questions for month N+1 while month N's response window is still open — that is the intended "immediately start suggesting" behavior from §4.

For the very first cycle of a group (created on group creation):
- If the current month's start has already passed → the *next* month's first becomes `responseOpenAt`.
- The bootstrap script creates the group with the appropriate first cycle pre-seeded.

---

## 5. `lambda-cycle-tick`

EventBridge Scheduler rule fires every 5 minutes.

### 5.1 Algorithm

```python
# pseudocode in Python style; real impl is Rust
def tick():
    now = utc_now()

    # 1. Find any voting cycles whose vote window has closed
    voting_due = query_gsi2(
        gsi2pk="NL_STATUS#voting",
        gsi2sk_lte=f"{now.isoformat()}#~~~"
    )
    for nl in voting_due:
        promote_and_open(nl)

    # 2. Find any open cycles whose response window has closed
    open_due = query_gsi2(
        gsi2pk="NL_STATUS#open",
        gsi2sk_lte=f"{now.isoformat()}#~~~"
    )
    for nl in open_due:
        publish(nl)

    # 3. (Deferred) Find any published cycles eligible for archival.
    #    Skipped in v1 — see 10-archival.md.

    # 4. Update tick sentinel
    upsert(pk="TICK", sk="CYCLE", lastRanAt=now)
```

The tick is idempotent: if a previous run started but didn't finish, the next run picks up the same items (because we update `gsi2pk` only inside the transactional state change — items remain `voting` if the transition was never written).

### 5.2 `promote_and_open(nl)`

```
group = get_group(nl.groupId)
pool = query AP10   # pk=GROUP#{nl.groupId}#CYCLE#{nl.cycleId}, begins_with(sk, "QC#") — bounded by group membership
chosen = sort(pool, key=(-vote_count, submitted_at, question_id))[: group.cycleSettings.questionsPerCycle]

# Admins cannot override the voting outcome — see §7.

locked = []
for (idx, c) in enumerate(chosen):
    locked.append({
        questionId: c.questionId,
        kind: c.kind,
        prompt: c.prompt,
        pollOptions: c.pollOptions,
        displayOrder: idx,
        submittedBy: c.submittedBy,
        isAnonymous: c.isAnonymous,
        lockedAt: now,
    })

# Single TransactWriteItems
items = [
    Update(nl,
        Set: status="open",
              lockedQuestionIds=[ids],
              gsi2pk="NL_STATUS#open",
              gsi2sk=f"{nl.responseCloseAt}#{nl.groupId}#{nl.cycleId}"
        Condition: status="voting"),
    *[Put(LockedQuestion(...)) for each in locked],
]
write_transact(items)

# Outside the transaction (best-effort):
create_next_voting_cycle(group, after=nl.cycleId)
fanout_cycle_open_notification(nl)   # see 07-notifications.md
```

**Promotion order: oldest wins ties** (decided 2026-10-07; lands in M13.5, D4 — replaces the earlier GSI1-driven selection above). Candidates are ranked by `vote_count` DESC, then `submitted_at` ASC (oldest first), then `question_id` ASC as a final tiebreak. `promote_and_open` reads the full candidate pool via AP10 (bounded by group membership, not a scan) and sorts in memory, rather than using GSI1's `#VOTES` index (AP11): that index's `ScanIndexForward=false` order breaks vote-count ties *newest*-first, which doesn't match the "oldest wins" rule. `GET …/candidate-questions?sort=top` (`03` §6.1) uses the identical comparator so the UI's "top" ordering always agrees with what actually gets promoted. GSI1 `#VOTES` keys are still written on every vote (`02` §4 #1/#2); nothing currently reads them. 0-vote candidates still fill any remaining slots.

If `TransactWriteItems` fails because the cycle's status changed concurrently (another tick in-flight), this run drops the item — the other run owns the transition.

If `chosen` is empty (no candidate questions submitted): the cycle still transitions to `open` with `lockedQuestionIds = []`. The UI shows an empty state ("Nobody suggested questions this month — see you next time!"). Members still get the cycle-open notification but it includes a note. The cycle then auto-publishes 7 days later (decided 2026-10-07, D3 — was 4 days) as an empty edition.

### 5.3 `publish(nl)`

**Auto-publish non-empty drafts first** (decided 2026-10-07; lands in M13.5, D2; `02` §4 #4a). This step runs **before** the status-flip transaction below, and is the reason `publish(nl)` is crash-safe/idempotent across re-runs:

```
for q in locked_questions(nl):
    for answer in query_answers(nl.groupId, nl.cycleId, q.questionId):   # AP14
        if answer.status != "draft":
            continue
        is_non_empty = (
            (q.kind == "text" and (answer.body.strip() or answer.imageMediaIds)) or
            (q.kind == "poll" and answer.pollOptionId is not None)
        )
        if not is_non_empty:
            continue   # empty drafts stay draft forever (§7, 02 §7)
        # One TransactWriteItems PER DRAFT, not one big transaction for the
        # whole cycle: members × questions rows can exceed the 100-item cap.
        publish_draft = Update(answer,
            Condition: status="draft",
            Set: status="published", publishedAt=nl.responseCloseAt)
        try:
            TransactWriteItems([publish_draft,
                                *editions_answered_bump(answer.userId, nl.cycleId)])  # same guard as 02 §4 #4
        except TransactionCanceled as e:
            if e.only_failed("editions_answered_bump"):
                # member already counted this cycle: publish without the bump
                TransactWriteItems([publish_draft])
            elif e.failed("publish_draft"):
                pass   # already published by an earlier pass (crash-recovery re-run): harmless
            else:
                raise
```

Then the status flip:

```
items = [
    Update(nl,
        Set: status="published",
              publishedAt=now,
              gsi2pk="NL_STATUS#published",
              gsi2sk=ARCHIVE_SENTINEL,
              nextTransitionAt=null
        Condition: status="open"),
]
write_transact(items)

# Outside transaction:
fanout_publication_notification(nl)
```

**As built (M13.5):** the bump's condition also requires `attribute_exists(pk)` on the membership row. Otherwise a member who left the group would fail the transaction on every tick, and the cycle could never publish. A departed member's non-empty draft still publishes, with no bump. `list_answers` follows `LastEvaluatedKey`, because 50 members × 20k-char answers can exceed one 1 MB page.

**Broken drafts (decided 2026-10-08, review fix).** One draft that can't be published must not hold the group's edition back forever. Previously any sweep error left the cycle `open` and every tick retried and failed the same way. Nothing alarmed, because the tick logs per-cycle failures instead of failing the invocation. Now:
- The sweep carries on past an error and counts it. Every other draft still publishes on the first pass. A row that won't deserialize counts as a failure, and so does a question whose answers can't be listed (`responses::list_answers_lenient`).
- A sweep with failures runs `newsletters::record_failed_sweep`. That is an `ADD auto_publish_failed_sweeps 1` on the newsletter row, conditioned on `status = open`, and the counter is not a `Newsletter` field. The cycle then stays `open`, so the next tick retries the failed drafts.
- Once the counter reaches `AUTO_PUBLISH_MAX_FAILED_SWEEPS` (3, `shared::config`; about 15 minutes at the 5-minute tick), the cycle publishes anyway. The drafts that kept failing stay `draft`, and each one is logged at ERROR with its response id or sort key for manual repair. The tick emits the EMF metric `AutoPublishAbandoned`, which the cycle-tick alarm counts (`01` §9.2).
- A text draft with no body publishes only if one of its images still exists and hasn't failed processing. Otherwise a draft listing only deleted images would publish empty.

Because auto-publish runs before this condition-guarded flip, a tick that crashes partway through the draft-sweep leaves `nl.status` at `open`: the next tick's GSI2 query finds the cycle again and redoes the sweep (rows it already flipped just fail their `status="draft"` condition and are skipped). There is no race with a user's own save: `PUT my-response` already refuses writes once `now >= responseCloseAt` (`03` §7.3), even while `nl.status` still reads `open` in the gap before this tick runs, so the last successful save is exactly what the sweep above sees.

ARCHIVE_SENTINEL = a far-future ISO date (e.g., `9999-12-31T00:00:00Z`) so the row sits at the end of GSI2 and is ignored by the tick query. When the archival job is implemented, it will rewrite this for cycles older than the cutoff.

### 5.4 `create_next_voting_cycle(group, after)`

`after` is the current cycle's `cycleId` (`yyyymm`) — the parameter is named for what it anchors on, per the fix described below, not for a point in time.

```
after_month_start = first_day_of_month_at_midnight(group.timezone, after.yyyymm) -> UTC
next_cycle_id = first_day_of_month_after(after_month_start, group.timezone) -> "yyyymm"
next_response_open = first_day_of_month_at_midnight(group.timezone, next_cycle_id) -> UTC
next_response_close = next_response_open + group.cycleSettings.responseWindowDays

put_if_not_exists(Newsletter(
    groupId = group.id,
    cycleId = next_cycle_id,
    status = "voting",
    voteWindowOpenAt = utc_now(),   # informational only — see §4.2

    voteWindowCloseAt = next_response_open,
    responseOpenAt = next_response_open,
    responseCloseAt = next_response_close,
    publishedAt = null,
    nextTransitionAt = next_response_open,
    gsi2pk = "NL_STATUS#voting",
    gsi2sk = f"{next_response_open}#{group.id}#{next_cycle_id}",
))
```

`put_if_not_exists` (`ConditionExpression: attribute_not_exists(pk)`) guards against duplicate creation if two tick runs race.

**Successor repair (decided 2026-10-08).** Promotion calls this best-effort, after its own transaction commits (§5.2). If that call fails (a DynamoDB error, a timeout, a crash between the two writes), the cycle has already left the voting-due query, so nothing would ever retry it, and the group's lifecycle would stop after this edition publishes. So every tick, between the promote and publish passes, lists every `open` cycle and every `voting` cycle (`newsletters::list_cycles_in_status`, one GSI2 query each) and, for any `open` cycle whose group has no `voting` cycle, calls this function. It runs before publication so a cycle closing that tick is still checked. (Changed 2026-10-08, review fix: it used to read each open group's whole newsletter partition every tick.) GSI2 is eventually consistent, so a successor created moments earlier may not show yet; the conditional create then just loses its race. A cycle promoted moments earlier that still shows as `voting` defers that group's repair by one tick. Regression test: `lambda-questions/tests/lifecycle.rs::a_tick_recreates_a_missing_next_voting_cycle_for_an_open_cycle`.

The next cycle is anchored on the current cycle's **`cycleId`**, not on one of its timestamps. Anchoring on `responseCloseAt` skips a month: a 28-day window (`MAX_RESPONSE_WINDOW_DAYS`) opening Feb 1 in a non-leap year closes at Mar 1 00:00, and "the first of the month after" that is Apr 1. `responseOpenAt` would fix that case, but the dev-only deadline rewind (`03` §11a.1) moves a voting cycle's `responseOpenAt` back by an arbitrary amount, which could land the "next" cycle on the current cycle's own `cycleId`. The `yyyymm` is fixed at creation, so it always yields the following month. Implemented as `shared::cycle_time::cycle_month_start`.

### 5.5 Time math (Rust)

Use `chrono` + `chrono-tz` in the persistence/shared crates:

```rust
use chrono::{DateTime, Utc, TimeZone, Datelike};
use chrono_tz::Tz;

pub fn first_day_of_next_month_local(after_utc: DateTime<Utc>, tz: Tz) -> DateTime<Utc> {
    let local = after_utc.with_timezone(&tz);
    let (y, m) = if local.month() == 12 { (local.year() + 1, 1) } else { (local.year(), local.month() + 1) };
    let next_local = tz.with_ymd_and_hms(y, m, 1, 0, 0, 0).unwrap();
    next_local.with_timezone(&Utc)
}
```

DST transitions in the group's timezone are handled by `chrono-tz`. The contract: `responseOpenAt` is exactly midnight local on the 1st; the UTC offset adjusts naturally.

---

## 6. Settings changes mid-cycle

**Rewritten 2026-10-07; lands in M13.5 (D10).** Settings now apply to the *current* cycle wherever that's technically possible, rather than waiting for a cycle created after the change. This replaces the old "only cycles created after the change" / "two cycles out" rule (previous text preserved in git history).

When admins `PATCH /groups/{g}`, each field's rule is independent:

- **`questionsPerCycle`** — read at promotion time (`§5.2`), so it applies to the current `voting` cycle's next promotion. An already-`open` cycle's questions are locked and unaffected — there's nothing left to apply it to.
- **`votesPerUserPerCycle`** — read live by the vote-cast transaction (`02` §4 #1), so it applies immediately, including to votes already in flight. Lowering it below a member's existing vote count doesn't retract those votes; the member simply can't cast more until they're back under the cap.
- **`responseWindowDays`**:
  - Current `voting` cycle: recompute `responseCloseAt` (and `nextTransitionAt`, since it still equals `responseOpenAt` while voting — GSI2 keys are unaffected) **regardless of candidate activity**. The old "only if the cycle has no votes yet" gate is gone.
  - Current `open` cycle: recompute `responseCloseAt = responseOpenAt + days` and its GSI2 keys (`gsi2sk`) **only if the new close is still in the future**. If the recomputed close would be in the past, the request is rejected: 422 `VALIDATION_FAILED` with a `fieldErrors` entry on `responseWindowDays`, code `CLOSES_IN_PAST` ("would close the open cycle in the past") — see `03` §4.3. Reminder markers already claimed (`NOTIFIED#CLOSE#{offset}`) stay claimed; extending the window doesn't re-send a reminder that already went out.
- The `voting`-cycle rewrite for either field is a put conditioned on the row still being `voting` (`newsletters::reschedule_voting_cycle`; changed 2026-10-08, review fix). Otherwise a tick that promoted the cycle between the handler's read and its write would be undone, putting the row back to `voting` without its locked questions. A lost race is logged at INFO.
- **`timezone`** — recompute the current `voting` cycle's `responseOpenAt`/`voteWindowCloseAt`/`responseCloseAt`/`nextTransitionAt` regardless of candidate activity, **except** when the recomputed value would change the cycle's `cycleId` (candidates are keyed by cycle id, `02` §2.6) — in that case the row is left untouched and the change applies starting from the next cycle, same as the old escape hatch. An `open` cycle's instants are never touched by a timezone change (its `cycleId` is already fixed and its clock has already started).
- **`notificationSettings` and `memberSoftCap`** — already read live at the point of use; no change needed here.

This logic lives in `lambda-groups`'s `patch_group` handler.

If the timezone changes, the upcoming `voting` cycle's `responseOpenAt` may shift by hours. Acceptable, documented in admin UI ("Timezone changes affect the current voting cycle's open time").

**Implementation notes, carried forward from M5 and updated for D10:**
- `responseWindowDays` on the current `voting` cycle recomputes via `newsletters::write_status_transition` the same way the old "no candidate activity" path did — the difference is the gate is gone, not the mechanism.
- `responseWindowDays` on the current `open` cycle is a new code path: recompute-if-future-else-422, distinct from the `voting`-cycle path above.
- `cycleId` is derived from the recomputed `responseOpenAt` and is also the row's partition key (`sk = NL#{cycleId}`). A timezone change can, in principle, shift which local month a given creation instant falls into, changing the recomputed `cycleId`. Rather than move the row to a new key — which would orphan anything already referencing the old `cycleId` mid-request — `patch_group` compares the recomputed `cycleId` to the existing one and, if they differ, leaves the cycle's row completely untouched (the setting applies starting next cycle, per above). This is a deliberate simplification: it means a timezone change can occasionally take one cycle longer than the "applies now" rule above suggests, which is judged less surprising than a cycle's identity changing under a user's feet. (Unchanged from the pre-D10 text — this escape hatch was already timezone-specific, not a "two cycles out" blanket rule.)
- The admin Settings form copy changes to reflect "applies now" semantics instead of "applies to future cycles."

---

## 7. Manual admin overrides

Admins **cannot** override the cycle in v1. The tick is the source of truth: top-N voted candidates promote, the cycle closes after the response window, and the edition publishes. There is no admin-curate surface, no manual promote/demote of candidates, no editing of locked questions, no early publish, no rollback, no skip-month, and no cycle cancel.

The simpler rule is intentional: every member's vote counts equally and the schedule is predictable. If a question is genuinely problematic, an admin can delete the candidate while it's still in `voting` (cascading its votes); after promotion the question stays as it was suggested.

---

## 8. Race conditions and idempotency

| Race | Mitigation |
|---|---|
| Two tick instances run simultaneously | DDB transactions with `Condition: status="X"` ensure only one wins. Loser's transact returns `TransactionCanceledException` and is logged at INFO. |
| User submits/votes between the tick deciding to promote and the transaction committing | The transaction reads candidate snapshots before `TransactWriteItems`. A new vote that arrives mid-flight is simply not reflected in the chosen set. Users may notice a candidate they voted for "just barely" missed the cut — acceptable UX. |
| Cycle-tick runs while a user is publishing a draft | `PUT my-response` checks `Newsletter.status = open` **and** `now < responseCloseAt` inside its transaction (one of its precondition reads). If the cycle has just transitioned to `published` — or even while it still reads `open` but the deadline has passed — the user gets `CYCLE_NOT_OPEN` 409. Updated 2026-10-07 (D2, lands in M13.5): the save that was refused is lost, but it no longer means the answer never appears — the *last save that succeeded* is what `publish(nl)`'s auto-publish sweep (§5.3) picks up, and if that save was non-empty it gets published automatically. SPA shows "This cycle has closed — your edit was not saved" for the refused save itself. |
| Notification fan-out runs twice | Idempotency markers `NOTIFIED#OPEN` / `NOTIFIED#CLOSE#{offset}` / `NOTIFIED#PUBLISH`, claimed before sending, prevent duplicates. See `07-notifications.md`. |

---

## 9. Empty-cycle edge cases

- **No candidate questions, no votes**: cycle transitions to `open` with empty `lockedQuestionIds`; transitions to `published` empty 7 days later (decided 2026-10-07, D3 — was 4 days). The UI renders an empty-state for both.
- **No responses**: cycle transitions to `published` with no answers (the auto-publish sweep in §5.3 has nothing non-empty to find). UI renders "No one responded this month." There is nothing to comment on or react to: engagement targets published text answers only (`03` §8, `09` §3).
- **Group with 1 member**: works fine. The member can vote on their own questions, write their own answers, comment on themselves. Slightly silly, never broken.

---

## 10. Read-side helpers

In `persistence/newsletters.rs`:

- `find_voting_cycle(group_id) -> Option<Newsletter>` — returns the current `voting` cycle for a group. Used by candidate question endpoints to determine "the next cycle".
- `find_open_cycle(group_id) -> Option<Newsletter>` — returns the current `open` cycle or None.
- `list_recent(group_id, limit, cursor) -> (Vec<Newsletter>, Option<Cursor>)` — for `/groups/{g}/newsletters`.
- The per-user draft/published counts come from `responses::list_my_responses_in_cycle` (AP15, filtered by group; `02` §3).

---

## 11. Tests required

Specified in detail in `11-testing-ci-cd.md`. The lifecycle-specific integration tests:

1. Tick promotes top-N candidates correctly, in `vote_count` DESC / `submitted_at` ASC / `question_id` ASC order (D4) — including a tie-vote case where an older candidate must win over a newer one with the same count.
2. Tick auto-publishes at deadline.
3. Tick is idempotent across rapid re-runs.
4. Empty-candidate cycle still opens and publishes.
5. Settings change applies to the current cycle per §6 (updated 2026-10-07, D10 — was "applies to N+2 cycle, not the in-flight one"): a `voting`-cycle `responseWindowDays`/`timezone` change recomputes its timestamps regardless of candidate activity; an `open`-cycle `responseWindowDays` change recomputes `responseCloseAt` when the new value is still in the future, and is rejected with 422 when it would land in the past.
6. Membership-cap enforcement at invite redemption.
7. Concurrent vote casting + admin candidate deletion produces consistent state.
8. DST transition: a group in `America/New_York` with `responseOpenAt` on 2026-03-01 lands at the right UTC instant given the spring-forward boundary.
9. A 28-day window opening Feb 1 in a non-leap year seeds a March cycle, not April (§5.4).
10. Auto-publish (added 2026-10-07; lands in M13.5, D2): at close, a non-empty text draft and a non-empty poll draft both end up `published` with `publishedAt = responseCloseAt` and the `editionsAnswered` bump applied once; an empty draft stays `draft`; a crash between two drafts' auto-publish transactions (simulated by killing the tick mid-sweep) leaves the cycle `open`, and a re-run completes the remaining drafts without double-bumping `editionsAnswered` for the ones already flipped.

(There is deliberately no "admin curation override" test — that feature does not exist; see §7.)

---

## 12. Operational dashboards

The Monitoring stack dashboard widget set for the lifecycle includes:
- Cycles by status (count of items per `gsi2pk` value)
- `lambda-cycle-tick` invocation count, error count, duration p95
- Cycles that transitioned in the last 24h (custom EMF metric)
- Average lag between `nextTransitionAt` and the actual transition time (should be < 10 min given a 5-min tick)
