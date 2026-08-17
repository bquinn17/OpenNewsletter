//! `GET /config`, `GET /me`, `PATCH /me` (`plans/03-api-contract.md` §2).

use crate::dto::{ConfigResponse, MembershipSummary, PatchMeRequest, UserResponse};
use crate::state::AppState;
use crate::validation;
use domain::{ApiError, GroupMembership, MediaStatus, User, UserId};
use persistence::{auth, groups, media, users};
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
    let avatar_url = user
        .avatar_media_id
        .as_ref()
        .and_then(|id| state.avatar_url(id));
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
                ApiError::validation(format!("avatarMediaId {avatar_id} is not owned by you"))
            })?;
        if avatar.status != MediaStatus::Ready {
            return Err(ApiError::validation(format!(
                "avatarMediaId {avatar_id} is still processing"
            )));
        }
    }

    users::update_profile(
        &state.repo,
        user_id,
        display_name.as_deref(),
        request.avatar_color.as_deref(),
        request.avatar_media_id.as_ref().map(|inner| inner.as_ref()),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = ?e, user_id = %user_id, "profile update failed");
        ApiError::internal("failed to update profile")
    })?;

    get_me(state, user_id).await
}

pub async fn list_memberships(
    state: &AppState,
    user_id: &UserId,
) -> Result<Vec<MembershipSummary>, ApiError> {
    membership_summaries(state, user_id).await
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

/// The denormalized join `GET /config` and `GET /groups` both return. The group
/// count per user is small enough that a fetch per membership stays cheap.
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

    let mut summaries = Vec::with_capacity(memberships.len());
    for membership in memberships {
        let group = groups::get_group(&state.repo, &membership.group_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %membership.group_id, "group lookup failed");
                ApiError::internal("failed to load group")
            })?;
        // A membership pointing at a deleted group is skipped rather than fatal —
        // the caller's other groups should still render.
        if let Some(group) = group {
            summaries.push(MembershipSummary {
                group_id: membership.group_id,
                role: membership.role,
                group_name: group.name,
            });
        }
    }
    Ok(summaries)
}
