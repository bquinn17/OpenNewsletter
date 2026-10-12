//! The `lambda-cycle-tick` algorithm itself (`plans/06-newsletter-lifecycle.md` §5.1).
//!
//! Runs identically whether triggered by the EventBridge schedule or the dev
//! `POST /admin/dev/tick/cycle` route — both call [`run_tick`].

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use domain::{
    CandidateQuestion, CycleId, GroupId, LockedQuestion, MediaStatus, Newsletter, NewsletterStatus,
    QuestionKind, Response, ResponseStatus,
};
use persistence::responses::AutoPublishOutcome;
use persistence::{groups, media, newsletters, questions, responses, Repo, RepoError};
use serde::Serialize;
use std::collections::HashSet;

use crate::notify::{FanoutKind, NoopNotifier, Notifier};

/// One state change this tick made, or a new cycle it created. `from: null`
/// marks a freshly created `voting` cycle rather than an existing row's
/// transition (`plans/03-api-contract.md` §11a.1).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub group_id: String,
    pub cycle_id: String,
    pub from: Option<&'static str>,
    pub to: &'static str,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TickSummary {
    pub transitions: Vec<Transition>,
}

/// Run one full tick: promote due `voting` cycles, publish due `open` cycles,
/// then record the sentinel. Every per-cycle failure is logged and skipped —
/// one group's bad data never blocks another group's tick (§8). Fan-outs go
/// nowhere; see [`run_tick_with`].
pub async fn run_tick(repo: &Repo) -> TickSummary {
    run_tick_with(repo, &NoopNotifier).await
}

/// [`run_tick`], triggering a notification fan-out through `notifier` after
/// each cycle opens or publishes.
pub async fn run_tick_with(repo: &Repo, notifier: &dyn Notifier) -> TickSummary {
    let now = Utc::now();
    let mut transitions = Vec::new();

    match newsletters::list_cycles_due(repo, NewsletterStatus::Voting, now).await {
        Ok(due) => {
            for nl in due {
                transitions.extend(promote_and_open(repo, notifier, &nl, now).await);
            }
        }
        Err(e) => tracing::error!(error = ?e, "failed to query voting cycles due for promotion"),
    }

    // Before publication, so a cycle closing this tick is still `open` here.
    // One GSI2 query for every group's `voting` cycle rather than a per-group
    // lookup: the successor almost always exists, so this is usually a no-op.
    // GSI2 is eventually consistent. A successor created moments ago that
    // doesn't show yet only means a create that loses its race. A cycle
    // promoted moments ago that still shows as `voting` only defers its
    // group's repair by one tick.
    let open = newsletters::list_cycles_in_status(repo, NewsletterStatus::Open).await;
    let voting = newsletters::list_cycles_in_status(repo, NewsletterStatus::Voting).await;
    match (open, voting) {
        (Ok(open), Ok(voting)) => {
            let has_voting: HashSet<GroupId> = voting.into_iter().map(|nl| nl.group_id).collect();
            for nl in open.iter().filter(|nl| !has_voting.contains(&nl.group_id)) {
                transitions.extend(ensure_next_voting_cycle(repo, nl).await);
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
    // `10-archival.md` and deliberately not attempted here.

    if let Err(e) = newsletters::upsert_tick_sentinel(repo, now).await {
        tracing::error!(error = ?e, "failed to upsert the tick sentinel row");
    }

    TickSummary { transitions }
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
) -> Vec<Transition> {
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
    let mut chosen = match questions::list_candidates(repo, &nl.group_id, &nl.cycle_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to list candidates");
            return Vec::new();
        }
    };
    chosen.sort_by(CandidateQuestion::promotion_order);
    chosen.truncate(group.cycle_settings.questions_per_cycle as usize);

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
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to promote candidates");
            return Vec::new();
        }
    }

    notifier
        .fanout(FanoutKind::CycleOpen, &nl.group_id, &nl.cycle_id)
        .await;

    let mut transitions = vec![Transition {
        group_id: nl.group_id.to_string(),
        cycle_id: nl.cycle_id.to_string(),
        from: Some("voting"),
        to: "open",
    }];

    if let Some(created) = create_next_voting_cycle(repo, &nl.group_id, &group, &nl.cycle_id).await
    {
        transitions.push(Transition {
            group_id: nl.group_id.to_string(),
            cycle_id: created.to_string(),
            from: None,
            to: "voting",
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
async fn ensure_next_voting_cycle(repo: &Repo, nl: &Newsletter) -> Option<Transition> {
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

    let created = create_next_voting_cycle(repo, &nl.group_id, &group, &nl.cycle_id).await?;
    tracing::warn!(group_id = %nl.group_id, cycle_id = %created, "repaired a missing next voting cycle");
    Some(Transition {
        group_id: nl.group_id.to_string(),
        cycle_id: created.to_string(),
        from: None,
        to: "voting",
    })
}

/// §5.4, best-effort: creates the cycle for the month after `current`.
/// Returns the new cycle's id when this call actually created it (as opposed
/// to losing a create race, or failing outright).
async fn create_next_voting_cycle(
    repo: &Repo,
    group_id: &GroupId,
    group: &domain::Group,
    current: &CycleId,
) -> Option<CycleId> {
    let tz: Tz = match group.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::error!(group_id = %group_id, timezone = %group.timezone, "group has an invalid timezone; cannot create its next voting cycle");
            return None;
        }
    };
    let Some(after) = shared::cycle_time::cycle_month_start(current, tz) else {
        tracing::error!(group_id = %group_id, cycle_id = %current, "cycle id is not a valid yyyymm; cannot create its next voting cycle");
        return None;
    };

    match newsletters::create_next_voting_cycle(
        repo,
        group_id,
        tz,
        group.cycle_settings.response_window_days,
        after,
    )
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
) -> Option<Transition> {
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

    match newsletters::publish_cycle(repo, &nl.group_id, &nl.cycle_id, now).await {
        Ok(()) => {
            notifier
                .fanout(FanoutKind::Publication, &nl.group_id, &nl.cycle_id)
                .await;
            Some(Transition {
                group_id: nl.group_id.to_string(),
                cycle_id: nl.cycle_id.to_string(),
                from: Some("open"),
                to: "published",
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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SweepReport {
    pub published: u32,
    /// Drafts left unpublished by an error. An answer row that won't
    /// deserialize counts as one, and so does a question whose answers
    /// couldn't be listed at all.
    pub failed: u32,
}

/// §5.3's first step (`02` §4 #4a): publish each locked question's non-empty
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
        for draft in answers
            .iter()
            .filter(|a| a.status == ResponseStatus::Draft && a.is_non_empty())
        {
            match auto_publish_one(repo, nl, draft).await {
                Ok(true) => report.published += 1,
                Ok(false) => {}
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

/// Publish one draft if it has live content. True when this call published it.
async fn auto_publish_one(
    repo: &Repo,
    nl: &Newsletter,
    draft: &Response,
) -> Result<bool, RepoError> {
    if !has_live_content(repo, draft).await? {
        return Ok(false);
    }
    let outcome = responses::auto_publish_draft(repo, draft, nl.response_close_at).await?;
    Ok(matches!(outcome, AutoPublishOutcome::Published { .. }))
}

/// One EMF log line: drafts the sweep gave up on when it published a cycle
/// without them (`01` §9.2's cycle-tick alarm counts these). CloudWatch Logs
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

/// [`Response::is_non_empty`] trusts `image_media_ids`, but a draft can still
/// list an image deleted since its last save: a stale tab, a failed autosave or
/// an older PWA. A text draft with no body publishes only if at least one of
/// its images still exists and hasn't failed processing. Otherwise it would
/// publish as an empty answer.
async fn has_live_content(repo: &Repo, draft: &Response) -> Result<bool, RepoError> {
    let has_body = draft.body.as_deref().is_some_and(|b| !b.trim().is_empty());
    if draft.kind != QuestionKind::Text || has_body {
        return Ok(true);
    }
    for image_id in &draft.image_media_ids {
        let image = media::get_image(repo, &draft.group_id, &draft.cycle_id, image_id).await?;
        if image.is_some_and(|i| i.status != MediaStatus::Failed) {
            return Ok(true);
        }
    }
    tracing::info!(group_id = %draft.group_id, cycle_id = %draft.cycle_id, response_id = %draft.response_id, "skipping a draft whose only images are gone");
    Ok(false)
}
