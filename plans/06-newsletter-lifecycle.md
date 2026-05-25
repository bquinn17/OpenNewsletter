# 06 — Newsletter Lifecycle

This document is the source of truth for how a Newsletter (cycle) progresses from "voting on questions" to "published" to "archived". It defines the state machine, the timestamps, the EventBridge schedules, and the algorithm `lambda-cycle-tick` runs on each invocation.

---

## 1. States

| State | Meaning | Visible UI |
|---|---|---|
| `voting` | Group members are suggesting and upvoting candidate questions. No locked questions yet. | `/g/:g/upcoming` |
| `open` | Locked questions are set; response window is active. Members write/edit drafts; only their own drafts are visible. | `/g/:g/n/:c` (open layout) |
| `published` | Response window closed. Answers are visible to everyone. Comments and reactions enabled. | `/g/:g/n/:c` (published layout) |
| `archived` | Long-past edition moved out of hot storage. Read-only via archive blob. (Deferred — see `10-archival.md`.) | 410 with archive link |

---

## 2. Timestamps and transitions

Every Newsletter row carries six timestamps:

| Field | Set when | Meaning |
|---|---|---|
| `voteWindowOpenAt` | created | Earliest time candidates can be suggested for this cycle |
| `voteWindowCloseAt` | created | == `responseOpenAt`. Voting freezes at this instant. |
| `responseOpenAt` | created | When the cycle transitions `voting → open` |
| `responseCloseAt` | created | When the cycle transitions `open → published` |
| `publishedAt` | on publish | Actual transition timestamp (≈ `responseCloseAt`) |
| `nextTransitionAt` | on every state change | The next instant any automatic action is due. Drives GSI2 query. |

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

Given group settings `responseWindowDays` (default 4) and `timezone` (default `America/New_York`):

```
voteWindowOpenAt   = previous cycle's responseCloseAt
                     (or "now" for the very first cycle)
responseOpenAt     = first day of next month at 00:00 group-local
                     converted to UTC
voteWindowCloseAt  = responseOpenAt
responseCloseAt    = responseOpenAt + responseWindowDays days
nextTransitionAt   = responseOpenAt   (because we're in voting)
```

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
candidates = query GSI1
   gsi1pk = "GROUP#{nl.groupId}#CYCLE#{nl.cycleId}#VOTES"
   ScanIndexForward = false
   Limit = group.cycleSettings.questionsPerCycle

if admin curated (lockedQuestionIds already set on nl):
    use those instead of voted top-N

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
create_next_voting_cycle(group, after=nl.responseCloseAt)
fanout_cycle_open_notification(nl)   # see 07-notifications.md
```

If `TransactWriteItems` fails because the cycle's status changed concurrently (another tick in-flight), this run drops the item — the other run owns the transition.

If `chosen` is empty (no candidate questions submitted): the cycle still transitions to `open` with `lockedQuestionIds = []`. The UI shows an empty state ("Nobody suggested questions this month — see you next time!"). Members still get the cycle-open notification but it includes a note. The cycle then auto-publishes 4 days later as an empty edition.

### 5.3 `publish(nl)`

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

ARCHIVE_SENTINEL = a far-future ISO date (e.g., `9999-12-31T00:00:00Z`) so the row sits at the end of GSI2 and is ignored by the tick query. When the archival job is implemented, it will rewrite this for cycles older than the cutoff.

### 5.4 `create_next_voting_cycle(group, after)`

```
next_cycle_id = first_day_of_month_after(after, group.timezone) -> "yyyymm"
next_response_open = first_day_of_month_at_midnight(group.timezone, ymd) -> UTC
next_response_close = next_response_open + group.cycleSettings.responseWindowDays

put_if_not_exists(Newsletter(
    groupId = group.id,
    cycleId = next_cycle_id,
    status = "voting",
    voteWindowOpenAt = after,
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

When admins `PATCH /groups/{g}` to change `responseWindowDays`, `questionsPerCycle`, or `timezone`:

- Settings apply **only to cycles created after the change.**
- Currently-active cycles keep their pre-computed timestamps.
- The next cycle (in `voting`) gets its timestamps recomputed if it has no votes yet. If it already has candidate activity, the change applies starting two cycles out (avoid disorienting users mid-vote).
- This logic lives in `lambda-groups` `patch_group` handler.

If the timezone changes, the upcoming `voting` cycle's `responseOpenAt` may shift by hours. Acceptable, documented in admin UI ("Timezone changes affect the next cycle's open time").

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
| Cycle-tick runs while a user is publishing a draft | `PUT my-response` checks `Newsletter.status = open` inside its transaction (one of its precondition reads). If the cycle has just transitioned to `published`, the user gets `CYCLE_NOT_OPEN` 409 and their draft remains unpublished forever. SPA shows "This cycle has closed — your draft was not published." |
| Notification fan-out runs twice | Idempotency markers `NOTIFIED#OPEN` / `NOTIFIED#CLOSE#{offset}` prevent duplicates. See `07-notifications.md`. |

---

## 9. Empty-cycle edge cases

- **No candidate questions, no votes**: cycle transitions to `open` with empty `lockedQuestionIds`; transitions to `published` empty 4 days later. The UI renders an empty-state for both.
- **No responses**: cycle transitions to `published` with no answers. UI renders "No one responded this month." Comments/reactions still allowed (on poll questions if any) but irrelevant.
- **Group with 1 member**: works fine. The member can vote on their own questions, write their own answers, comment on themselves. Slightly silly, never broken.

---

## 10. Read-side helpers

In `persistence/newsletters.rs`:

- `find_voting_cycle(group_id) -> Option<Newsletter>` — returns the current `voting` cycle for a group. Used by candidate question endpoints to determine "the next cycle".
- `find_or_open_cycle(group_id) -> Option<Newsletter>` — returns the current `open` cycle or None.
- `list_recent(group_id, limit, cursor) -> (Vec<Newsletter>, Option<Cursor>)` — for `/groups/{g}/newsletters`.
- `recompute_my_counts(group_id, cycle_id, user_id) -> (drafts: usize, published: usize)` — uses AP15.

---

## 11. Tests required

Specified in detail in `11-testing-ci-cd.md`. The lifecycle-specific integration tests:

1. Tick promotes top-N candidates correctly.
2. Tick respects admin curation override.
3. Tick auto-publishes at deadline.
4. Tick is idempotent across rapid re-runs.
5. Empty-candidate cycle still opens and publishes.
6. Settings change mid-cycle applies to N+2 cycle, not the in-flight one.
7. Membership-cap enforcement at invite redemption.
8. Concurrent vote casting + admin candidate deletion produces consistent state.
9. DST transition: a group in `America/New_York` with `responseOpenAt` on 2026-03-01 lands at the right UTC instant given the spring-forward boundary.

---

## 12. Operational dashboards

The Monitoring stack dashboard widget set for the lifecycle includes:
- Cycles by status (count of items per `gsi2pk` value)
- `lambda-cycle-tick` invocation count, error count, duration p95
- Cycles that transitioned in the last 24h (custom EMF metric)
- Average lag between `nextTransitionAt` and the actual transition time (should be < 10 min given a 5-min tick)
