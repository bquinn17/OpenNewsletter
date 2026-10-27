//! `GET /config`, `GET /me`, `PATCH /me` (`docs/design/api.md` §2).

use crate::state::AppState;
use crate::validation;
use domain::api::{ConfigResponse, MembershipSummary, PatchMeRequest, UserResponse};
use domain::{ApiError, AvatarId, GroupMembership, MediaStatus, User, UserId};
use persistence::{auth, groups, media, users, RepoError};
use shared::http::AuthClaims;

pub async fn get_config(state: &AppState, claims: &AuthClaims) -> Result<ConfigResponse, ApiError> {
    let base = ConfigResponse {
        user_id: None,
        email: claims.email.clone(),
        display_name: claims.name.clone(),
        vapid_public_key: state.vapid_public_key.clone(),
        group_defaults: state.group_defaults.clone(),
        memberships: Vec::new(),
    };

    // A valid JWT with no user row is the normal pre-onboarding state: the SPA
    // reads the empty membership list and routes the caller to /join.
    let Some(user_id) = auth::resolve_user_id(&state.repo, &claims.sub)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "cognito sub lookup failed");
            ApiError::internal("failed to resolve caller")
        })?
    else {
        return Ok(base);
    };

    let user = load_user(state, &user_id).await?;
    let memberships = membership_summaries(state, &user_id).await?;

    Ok(ConfigResponse {
        user_id: Some(user.user_id),
        email: Some(user.email),
        display_name: Some(user.display_name),
        memberships,
        ..base
    })
}

pub async fn get_me(state: &AppState, user_id: &UserId) -> Result<UserResponse, ApiError> {
    let user = load_user(state, user_id).await?;
    let avatar_url = state
        .media_urls
        .optional_avatar_url(user.avatar_media_id.as_ref())
        .await?;
    Ok(UserResponse::new(user, avatar_url))
}

pub async fn patch_me(
    state: &AppState,
    user_id: &UserId,
    request: PatchMeRequest,
) -> Result<UserResponse, ApiError> {
    let display_name = request
        .display_name
        .as_deref()
        .map(validation::display_name)
        .transpose()?;

    if let Some(color) = request.avatar_color.as_deref() {
        validation::avatar_color(color)?;
    }

    if let Some(Some(avatar_id)) = request.avatar_media_id.as_ref() {
        let avatar = media::get_avatar(&state.repo, user_id, avatar_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, user_id = %user_id, "avatar lookup failed");
                ApiError::internal("failed to load avatar")
            })?
            .ok_or_else(|| {
                ApiError::invalid_field(
                    "avatarMediaId",
                    format!("avatarMediaId {avatar_id} is not owned by you"),
                )
            })?;
        if avatar.status != MediaStatus::Ready {
            return Err(ApiError::invalid_field(
                "avatarMediaId",
                format!("avatarMediaId {avatar_id} is still processing"),
            ));
        }
    }

    let previous_avatar_id = users::update_profile(
        &state.repo,
        user_id,
        display_name.as_deref(),
        request.avatar_color.as_deref(),
        request.avatar_media_id.as_ref().map(|inner| inner.as_ref()),
    )
    .await
    .map_err(|e| match e {
        RepoError::ConditionalCheckFailed => ApiError::not_found("user profile not found"),
        e => {
            tracing::error!(error = ?e, user_id = %user_id, "profile update failed");
            ApiError::internal("failed to update profile")
        }
    })?;

    // Replacing or clearing the photo deletes the one it displaced
    // (`docs/design/api.md` §2.3, §9.7). `previous_avatar_id` comes from the
    // write itself, so two concurrent PATCHes each delete the avatar they
    // really displaced.
    if let (Some(new_id), Some(old_id)) = (&request.avatar_media_id, &previous_avatar_id) {
        if new_id.as_ref() != Some(old_id) {
            delete_replaced_avatar(state, user_id, old_id).await;
        }
    }

    get_me(state, user_id).await
}

/// Best effort (`docs/design/api.md` §9.7): the profile no longer points at the
/// old avatar, so a failure here only leaves its objects behind.
async fn delete_replaced_avatar(state: &AppState, user_id: &UserId, avatar_id: &AvatarId) {
    let avatar = match media::get_avatar(&state.repo, user_id, avatar_id).await {
        Ok(Some(avatar)) => avatar,
        Ok(None) => return,
        Err(e) => {
            tracing::error!(error = ?e, user_id = %user_id, avatar_id = %avatar_id, "replaced avatar lookup failed; not deleted");
            return;
        }
    };
    if let Err(e) =
        media_delete::delete_avatar_media(&state.repo, &state.avatar_deleter, &avatar).await
    {
        tracing::error!(error = ?e, user_id = %user_id, avatar_id = %avatar_id, "replaced avatar delete failed");
    }
}

async fn load_user(state: &AppState, user_id: &UserId) -> Result<User, ApiError> {
    users::get_user(&state.repo, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, "user lookup failed");
            ApiError::internal("failed to load profile")
        })?
        .ok_or_else(|| ApiError::not_found("user profile not found"))
}

/// The denormalized join `GET /config` returns: one query
/// for the caller's memberships, then one batched read for the group rows.
async fn membership_summaries(
    state: &AppState,
    user_id: &UserId,
) -> Result<Vec<MembershipSummary>, ApiError> {
    let memberships: Vec<GroupMembership> = groups::list_memberships_for_user(&state.repo, user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, "membership listing failed");
            ApiError::internal("failed to list memberships")
        })?;

    let group_ids: Vec<_> = memberships.iter().map(|m| m.group_id.clone()).collect();
    let mut groups_by_id = groups::get_groups_batch(&state.repo, &group_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, "group batch lookup failed");
            ApiError::internal("failed to load groups")
        })?;

    // A membership pointing at a deleted group is skipped rather than fatal —
    // the caller's other groups should still render.
    Ok(memberships
        .into_iter()
        .filter_map(|membership| {
            let group = groups_by_id.remove(&membership.group_id)?;
            Some(MembershipSummary {
                group_id: membership.group_id,
                role: membership.role,
                group_name: group.name,
                timezone: group.timezone,
                gradient: group.gradient,
            })
        })
        .collect())
}
