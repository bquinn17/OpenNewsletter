//! `GET /groups/{g}`, `PATCH /groups/{g}`, and the member-management routes
//! (`docs/design/api.md` §3.5, §3.6, §4).

use crate::state::AppState;
use crate::validation;
use domain::api::{GroupResponse, MemberResponse, PatchGroupRequest, PatchMemberRequest};
use domain::{ApiError, ApiErrorCode, Group, GroupId, GroupMembership, Role, UserId};
use persistence::{auth, groups, users, RepoError};

pub async fn get_group(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
) -> Result<GroupResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, false).await?;
    let group = load_group(state, group_id).await?;
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(
        clamp_response_window(state, group),
        members,
    ))
}

pub async fn patch_group(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
    request: PatchGroupRequest,
) -> Result<GroupResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, true).await?;
    let group = load_group(state, group_id).await?;

    let patch = validation::group_patch(&group, request, state.cadence)?;
    groups::update_group(&state.repo, group_id, &patch)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group update failed");
            ApiError::internal("failed to update group")
        })?;

    let updated = load_group(state, group_id).await?;
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(
        clamp_response_window(state, updated),
        members,
    ))
}

/// A stored `responseWindowDays` can predate a cadence switch (e.g. a
/// monthly deployment's value inherited across a later switch to weekly) —
/// `GET`/`PATCH /groups/{g}` report the effective, cadence-capped value
/// without rewriting the stored one (`docs/design/lifecycle.md` §6).
fn clamp_response_window(state: &AppState, mut group: Group) -> Group {
    group.cycle_settings.response_window_days = state
        .cadence
        .clamp_window(group.cycle_settings.response_window_days);
    group
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

    let avatar_url = state
        .media_urls
        .optional_avatar_url(user.avatar_media_id.as_ref())
        .await?;
    Ok(MemberResponse {
        user_id: user.user_id,
        display_name: user.display_name,
        role: request.role,
        avatar_color: user.avatar_color,
        avatar_url,
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
/// (`docs/design/data-model.md` §4, last-admin guard). An admin acting on someone else is their own
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

    let mut out = Vec::with_capacity(memberships.len());
    for membership in memberships {
        let Some(user) = profiles.get(&membership.user_id) else {
            continue;
        };
        let avatar_url = state
            .media_urls
            .optional_avatar_url(user.avatar_media_id.as_ref())
            .await?;
        out.push(MemberResponse {
            user_id: membership.user_id,
            display_name: user.display_name.clone(),
            role: membership.role,
            avatar_color: user.avatar_color.clone(),
            avatar_url,
            joined_at: membership.joined_at,
            editions_answered: membership.editions_answered,
        });
    }
    Ok(out)
}
