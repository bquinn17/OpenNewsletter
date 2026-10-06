//! Cycle-open / publication notification fan-outs, triggered by
//! `lambda-cycle-tick`'s async invoke of this Lambda
//! (`plans/07-notifications.md` §6, §8; M11 decisions D2-D4, D9).

use crate::payload::{self, PushPayload};
use crate::state::AppState;
use crate::{delivery, metrics::FanoutMetrics};
use chrono::Utc;
use domain::{
    CycleId, Group, GroupId, Newsletter, NewsletterStatus, NotificationPref, QuestionKind,
    ResponseStatus,
};
use persistence::{groups, newsletters, push as push_repo, questions, responses};
use shared::config::PUSH_FANOUT_SEND_CUTOFF_SECS;
use std::time::{Duration, Instant};

/// `cycle_open_fanout` (`07-notifications.md` §6.1). Skipped entirely when
/// `group.notificationSettings.onCycleOpen` is false; per member skipped
/// when their `NotificationPref.cycleOpen` is false or they have zero
/// subscriptions. At-most-once via the `NOTIFIED#OPEN` marker.
pub async fn cycle_open_fanout(state: &AppState, group_id: &GroupId, cycle_id: &CycleId) {
    let cutoff = send_cutoff();
    if !claim_marker(state, group_id, cycle_id, Marker::Open).await {
        return;
    }

    let Some(group) = load_group(state, group_id).await else {
        return;
    };
    let Some(nl) = load_newsletter(state, group_id, cycle_id).await else {
        return;
    };

    if !group.notification_settings.on_cycle_open {
        tracing::debug!(group_id = %group_id, cycle_id = %cycle_id, "cycle-open push disabled for this group");
        return;
    }

    let total_questions = nl.locked_question_ids.len() as u32;
    let window_hours = (nl.response_close_at - nl.response_open_at)
        .num_hours()
        .max(0);
    let window_days = ((window_hours as f64) / 24.0).round().max(1.0) as u32;
    let payload = payload::cycle_open(
        &group.name,
        group_id,
        cycle_id,
        total_questions,
        window_days,
    );

    let mut fanout_metrics = FanoutMetrics::new("cycle_open");
    fan_out_to_members(
        state,
        group_id,
        &payload,
        cycle_open_should_send,
        cutoff,
        &mut fanout_metrics,
    )
    .await;
    fanout_metrics.emit();
}

/// `publication_fanout` (`07-notifications.md` §8). Always on — no group
/// switch, no per-user preference. At-most-once via the `NOTIFIED#PUBLISH`
/// marker.
pub async fn publication_fanout(state: &AppState, group_id: &GroupId, cycle_id: &CycleId) {
    let cutoff = send_cutoff();
    if !claim_marker(state, group_id, cycle_id, Marker::Publish).await {
        return;
    }

    let Some(group) = load_group(state, group_id).await else {
        return;
    };
    let Some(nl) = load_newsletter(state, group_id, cycle_id).await else {
        return;
    };
    if nl.status != NewsletterStatus::Published {
        tracing::error!(group_id = %group_id, cycle_id = %cycle_id, status = ?nl.status, "publication fanout invoked for a non-published cycle");
        return;
    }

    let locked = match questions::list_locked_questions(&state.repo, group_id, cycle_id).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "failed to list locked questions for publication fanout");
            return;
        }
    };
    let polls = locked
        .iter()
        .filter(|q| q.kind == QuestionKind::Poll)
        .count() as u32;

    let mut answers = 0u32;
    for q in locked.iter().filter(|q| q.kind == QuestionKind::Text) {
        match responses::list_answers(&state.repo, group_id, cycle_id, &q.question_id).await {
            Ok(list) => {
                answers += list
                    .iter()
                    .filter(|r| r.status == ResponseStatus::Published)
                    .count() as u32
            }
            Err(e) => {
                tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, question_id = %q.question_id, "failed to count answers for publication fanout");
            }
        }
    }

    let payload = payload::publication(&group.name, group_id, cycle_id, answers, polls);

    let mut fanout_metrics = FanoutMetrics::new("publication");
    fan_out_to_members(
        state,
        group_id,
        &payload,
        |_| true,
        cutoff,
        &mut fanout_metrics,
    )
    .await;
    fanout_metrics.emit();
}

/// The send cutoff for this invocation, measured from its start
/// (`PUSH_FANOUT_SEND_CUTOFF_SECS`). The marker is claimed before sending,
/// so a retry never resumes a cut-off fan-out — the cutoff instead keeps
/// the Lambda from being killed partway, and `delivery::send_all` logs and
/// counts anything it had to skip.
fn send_cutoff() -> Instant {
    Instant::now() + Duration::from_secs(PUSH_FANOUT_SEND_CUTOFF_SECS)
}

enum Marker {
    Open,
    Publish,
}

async fn claim_marker(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    marker: Marker,
) -> bool {
    let now = Utc::now();
    let claimed = match marker {
        Marker::Open => {
            push_repo::claim_open_notified_marker(&state.repo, group_id, cycle_id, now).await
        }
        Marker::Publish => {
            push_repo::claim_publish_notified_marker(&state.repo, group_id, cycle_id, now).await
        }
    };
    match claimed {
        Ok(true) => true,
        Ok(false) => {
            tracing::info!(group_id = %group_id, cycle_id = %cycle_id, "notification already fanned out for this cycle");
            false
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "failed to claim notification marker");
            false
        }
    }
}

fn cycle_open_should_send(pref: Option<&NotificationPref>) -> bool {
    pref.map(|p| p.cycle_open).unwrap_or(true)
}

async fn load_group(state: &AppState, group_id: &GroupId) -> Option<Group> {
    match groups::get_group(&state.repo, group_id).await {
        Ok(Some(g)) => Some(g),
        Ok(None) => {
            tracing::error!(group_id = %group_id, "fanout: group not found");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "fanout: group lookup failed");
            None
        }
    }
}

async fn load_newsletter(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Option<Newsletter> {
    match newsletters::get_newsletter(&state.repo, group_id, cycle_id).await {
        Ok(Some(nl)) => Some(nl),
        Ok(None) => {
            tracing::error!(group_id = %group_id, cycle_id = %cycle_id, "fanout: newsletter not found");
            None
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "fanout: newsletter lookup failed");
            None
        }
    }
}

/// Fan-out over a group's members (`07-notifications.md` §6.2): resolve the
/// VAPID keys once, then for each member apply `should_send` to their
/// (possibly absent) `NotificationPref` and gather their subscriptions;
/// finally send the same `payload` to every gathered subscription
/// concurrently via `delivery::send_all`, starting none after `cutoff`.
async fn fan_out_to_members(
    state: &AppState,
    group_id: &GroupId,
    payload: &PushPayload,
    should_send: impl Fn(Option<&NotificationPref>) -> bool,
    cutoff: Instant,
    fanout_metrics: &mut FanoutMetrics,
) {
    let members = match groups::list_members_for_group(&state.repo, group_id).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "fanout: member listing failed");
            return;
        }
    };

    let vapid = match state.vapid_keys().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "fanout: failed to load vapid keys");
            return;
        }
    };

    let mut targets = Vec::new();
    for member in members {
        let pref = match push_repo::get_pref(&state.repo, &member.user_id, group_id).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = ?e, user_id = %member.user_id, group_id = %group_id, "fanout: pref lookup failed");
                continue;
            }
        };
        if !should_send(pref.as_ref()) {
            continue;
        }

        let subs = match push_repo::list_subs_for_user(&state.repo, &member.user_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = ?e, user_id = %member.user_id, "fanout: subscription listing failed");
                continue;
            }
        };
        targets.extend(subs.into_iter().map(|sub| (member.user_id.clone(), sub)));
    }

    let deliveries: Vec<_> = targets
        .iter()
        .map(|(user_id, sub)| delivery::Delivery {
            user_id,
            sub,
            payload,
        })
        .collect();
    let results = delivery::send_all(
        &state.repo,
        state.sender.as_ref(),
        vapid,
        &deliveries,
        cutoff,
    )
    .await;
    fanout_metrics.record_all(&results);
}
