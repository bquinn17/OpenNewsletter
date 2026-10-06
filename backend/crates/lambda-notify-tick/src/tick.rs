//! The `lambda-notify-tick` deadline-reminder fan-out algorithm
//! (`plans/07-notifications.md` §7, M11 decision D5). Runs identically
//! whether triggered by the EventBridge schedule or the dev
//! `POST /admin/dev/tick/notify` route — both call [`run_notify_tick`].

use chrono::{DateTime, Duration, Utc};
use chrono_tz::Tz;
use domain::{Group, Newsletter, ResponseStatus};
use persistence::{groups, newsletters, push as push_repo, responses};
use push::metrics::FanoutMetrics;
use push::state::AppState;
use push::{delivery, payload};
use serde::Serialize;
use shared::config::MAX_REMINDER_OFFSET_HOURS;

/// One deadline-reminder fan-out this tick ran
/// (`plans/03-api-contract.md` §11a.2). This route/tick only ever reports
/// `deadline_reminder` — cycle-open and publication are fanned out from
/// `lambda-cycle-tick` via `lambda-push` directly.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotifyFanout {
    pub group_id: String,
    pub cycle_id: String,
    pub kind: &'static str,
    pub offset_hours: u32,
    pub delivered: u32,
    pub failed: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct NotifyTickSummary {
    pub fanouts: Vec<NotifyFanout>,
}

/// Run one full tick: find every `open` cycle whose `responseCloseAt` falls
/// within the configurable max offset, fan out at most one deadline
/// reminder per cycle, then record the sentinel.
pub async fn run_notify_tick(state: &AppState) -> NotifyTickSummary {
    let now = Utc::now();
    let mut fanouts = Vec::new();

    let candidates = match newsletters::list_open_cycles_closing_within(
        &state.repo,
        now,
        Duration::hours(MAX_REMINDER_OFFSET_HOURS as i64),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = ?e, "failed to query open cycles for notify tick");
            Vec::new()
        }
    };

    for nl in candidates {
        if let Some(fanout) = process_cycle(state, &nl, now).await {
            fanouts.push(fanout);
        }
    }

    if let Err(e) = push_repo::upsert_notify_tick_sentinel(&state.repo, now).await {
        tracing::error!(error = ?e, "failed to upsert the notify-tick sentinel row");
    }

    NotifyTickSummary { fanouts }
}

/// Claims the `NOTIFIED#CLOSE#{offset}` marker for every due-or-moot offset
/// in the group's configured list (moot ones are marked and never sent;
/// among due ones newly claimed *this* call, only the smallest is sent —
/// §7.1, D5), then fans out that one reminder to the group's members.
/// Returns `None` when no offset was due this tick (nothing to report).
async fn process_cycle(
    state: &AppState,
    nl: &Newsletter,
    now: DateTime<Utc>,
) -> Option<NotifyFanout> {
    let group = match groups::get_group(&state.repo, &nl.group_id).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            tracing::error!(group_id = %nl.group_id, "notify tick: group not found");
            return None;
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, "notify tick: group lookup failed");
            return None;
        }
    };

    let offsets = &group.notification_settings.offsets_hours_before_close;
    let smallest_offset_overall = *offsets.iter().min()?;

    let mut newly_due_offsets: Vec<u32> = Vec::new();
    for &offset in offsets {
        let target = nl.response_close_at - Duration::hours(offset as i64);
        let is_moot = target <= nl.response_open_at;

        if is_moot {
            if let Err(e) = push_repo::claim_close_notified_marker(
                &state.repo,
                &nl.group_id,
                &nl.cycle_id,
                offset,
                now,
            )
            .await
            {
                tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, offset_hours = offset, "failed to claim moot deadline-reminder marker");
            }
            continue;
        }

        let is_due = target <= now && now < nl.response_close_at;
        if !is_due {
            continue;
        }

        match push_repo::claim_close_notified_marker(
            &state.repo,
            &nl.group_id,
            &nl.cycle_id,
            offset,
            now,
        )
        .await
        {
            Ok(true) => newly_due_offsets.push(offset),
            Ok(false) => {} // already handled by an earlier tick
            Err(e) => {
                tracing::error!(error = ?e, group_id = %nl.group_id, cycle_id = %nl.cycle_id, offset_hours = offset, "failed to claim deadline-reminder marker");
            }
        }
    }

    let offset_to_send = *newly_due_offsets.iter().min()?;

    let tz: Tz = match group.timezone.parse() {
        Ok(tz) => tz,
        Err(_) => {
            tracing::error!(group_id = %nl.group_id, timezone = %group.timezone, "notify tick: invalid group timezone");
            return None;
        }
    };

    let fanout_metrics = fan_out_deadline_reminder(
        state,
        &group,
        nl,
        offset_to_send,
        smallest_offset_overall,
        tz,
    )
    .await;

    Some(NotifyFanout {
        group_id: nl.group_id.to_string(),
        cycle_id: nl.cycle_id.to_string(),
        kind: "deadline_reminder",
        offset_hours: offset_to_send,
        delivered: fanout_metrics.delivered_count(),
        failed: fanout_metrics.failed_count(),
    })
}

async fn fan_out_deadline_reminder(
    state: &AppState,
    group: &Group,
    nl: &Newsletter,
    offset_hours: u32,
    smallest_offset_overall: u32,
    tz: Tz,
) -> FanoutMetrics {
    let mut fanout_metrics = FanoutMetrics::new("deadline_reminder");

    let members = match groups::list_members_for_group(&state.repo, &nl.group_id).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, "notify tick: member listing failed");
            return fanout_metrics;
        }
    };

    let vapid = match state.vapid_keys().await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %nl.group_id, "notify tick: failed to load vapid keys");
            return fanout_metrics;
        }
    };

    let total_questions = nl.locked_question_ids.len() as u32;

    for member in members {
        let pref = match push_repo::get_pref(&state.repo, &member.user_id, &nl.group_id).await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = ?e, user_id = %member.user_id, group_id = %nl.group_id, "notify tick: pref lookup failed");
                continue;
            }
        };
        if !pref.map(|p| p.deadline_reminders).unwrap_or(true) {
            continue;
        }

        let published_count = match responses::list_my_responses_in_cycle(
            &state.repo,
            &member.user_id,
            &nl.group_id,
            &nl.cycle_id,
        )
        .await
        {
            Ok(list) => list
                .iter()
                .filter(|r| r.status == ResponseStatus::Published)
                .count() as u32,
            Err(e) => {
                tracing::error!(error = ?e, user_id = %member.user_id, group_id = %nl.group_id, "notify tick: response listing failed");
                continue;
            }
        };
        let is_done = total_questions > 0 && published_count >= total_questions;
        if is_done && offset_hours == smallest_offset_overall {
            continue;
        }

        let subs = match push_repo::list_subs_for_user(&state.repo, &member.user_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = ?e, user_id = %member.user_id, "notify tick: subscription listing failed");
                continue;
            }
        };
        if subs.is_empty() {
            continue;
        }

        let message = payload::deadline_reminder(
            &group.name,
            &nl.group_id,
            &nl.cycle_id,
            offset_hours,
            nl.response_close_at,
            tz,
            published_count,
            total_questions,
            is_done,
        );

        for sub in subs {
            let result = delivery::send_one(
                &state.repo,
                state.sender.as_ref(),
                vapid,
                &member.user_id,
                &sub,
                &message,
            )
            .await;
            fanout_metrics.record(result.outcome, result.latency_ms);
        }
    }

    fanout_metrics.emit();
    fanout_metrics
}
