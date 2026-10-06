//! The `lambda-cycle-tick` algorithm itself (`plans/06-newsletter-lifecycle.md` §5.1).
//!
//! Runs identically whether triggered by the EventBridge schedule or the dev
//! `POST /admin/dev/tick/cycle` route — both call [`run_tick`].

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use domain::{CycleId, GroupId, LockedQuestion, Newsletter, NewsletterStatus};
use persistence::{groups, newsletters, questions, Repo};
use serde::Serialize;

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

    let chosen = match questions::list_top_candidates_by_votes(
        repo,
        &nl.group_id,
        &nl.cycle_id,
        group.cycle_settings.questions_per_cycle as i32,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, "failed to list top candidates");
            return Vec::new();
        }
    };

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

    if let Some(created) =
        create_next_voting_cycle(repo, &nl.group_id, &group, nl.response_close_at).await
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

/// §5.4, best-effort. Returns the new cycle's id when this call actually
/// created it (as opposed to losing a create race, or failing outright).
async fn create_next_voting_cycle(
    repo: &Repo,
    group_id: &GroupId,
    group: &domain::Group,
    after: DateTime<Utc>,
) -> Option<CycleId> {
    let tz: Tz = match group.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::error!(group_id = %group_id, timezone = %group.timezone, "group has an invalid timezone; cannot create its next voting cycle");
            return None;
        }
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

/// §5.3 — `open -> published`.
async fn publish(
    repo: &Repo,
    notifier: &dyn Notifier,
    nl: &Newsletter,
    now: DateTime<Utc>,
) -> Option<Transition> {
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
