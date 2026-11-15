//! The `lambda-cycle-tick` algorithm itself (`docs/design/lifecycle.md` §5.1).
//!
//! Runs identically whether triggered by the EventBridge schedule or the dev
//! `POST /admin/dev/tick/cycle` route — both call [`run_tick`].

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use domain::api::{CycleTransition, DevTickCycleResponse};
use domain::{
    CandidateQuestion, CycleId, GroupId, LockedQuestion, MemberStats, Newsletter, NewsletterStatus,
    Response, ResponseStatus, UserId,
};
use futures::StreamExt;
use persistence::responses::AutoPublishOutcome;
use persistence::{groups, member_stats, newsletters, questions, responses, Repo, RepoError};
use shared::config::STREAK_SETTLE_CONCURRENCY;
use shared::cycle_time::CycleCadence;
use std::collections::{HashMap, HashSet};

use crate::notify::{FanoutKind, NoopNotifier, Notifier};

/// Run one full tick: promote due `voting` cycles, publish due `open` cycles,
/// then record the sentinel. Every per-cycle failure is logged and skipped —
/// one group's bad data never blocks another group's tick (§8). Fan-outs go
/// nowhere; see [`run_tick_with`].
pub async fn run_tick(repo: &Repo, cadence: CycleCadence) -> DevTickCycleResponse {
    run_tick_with(repo, &NoopNotifier, cadence).await
}

/// [`run_tick`], triggering a notification fan-out through `notifier` after
/// each cycle opens or publishes.
pub async fn run_tick_with(
    repo: &Repo,
    notifier: &dyn Notifier,
    cadence: CycleCadence,
) -> DevTickCycleResponse {
    let now = Utc::now();
    let mut transitions = Vec::new();

    transitions.extend(create_first_cycles(repo, cadence).await);

    match newsletters::list_cycles_due(repo, NewsletterStatus::Voting, now).await {
        Ok(due) => {
            for nl in due {
                transitions.extend(promote_and_open(repo, notifier, &nl, now, cadence).await);
            }
        }
        Err(e) => tracing::error!(error = ?e, "failed to query voting cycles due for promotion"),
    }

    // Before publication, so a cycle closing this tick is still `open` here.
    // One GSI2 query for every group's `voting` cycle rather than a per-group
    // lookup: the successor almost always exists, so this is usually a no-op.
    // The two queries run concurrently, and the `voting` one reads only group
    // ids. GSI2 is eventually consistent. A successor created moments ago that
    // doesn't show yet only means a create that loses its race. A cycle
    // promoted moments ago that still shows as `voting` only defers its
    // group's repair by one tick.
    let (open, has_voting) = tokio::join!(
        newsletters::list_cycles_in_status_lenient(repo, NewsletterStatus::Open),
        newsletters::list_group_ids_in_status(repo, NewsletterStatus::Voting),
    );
    match (open, has_voting) {
        (Ok((open, unreadable)), Ok(has_voting)) => {
            for bad in unreadable {
                tracing::error!(pk = %bad.pk, sk = %bad.sk, error = %bad.error, "skipping an unreadable open cycle in successor repair");
            }
            for nl in open.iter().filter(|nl| !has_voting.contains(&nl.group_id)) {
                transitions.extend(ensure_next_voting_cycle(repo, nl, cadence).await);
            }
        }
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!(error = ?e, "failed to query cycles for successor repair");
        }
    }

    match newsletters::list_cycles_due(repo, NewsletterStatus::Open, now).await {
        Ok(due) => {
            for nl in due {
                transitions.extend(publish(repo, notifier, &nl, now).await);
            }
        }
        Err(e) => tracing::error!(error = ?e, "failed to query open cycles due for publication"),
    }

    // Archival (published cycles eligible for cold storage) is deferred to
    // `docs/design/archival.md` and deliberately not attempted here.

    if let Err(e) = newsletters::upsert_tick_sentinel(repo, now).await {
        tracing::error!(error = ?e, "failed to upsert the tick sentinel row");
    }

    DevTickCycleResponse { transitions }
}

/// §4.2 — create the first `voting` cycle for every group the
/// bootstrap/seed scripts left "awaiting first cycle"
/// (`persistence::groups::list_groups_awaiting_first_cycle`), before the
/// promotion pass. Per-group failures are logged and skipped, never failing
/// the tick, like every other pass.
async fn create_first_cycles(repo: &Repo, cadence: CycleCadence) -> Vec<CycleTransition> {
    let awaiting = match groups::list_groups_awaiting_first_cycle(repo).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = ?e, "failed to query groups awaiting their first cycle");
            return Vec::new();
        }
    };

    let mut transitions = Vec::new();
    for group_id in awaiting {
        if let Some(created) = create_first_cycle(repo, &group_id, cadence).await {
            transitions.push(created);
        }
    }
    transitions
}

/// One group's first-cycle creation. A marked group that already has a
/// `voting` or `open` cycle (e.g. a script re-run that rewrote the META row
/// of a live group) just has its marker cleared, so it never gets a second
/// `voting` cycle.
async fn create_first_cycle(
    repo: &Repo,
    group_id: &GroupId,
    cadence: CycleCadence,
) -> Option<CycleTransition> {
    let group = match groups::get_group(repo, group_id).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            tracing::error!(group_id = %group_id, "group awaiting its first cycle is missing its META row");
            return None;
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to load group awaiting its first cycle");
            return None;
        }
    };

    let has_cycle = match group_has_a_cycle(repo, group_id).await {
        Ok(has_cycle) => has_cycle,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to check for an existing cycle before creating the first voting cycle");
            return None;
        }
    };
    if has_cycle {
        match groups::clear_awaiting_first_cycle_marker(repo, group_id).await {
            Ok(()) => {}
            Err(e) if e.is_lost_race() => {
                tracing::info!(group_id = %group_id, "first-cycle marker already cleared by a concurrent tick");
            }
            Err(e) => {
                tracing::error!(error = ?e, group_id = %group_id, "failed to clear the first-cycle marker");
            }
        }
        return None;
    }

    let tz: Tz = match group.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::error!(group_id = %group_id, timezone = %group.timezone, "group has an invalid timezone; cannot create its first voting cycle");
            return None;
        }
    };
    let schedule = shared::cycle_time::next_cycle_schedule(
        cadence,
        Utc::now(),
        tz,
        group.cycle_settings.response_window_days,
    );

    match newsletters::create_first_voting_cycle(repo, group_id, schedule, &group.cycle_settings)
        .await
    {
        Ok(nl) => Some(CycleTransition {
            group_id: group_id.clone(),
            cycle_id: nl.cycle_id,
            from: None,
            to: NewsletterStatus::Voting,
        }),
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %group_id, "first voting cycle already created by a concurrent tick");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to create the first voting cycle");
            None
        }
    }
}

async fn group_has_a_cycle(repo: &Repo, group_id: &GroupId) -> Result<bool, RepoError> {
    if newsletters::find_voting_cycle(repo, group_id)
        .await?
        .is_some()
    {
        return Ok(true);
    }
    Ok(newsletters::find_open_cycle(repo, group_id)
        .await?
        .is_some())
}

/// §5.2 — promote the top-N candidates into locked questions, flip the cycle
/// to `open`, then (best-effort, outside that transaction) create the group's
/// next `voting` cycle. Returns 0-2 transitions: the `open` flip, and — if a
/// next cycle didn't already exist — its creation.
async fn promote_and_open(
    repo: &Repo,
    notifier: &dyn Notifier,
    nl: &Newsletter,
    now: DateTime<Utc>,
    cadence: CycleCadence,
) -> Vec<CycleTransition> {
    let group = match groups::get_group(repo, &nl.group_id).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            tracing::error!(group_id = %nl.group_id, "voting cycle references a missing group");
            return Vec::new();
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, "failed to load group for promotion");
            return Vec::new();
        }
    };

    // The whole pool, sorted in memory: GSI1's vote-count order breaks ties
    // newest-first, but promotion breaks them oldest-first (§5.2).
    // Lenient: an unreadable candidate is logged and left out rather than
    // holding the cycle in `voting` on every tick.
    let mut chosen = match questions::list_candidates_lenient(repo, &nl.group_id, &nl.cycle_id)
        .await
    {
        Ok((candidates, unreadable)) => {
            for bad in unreadable {
                tracing::error!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, sk = %bad.sk, error = %bad.error, "skipping an unreadable candidate at promotion");
            }
            candidates
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to list candidates");
            return Vec::new();
        }
    };
    chosen.sort_by(CandidateQuestion::promotion_order);
    chosen.truncate(nl.questions_to_promote(&group.cycle_settings) as usize);

    let locked: Vec<LockedQuestion> = chosen
        .into_iter()
        .enumerate()
        .map(|(idx, c)| LockedQuestion {
            question_id: c.question_id,
            group_id: c.group_id,
            cycle_id: nl.cycle_id.clone(),
            kind: c.kind,
            prompt: c.prompt,
            poll_options: c.poll_options,
            display_order: idx as u32,
            submitted_by: c.submitted_by,
            is_anonymous: c.is_anonymous,
            locked_at: now,
        })
        .collect();

    let mut opened = nl.clone();
    opened.status = NewsletterStatus::Open;
    opened.locked_question_ids = locked.iter().map(|l| l.question_id.clone()).collect();
    opened.next_transition_at = Some(nl.response_close_at);

    match questions::promote_candidates_tx(repo, &locked, &opened).await {
        Ok(()) => {}
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, "promotion lost the race to a concurrent tick");
            return Vec::new();
        }
        Err(e @ RepoError::TransactionConflict(_)) => {
            tracing::warn!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "promotion kept conflicting with concurrent writes; the next tick retries");
            return Vec::new();
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to promote candidates");
            return Vec::new();
        }
    }

    notifier
        .fanout(FanoutKind::CycleOpen, &nl.group_id, &nl.cycle_id)
        .await;

    let mut transitions = vec![CycleTransition {
        group_id: nl.group_id.clone(),
        cycle_id: nl.cycle_id.clone(),
        from: Some(NewsletterStatus::Voting),
        to: NewsletterStatus::Open,
    }];

    if let Some(created) = create_next_voting_cycle(
        repo,
        &nl.group_id,
        &group,
        cadence,
        &nl.cycle_id,
        nl.response_close_at,
    )
    .await
    {
        transitions.push(CycleTransition {
            group_id: nl.group_id.clone(),
            cycle_id: created,
            from: None,
            to: NewsletterStatus::Voting,
        });
    }

    transitions
}

/// §5.4 repair: an `open` cycle's group must also have a `voting` cycle.
/// Promotion creates it best-effort after its own transaction commits, and
/// nothing else would ever retry a failed create — the cycle has left the
/// voting-due query — so the group's lifecycle would stop after this edition
/// publishes. Called only for an `open` cycle whose group showed no `voting`
/// cycle on GSI2 this tick.
async fn ensure_next_voting_cycle(
    repo: &Repo,
    nl: &Newsletter,
    cadence: CycleCadence,
) -> Option<CycleTransition> {
    let group = match groups::get_group(repo, &nl.group_id).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            tracing::error!(group_id = %nl.group_id, "open cycle references a missing group");
            return None;
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, "failed to load group for successor repair");
            return None;
        }
    };

    let created = create_next_voting_cycle(
        repo,
        &nl.group_id,
        &group,
        cadence,
        &nl.cycle_id,
        nl.response_close_at,
    )
    .await?;
    tracing::warn!(group_id = %nl.group_id, cycle_id = %created, "repaired a missing next voting cycle");
    Some(CycleTransition {
        group_id: nl.group_id.clone(),
        cycle_id: created,
        from: None,
        to: NewsletterStatus::Voting,
    })
}

/// §5.4/§13, best-effort: creates `current`'s successor cycle. Returns the
/// new cycle's id when this call actually created it (as opposed to losing a
/// create race, or failing outright).
async fn create_next_voting_cycle(
    repo: &Repo,
    group_id: &GroupId,
    group: &domain::Group,
    cadence: CycleCadence,
    current: &CycleId,
    current_close: DateTime<Utc>,
) -> Option<CycleId> {
    let tz: Tz = match group.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::error!(group_id = %group_id, timezone = %group.timezone, "group has an invalid timezone; cannot create its next voting cycle");
            return None;
        }
    };
    let Some(schedule) = shared::cycle_time::successor_schedule(
        cadence,
        current,
        current_close,
        tz,
        group.cycle_settings.response_window_days,
    ) else {
        tracing::error!(group_id = %group_id, cycle_id = %current, "cycle id is not valid under this deployment's cadence; cannot create its next voting cycle");
        return None;
    };

    match newsletters::create_next_voting_cycle(repo, group_id, schedule, &group.cycle_settings)
        .await
    {
        Ok(nl) => Some(nl.cycle_id),
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %group_id, "next voting cycle already created by a concurrent tick");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to create next voting cycle");
            None
        }
    }
}

/// §5.3 — auto-publish every non-empty draft, then `open -> published`.
///
/// A sweep that leaves drafts unpublished by an error counts against the
/// cycle (`newsletters::record_failed_sweep`) and keeps it `open`, so the next
/// tick retries them. After `AUTO_PUBLISH_MAX_FAILED_SWEEPS` such sweeps the
/// cycle publishes anyway, without the drafts that kept failing: one broken
/// row mustn't hold a group's edition back forever. Giving up emits the
/// `AutoPublishAbandoned` metric, which the cycle-tick alarm watches.
async fn publish(
    repo: &Repo,
    notifier: &dyn Notifier,
    nl: &Newsletter,
    now: DateTime<Utc>,
) -> Option<CycleTransition> {
    let sweep = auto_publish_drafts(repo, nl).await;
    if sweep.failed > 0 {
        let max = shared::config::AUTO_PUBLISH_MAX_FAILED_SWEEPS;
        match newsletters::record_failed_sweep(repo, &nl.group_id, &nl.cycle_id).await {
            Ok(attempt) if attempt < max => {
                tracing::warn!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, failed = sweep.failed, attempt, max, "auto-publish left drafts unpublished; cycle left open for the next tick");
                return None;
            }
            Ok(attempt) => {
                tracing::error!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, failed = sweep.failed, attempt, "auto-publish kept failing; publishing the cycle without those drafts");
                emit_auto_publish_abandoned(sweep.failed);
            }
            Err(e) if e.is_lost_race() => {
                tracing::info!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, "cycle published by a concurrent tick");
                return None;
            }
            Err(e) => {
                tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to record the failed auto-publish sweep; cycle left open");
                return None;
            }
        }
    }

    settle_streaks(repo, nl, &sweep.authors).await;

    match newsletters::publish_cycle(repo, &nl.group_id, &nl.cycle_id, now).await {
        Ok(()) => {
            notifier
                .fanout(FanoutKind::Publication, &nl.group_id, &nl.cycle_id)
                .await;
            Some(CycleTransition {
                group_id: nl.group_id.clone(),
                cycle_id: nl.cycle_id.clone(),
                from: Some(NewsletterStatus::Open),
                to: NewsletterStatus::Published,
            })
        }
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, "publish lost the race to a concurrent tick");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to publish cycle");
            None
        }
    }
}

/// What one auto-publish sweep did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SweepReport {
    pub published: u32,
    /// Drafts left unpublished by an error. An answer row that won't
    /// deserialize counts as one, and so does a question whose answers
    /// couldn't be listed at all.
    pub failed: u32,
    /// Every user with at least one `published` answer to a locked question,
    /// after the sweep — i.e. answers that were already published plus
    /// drafts this pass just published. Feeds streak settlement (§5.3): the
    /// AP14 reads this sweep already did are reused instead of re-listing.
    pub authors: HashSet<UserId>,
}

/// §5.3's first step (`docs/design/data-model.md` §4 #4a): publish each locked question's non-empty
/// drafts, one transaction per draft, with `publishedAt = responseCloseAt`.
/// Safe to re-run: a row an earlier pass already published is skipped. An
/// error on one draft is logged and counted, and the sweep carries on, so
/// every other draft still publishes.
pub async fn auto_publish_drafts(repo: &Repo, nl: &Newsletter) -> SweepReport {
    let mut report = SweepReport::default();
    for question_id in &nl.locked_question_ids {
        let (answers, unreadable) = match responses::list_answers_lenient(
            repo,
            &nl.group_id,
            &nl.cycle_id,
            question_id,
        )
        .await
        {
            Ok(listing) => listing,
            Err(e) => {
                tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, question_id = %question_id, "failed to list answers for auto-publish");
                report.failed += 1;
                continue;
            }
        };
        for row in unreadable {
            tracing::error!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, question_id = %question_id, sk = %row.sk, error = %row.error, "unreadable answer row; can't auto-publish it");
            report.failed += 1;
        }
        for answer in &answers {
            if answer.status == ResponseStatus::Published {
                report.authors.insert(answer.user_id.clone());
            }
        }
        for draft in answers
            .iter()
            .filter(|a| a.status == ResponseStatus::Draft && a.is_non_empty())
        {
            match auto_publish_one(repo, nl, draft).await {
                Ok(Some(AutoPublishOutcome::Published { .. })) => {
                    report.published += 1;
                    report.authors.insert(draft.user_id.clone());
                }
                // A concurrent pass published it after this one listed it, so
                // the author did answer: leaving them out would break their
                // streak, and `streak_cycle` would keep the other pass from
                // correcting it.
                Ok(Some(AutoPublishOutcome::AlreadyPublished)) => {
                    report.authors.insert(draft.user_id.clone());
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, response_id = %draft.response_id, "failed to auto-publish draft");
                    report.failed += 1;
                }
            }
        }
    }
    if report.published > 0 {
        tracing::info!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, published = report.published, "auto-published non-empty drafts");
    }
    report
}

/// Publish one draft if it has live content. `None` when it was skipped for
/// having none.
async fn auto_publish_one(
    repo: &Repo,
    nl: &Newsletter,
    draft: &Response,
) -> Result<Option<AutoPublishOutcome>, RepoError> {
    if !responses::has_live_content(repo, draft).await? {
        tracing::info!(group_id = %draft.group_id, cycle_id = %draft.cycle_id, response_id = %draft.response_id, "skipping a draft whose only images are gone");
        return Ok(None);
    }
    responses::auto_publish_draft(repo, draft, nl.response_close_at)
        .await
        .map(Some)
}

/// §5.3 — settle every current member's streak for this edition, after the
/// auto-publish sweep has decided to proceed and before the status flips to
/// `published` (`docs/design/data-model.md` §4 call-out under §2.17;
/// `docs/design/lifecycle.md` §5.3). Skipped entirely for an empty edition
/// (`lifecycle.md` §9: no locked questions) — it neither extends nor breaks a
/// streak. `authors` is [`SweepReport::authors`]: every user with at least
/// one published answer to a locked question, after the sweep.
///
/// Members settle concurrently, up to `STREAK_SETTLE_CONCURRENCY` at a time.
/// Best-effort: an error settling one member, or listing members/stats for
/// the whole step, is logged at ERROR and never stops the publish that
/// follows. `scripts/backfill_member_stats.py` repairs a missed settlement.
///
/// `pub` (not just used by `publish`) so integration tests can simulate a
/// crashed-then-rerun pass the same way `auto_publish_drafts` is exposed for
/// that purpose.
pub async fn settle_streaks(repo: &Repo, nl: &Newsletter, authors: &HashSet<UserId>) {
    if nl.locked_question_ids.is_empty() {
        return;
    }

    let members = match groups::list_members_for_group(repo, &nl.group_id).await {
        Ok(members) => members,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to list members for streak settlement; skipping this edition's streaks");
            return;
        }
    };

    let stats: HashMap<UserId, MemberStats> = match member_stats::list_stats_for_group(
        repo,
        &nl.group_id,
    )
    .await
    {
        Ok(rows) => rows.into_iter().map(|s| (s.user_id.clone(), s)).collect(),
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to list member stats for streak settlement; skipping this edition's streaks");
            return;
        }
    };

    let settlements = members.iter().filter_map(|member| {
        let existing = stats.get(&member.user_id);
        if existing.and_then(|s| s.streak_cycle.as_ref()) == Some(&nl.cycle_id) {
            // Already settled by an earlier, crashed pass over this cycle.
            return None;
        }

        let prior_current = existing.map_or(0, |s| s.current_streak);
        let prior_longest = existing.map_or(0, |s| s.longest_streak);
        let (current, longest) = if authors.contains(&member.user_id) {
            let current = prior_current + 1;
            (current, prior_longest.max(current))
        } else {
            (0, prior_longest)
        };

        Some(async move {
            let user_id = &member.user_id;
            match member_stats::settle_streak(
                repo,
                &nl.group_id,
                user_id,
                current,
                longest,
                &nl.cycle_id,
            )
            .await
            {
                Ok(()) => {}
                Err(e) if e.is_lost_race() => {
                    tracing::info!(group_id = %nl.group_id, cycle_id = %nl.cycle_id, user_id = %user_id, "member's streak already settled for this cycle by an earlier pass, or they left the group");
                }
                Err(e) => {
                    tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, user_id = %user_id, "failed to settle member's streak");
                }
            }
        })
    });
    futures::stream::iter(settlements)
        .buffer_unordered(STREAK_SETTLE_CONCURRENCY)
        .collect::<()>()
        .await;
}

/// One EMF log line: drafts the sweep gave up on when it published a cycle
/// without them (`docs/design/infrastructure.md` §9.2's cycle-tick alarm counts these). CloudWatch Logs
/// extracts the metric from stdout, so there's no `PutMetricData` call.
fn emit_auto_publish_abandoned(count: u32) {
    let env = std::env::var("ENV").unwrap_or_default();
    let line = serde_json::json!({
        "_aws": {
            "Timestamp": Utc::now().timestamp_millis(),
            "CloudWatchMetrics": [{
                "Namespace": format!("OpenNewsletter/{env}"),
                "Dimensions": [[]],
                "Metrics": [{"Name": "AutoPublishAbandoned", "Unit": "Count"}],
            }],
        },
        "AutoPublishAbandoned": count,
    });
    println!("{line}");
}
