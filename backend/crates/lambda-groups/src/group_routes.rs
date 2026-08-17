//! `GET /groups/{g}`, `PATCH /groups/{g}`, and the member-management routes
//! (`plans/03-api-contract.md` §3.5, §3.6, §4).

use crate::dto::{GroupResponse, MemberResponse, PatchGroupRequest, PatchMemberRequest};
use crate::state::AppState;
use crate::validation;
use domain::{ApiError, ApiErrorCode, Group, GroupId, GroupMembership, Role, UserId};
use persistence::{auth, groups, users};

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
    let members = hydrate_members(state, group_id).await?;
    Ok(GroupResponse::new(updated, members))
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
