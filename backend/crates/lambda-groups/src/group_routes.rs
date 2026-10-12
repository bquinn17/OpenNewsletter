//! `GET /groups/{g}`, `PATCH /groups/{g}`, and the member-management routes
//! (`plans/03-api-contract.md` §3.5, §3.6, §4).

use crate::state::AppState;
use crate::validation;
use chrono::{DateTime, Duration, Utc};
use domain::api::{GroupResponse, MemberResponse, PatchGroupRequest, PatchMemberRequest};
use domain::{
    ApiError, ApiErrorCode, CycleId, FieldErrorCode, Group, GroupId, GroupMembership, Role, UserId,
};
use persistence::groups::GroupPatch;
use persistence::{auth, groups, newsletters, users, RepoError};

pub async fn get_group(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
) -> Result<GroupResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let group = load_group(state, group_id).await?;
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(group, members))
}

pub async fn patch_group(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    request: PatchGroupRequest,
) -> Result<GroupResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, true).await?;
    let group = load_group(state, group_id).await?;

    let patch = validation::group_patch(&group, request)?;
    // Validated before any write, so a 422 here leaves the group untouched.
    let open_cycle_close = open_cycle_close_change(state, group_id, &group, &patch).await?;
    groups::update_group(&state.repo, group_id, &patch)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group update failed");
            ApiError::internal("failed to update group")
        })?;

    let updated = load_group(state, group_id).await?;
    reschedule_voting_cycle_if_needed(state, group_id, &group, &updated).await;
    if let Some((cycle_id, new_close)) = open_cycle_close {
        move_open_cycle_close(state, group_id, &cycle_id, new_close).await;
    }
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(updated, members))
}

/// `plans/06-newsletter-lifecycle.md` §6 — when `responseWindowDays` changes
/// and the group has an `open` cycle, that cycle's close moves to
/// `responseOpenAt + days`. Returns the cycle and its new close, or a 422 on
/// `responseWindowDays` with code `CLOSES_IN_PAST` if the new close isn't in
/// the future.
async fn open_cycle_close_change(
    state: &AppState,
    group_id: &GroupId,
    current: &Group,
    patch: &GroupPatch,
) -> Result<Option<(CycleId, DateTime<Utc>)>, ApiError> {
    let Some(settings) = &patch.cycle_settings else {
        return Ok(None);
    };
    let days = settings.response_window_days;
    if days == current.cycle_settings.response_window_days {
        return Ok(None);
    }
    let open = newsletters::find_open_cycle(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "failed to load the open cycle");
            ApiError::internal("failed to load the open cycle")
        })?;
    let Some(open) = open else {
        return Ok(None);
    };
    let new_close = open.response_open_at + Duration::days(days.into());
    if new_close <= Utc::now() {
        return Err(ApiError::invalid_field_because(
            "responseWindowDays",
            FieldErrorCode::ClosesInPast,
            format!("responseWindowDays {days} would close the open cycle in the past"),
        ));
    }
    if new_close == open.response_close_at {
        return Ok(None);
    }
    Ok(Some((open.cycle_id, new_close)))
}

/// Best-effort, like the voting-cycle reschedule: the settings change itself
/// has already been saved.
async fn move_open_cycle_close(
    state: &AppState,
    group_id: &GroupId,
    cycle_id: &CycleId,
    new_close: DateTime<Utc>,
) {
    match newsletters::reschedule_open_cycle_close(&state.repo, group_id, cycle_id, new_close).await
    {
        Ok(()) => {}
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %group_id, cycle_id = %cycle_id, "open cycle published before its close could move");
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %cycle_id, "failed to move the open cycle's close");
        }
    }
}

/// `plans/06-newsletter-lifecycle.md` §6 — a `timezone`/`responseWindowDays`
/// change recomputes the group's current `voting` cycle's schedule, whether
/// or not it already has candidates.
///
/// `cycleId` is derived from the recomputed `responseOpenAt` and doubles as
/// the row's partition key. If recomputing would change it (a timezone shift
/// can push the same creation instant into a different local month), the row
/// is left untouched rather than moved to a new key — silently renaming an
/// in-flight cycle out from under anyone looking at its old id would be more
/// disorienting than deferring the setting by one cycle.
///
/// Best-effort: a failure here is logged but does not fail the settings PATCH,
/// consistent with how `create_next_voting_cycle` is a best-effort step
/// outside the tick's core transaction (`06-newsletter-lifecycle.md` §5.2).
async fn reschedule_voting_cycle_if_needed(
    state: &AppState,
    group_id: &GroupId,
    previous: &Group,
    updated: &Group,
) {
    let schedule_changed = previous.timezone != updated.timezone
        || previous.cycle_settings.response_window_days
            != updated.cycle_settings.response_window_days;
    if !schedule_changed {
        return;
    }

    let voting = match newsletters::find_voting_cycle(&state.repo, group_id).await {
        Ok(Some(nl)) => nl,
        Ok(None) => return,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to load voting cycle for reschedule");
            return;
        }
    };

    let Ok(tz) = updated.timezone.parse::<chrono_tz::Tz>() else {
        tracing::error!(group_id = %group_id, timezone = %updated.timezone, "validated timezone failed to parse during reschedule");
        return;
    };

    let schedule = shared::cycle_time::next_cycle_schedule(
        voting.vote_window_open_at,
        tz,
        updated.cycle_settings.response_window_days,
    );

    if schedule.cycle_id != voting.cycle_id {
        tracing::info!(
            group_id = %group_id,
            old_cycle_id = %voting.cycle_id,
            new_cycle_id = %schedule.cycle_id,
            "timezone change would move the voting cycle's id; leaving it unchanged this cycle"
        );
        return;
    }

    let mut rescheduled = voting;
    rescheduled.response_open_at = schedule.response_open_at;
    rescheduled.vote_window_close_at = schedule.response_open_at;
    rescheduled.response_close_at = schedule.response_close_at;
    rescheduled.next_transition_at = Some(schedule.response_open_at);

    match newsletters::reschedule_voting_cycle(&state.repo, &rescheduled).await {
        Ok(()) => {}
        Err(e) if e.is_lost_race() => {
            tracing::info!(group_id = %group_id, cycle_id = %rescheduled.cycle_id, "voting cycle promoted before it could be rescheduled");
        }
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, "failed to reschedule voting cycle");
        }
    }
}

/// `DELETE /groups/{g}/members/{u}` — a member removing themselves, or an admin
/// removing someone else. Either way the group must keep at least one admin.
pub async fn remove_member(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    target: &UserId,
) -> Result<(), ApiError> {
    let is_self = caller == target;
    let caller_membership =
        auth::require_membership(&state.repo, caller, group_id, !is_self).await?;

    let target_membership = if is_self {
        caller_membership
    } else {
        groups::get_membership(&state.repo, target, group_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %group_id, "membership lookup failed");
                ApiError::internal("failed to load membership")
            })?
            .ok_or_else(|| ApiError::not_found("member not found in this group"))?
    };

    let witness = if target_membership.role == Role::Admin {
        Some(admin_witness(state, group_id, caller, target).await?)
    } else {
        None
    };

    groups::leave_group_tx(&state.repo, target, group_id, witness.as_ref())
        .await
        .map_err(|e| admin_change_error(e, witness.is_some(), group_id, target, "remove member"))?;
    auth::invalidate_membership(target, group_id);
    Ok(())
}

/// `PATCH /groups/{g}/members/{u}` — admin-only role change.
pub async fn patch_member(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    target: &UserId,
    request: PatchMemberRequest,
) -> Result<MemberResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, true).await?;

    let membership = groups::get_membership(&state.repo, target, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "membership lookup failed");
            ApiError::internal("failed to load membership")
        })?
        .ok_or_else(|| ApiError::not_found("member not found in this group"))?;

    let is_demotion = membership.role == Role::Admin && request.role == Role::Member;
    let witness = if is_demotion {
        Some(admin_witness(state, group_id, caller, target).await?)
    } else {
        None
    };

    groups::update_membership_role(
        &state.repo,
        target,
        group_id,
        request.role,
        witness.as_ref(),
    )
    .await
    .map_err(|e| {
        admin_change_error(e, witness.is_some(), group_id, target, "change member role")
    })?;
    auth::invalidate_membership(target, group_id);

    let user = users::get_user(&state.repo, target)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %target, "user lookup failed");
            ApiError::internal("failed to load member profile")
        })?
        .ok_or_else(|| ApiError::not_found("member profile not found"))?;

    Ok(MemberResponse {
        user_id: user.user_id,
        display_name: user.display_name,
        role: request.role,
        avatar_color: user.avatar_color,
        avatar_url: user
            .avatar_media_id
            .as_ref()
            .and_then(|id| state.avatar_url(id)),
        joined_at: membership.joined_at,
        editions_answered: membership.editions_answered,
    })
}

async fn load_group(state: &AppState, group_id: &GroupId) -> Result<Group, ApiError> {
    groups::get_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group lookup failed");
            ApiError::internal("failed to load group")
        })?
        .ok_or_else(|| ApiError::not_found("group not found"))
}

async fn list_members(
    state: &AppState,
    group_id: &GroupId,
) -> Result<Vec<GroupMembership>, ApiError> {
    groups::list_members_for_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "member listing failed");
            ApiError::internal("failed to list members")
        })
}

/// Pick the admin whose role the removal/demotion of admin `target` is
/// conditioned on, so the group can never commit a state with zero admins
/// (`02` §4, last-admin guard). An admin acting on someone else is their own
/// witness, already verified by `require_membership(.., true)`; an admin acting
/// on themselves needs another admin, or gets `LAST_ADMIN`.
async fn admin_witness(
    state: &AppState,
    group_id: &GroupId,
    caller: &UserId,
    target: &UserId,
) -> Result<UserId, ApiError> {
    if caller != target {
        return Ok(caller.clone());
    }
    list_members(state, group_id)
        .await?
        .into_iter()
        .find(|m| m.role == Role::Admin && m.user_id != *target)
        .map(|m| m.user_id)
        .ok_or_else(|| {
            ApiError::new(
                ApiErrorCode::LastAdmin,
                "a group must keep at least one admin",
            )
        })
}

/// Map a failed membership write. With a witness in the transaction, a
/// cancellation means a concurrent admin change won the race (e.g. two admins
/// demoting each other), so it surfaces as `LAST_ADMIN`.
fn admin_change_error(
    err: RepoError,
    had_witness: bool,
    group_id: &GroupId,
    target: &UserId,
    action: &str,
) -> ApiError {
    if had_witness && err.is_lost_race() {
        tracing::info!(error = ?err, group_id = %group_id, user_id = %target, "{action} lost an admin race");
        return ApiError::new(
            ApiErrorCode::LastAdmin,
            "another admin change happened at the same time; reload and try again",
        );
    }
    tracing::error!(error = ?err, group_id = %group_id, user_id = %target, "{action} failed");
    ApiError::internal(format!("failed to {action}"))
}

async fn hydrate_members(
    state: &AppState,
    group_id: &GroupId,
) -> Result<Vec<MemberResponse>, ApiError> {
    let memberships = list_members(state, group_id).await?;
    let user_ids: Vec<UserId> = memberships.iter().map(|m| m.user_id.clone()).collect();

    let profiles = users::get_users_batch(&state.repo, &user_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "member profile batch failed");
            ApiError::internal("failed to load member profiles")
        })?;

    Ok(memberships
        .into_iter()
        .filter_map(|membership| {
            let user = profiles.get(&membership.user_id)?;
            Some(MemberResponse {
                user_id: membership.user_id,
                display_name: user.display_name.clone(),
                role: membership.role,
                avatar_color: user.avatar_color.clone(),
                avatar_url: user
                    .avatar_media_id
                    .as_ref()
                    .and_then(|id| state.avatar_url(id)),
                joined_at: membership.joined_at,
                editions_answered: membership.editions_answered,
            })
        })
        .collect())
}
