//! `GET /groups/{g}`, `PATCH /groups/{g}`, and the member-management routes
//! (`plans/03-api-contract.md` §3.5, §3.6, §4).

use crate::state::AppState;
use crate::validation;
use domain::api::{GroupResponse, MemberResponse, PatchGroupRequest, PatchMemberRequest};
use domain::{ApiError, ApiErrorCode, Group, GroupId, GroupMembership, Role, UserId};
use persistence::{auth, groups, newsletters, questions, users};

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
    groups::update_group(&state.repo, group_id, &patch)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group update failed");
            ApiError::internal("failed to update group")
        })?;

    let updated = load_group(state, group_id).await?;
    reschedule_voting_cycle_if_needed(state, group_id, &group, &updated).await;
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(updated, members))
}

/// `plans/06-newsletter-lifecycle.md` §6 — a `timezone`/`responseWindowDays`
/// change recomputes the group's current `voting` cycle's schedule, but only
/// when nobody has suggested a candidate question for it yet; a cycle with
/// candidate activity keeps its original timestamps (the change instead takes
/// effect on the cycle created after this one transitions to `open`, since
/// `create_next_voting_cycle` always reads the group's settings fresh).
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

    let candidates = match questions::list_candidates(&state.repo, group_id, &voting.cycle_id).await
    {
        Ok(list) => list,
        Err(e) => {
            tracing::error!(error = ?e, group_id = %group_id, cycle_id = %voting.cycle_id, "failed to list candidates for reschedule");
            return;
        }
    };
    if !candidates.is_empty() {
        tracing::info!(
            group_id = %group_id,
            cycle_id = %voting.cycle_id,
            "settings changed but the voting cycle already has candidates; leaving its schedule as-is"
        );
        return;
    }

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

    if let Err(e) = newsletters::write_status_transition(&state.repo, &rescheduled).await {
        tracing::error!(error = ?e, group_id = %group_id, "failed to reschedule voting cycle");
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

    if target_membership.role == Role::Admin && admin_count(state, group_id).await? <= 1 {
        return Err(ApiError::new(
            ApiErrorCode::LastAdmin,
            "a group must keep at least one admin",
        ));
    }

    groups::leave_group_tx(&state.repo, target, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, user_id = %target, "leave group failed");
            ApiError::internal("failed to remove member")
        })?;
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
    if is_demotion && admin_count(state, group_id).await? <= 1 {
        return Err(ApiError::new(
            ApiErrorCode::LastAdmin,
            "a group must keep at least one admin",
        ));
    }

    groups::update_membership_role(&state.repo, target, group_id, request.role)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, user_id = %target, "role change failed");
            ApiError::internal("failed to change member role")
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

async fn admin_count(state: &AppState, group_id: &GroupId) -> Result<usize, ApiError> {
    let members = list_members(state, group_id).await?;
    Ok(members.iter().filter(|m| m.role == Role::Admin).count())
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
