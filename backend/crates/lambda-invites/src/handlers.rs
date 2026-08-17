//! Invite creation, listing, revocation, and redemption
//! (`plans/03-api-contract.md` §3, `plans/05-auth-flow.md` §4).

use crate::code;
use crate::dto::{
    CreateInviteRequest, CreateInviteResponse, InviteListResponse, InviteSummary, RedeemRequest,
    RedeemResponse,
};
use crate::state::AppState;
use chrono::{Duration, Utc};
use domain::{
    ApiError, ApiErrorCode, Group, GroupId, GroupMembership, Invite, InviteCode, InviteStatus,
    User, UserId,
};
use persistence::{auth, groups, invites, RepoError};
use shared::config::{
    derive_avatar_color, INVITE_AUDIT_RETENTION_DAYS, INVITE_DEFAULT_TTL_DAYS, INVITE_MAX_TTL_DAYS,
};
use shared::http::AuthClaims;

pub async fn create_invite(
    state: &AppState,
    caller: &UserId,
    request: CreateInviteRequest,
) -> Result<CreateInviteResponse, ApiError> {
    auth::require_membership(&state.repo, caller, &request.group_id, true).await?;

    let ttl_days = request.ttl_days.unwrap_or(INVITE_DEFAULT_TTL_DAYS);
    if !(1..=INVITE_MAX_TTL_DAYS).contains(&ttl_days) {
        return Err(ApiError::validation(format!(
            "ttlDays must be between 1 and {INVITE_MAX_TTL_DAYS}"
        )));
    }

    let now = Utc::now();
    let expires_at = now + Duration::days(i64::from(ttl_days));
    let invite = Invite {
        code: code::generate(),
        group_id: request.group_id,
        created_by: caller.clone(),
        created_at: now,
        expires_at,
        status: InviteStatus::Pending,
        consumed_by: None,
        consumed_at: None,
        role_on_redeem: request.role_on_redeem,
    };

    // The row outlives the invite itself so admins can audit who joined with what.
    let ttl_epoch_seconds = (expires_at + Duration::days(INVITE_AUDIT_RETENTION_DAYS)).timestamp();

    invites::put_invite(&state.repo, &invite, ttl_epoch_seconds)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %invite.group_id, "invite write failed");
            ApiError::internal("failed to create invite")
        })?;

    Ok(CreateInviteResponse {
        code: code::format_for_display(&invite.code),
        group_id: invite.group_id,
        expires_at: invite.expires_at,
        role_on_redeem: invite.role_on_redeem,
    })
}

pub async fn list_invites(
    state: &AppState,
    caller: &UserId,
    group_id: &GroupId,
) -> Result<InviteListResponse, ApiError> {
    auth::require_membership(&state.repo, caller, group_id, true).await?;

    let items = invites::list_invites_for_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "invite listing failed");
            ApiError::internal("failed to list invites")
        })?
        .into_iter()
        .map(|invite| {
            let display = code::format_for_display(&invite.code);
            InviteSummary::new(invite, display)
        })
        .collect();

    Ok(InviteListResponse {
        items,
        next_cursor: None,
    })
}

pub async fn revoke_invite(
    state: &AppState,
    caller: &UserId,
    raw_code: &str,
) -> Result<(), ApiError> {
    let invite = load_invite(state, &code::normalize(raw_code)).await?;
    auth::require_membership(&state.repo, caller, &invite.group_id, true).await?;

    match invite.status {
        // Revocation is idempotent, but a consumed invite is a membership that
        // already exists — undoing it here would be a lie.
        InviteStatus::Revoked => return Ok(()),
        InviteStatus::Consumed => {
            return Err(ApiError::new(
                ApiErrorCode::InviteConsumed,
                "invite has already been redeemed",
            ))
        }
        InviteStatus::Pending => {}
    }

    invites::revoke(&state.repo, &invite.code)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %invite.group_id, "invite revoke failed");
            ApiError::internal("failed to revoke invite")
        })?;
    Ok(())
}

/// `POST /invites/redeem` — the single point where invites are consumed, covering
/// both first signup and an existing user joining another group
/// (`plans/05-auth-flow.md` §4.2).
pub async fn redeem(
    state: &AppState,
    claims: &AuthClaims,
    request: RedeemRequest,
) -> Result<RedeemResponse, ApiError> {
    let invite = load_invite(state, &code::normalize(&request.code)).await?;
    let group = load_group(state, &invite.group_id).await?;

    let existing_user_id = auth::resolve_user_id(&state.repo, &claims.sub)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "cognito sub lookup failed");
            ApiError::internal("failed to resolve caller")
        })?;

    // Idempotency: a retried submit of an invite this caller already consumed
    // should land on the same success it did the first time.
    if let Some(user_id) = &existing_user_id {
        if let Some(membership) = groups::get_membership(&state.repo, user_id, &invite.group_id)
            .await
            .map_err(|e| {
                tracing::error!(error = ?e, group_id = %invite.group_id, "membership lookup failed");
                ApiError::internal("failed to load membership")
            })?
        {
            return Ok(RedeemResponse {
                group_id: invite.group_id,
                group_name: group.name,
                role: membership.role,
            });
        }
    }

    check_redeemable(&invite)?;
    if group.member_count >= group.member_soft_cap {
        return Err(ApiError::new(
            ApiErrorCode::MemberCapReached,
            "group has reached its member cap",
        ));
    }

    let now = Utc::now();
    let (user_id, new_user) = match &existing_user_id {
        Some(user_id) => (user_id.clone(), None),
        None => {
            let user_id = UserId::generate();
            let email = claims.require_email()?.to_owned();
            let user = User {
                display_name: claims.name.clone().unwrap_or_else(|| email.clone()),
                avatar_color: derive_avatar_color(user_id.as_str()).to_owned(),
                user_id: user_id.clone(),
                cognito_sub: claims.sub.clone(),
                email,
                avatar_media_id: None,
                created_at: now,
                last_login_at: now,
            };
            (user_id, Some(user))
        }
    };

    let membership = GroupMembership {
        user_id: user_id.clone(),
        group_id: invite.group_id.clone(),
        role: invite.role_on_redeem,
        joined_at: now,
        editions_answered: 0,
    };

    invites::join_via_invite_tx(
        &state.repo,
        &invite,
        &membership,
        new_user.as_ref(),
        &now.to_rfc3339(),
        group.member_soft_cap,
    )
    .await
    .map_err(|e| match e {
        // The transaction's own guards lost a race: another caller consumed the
        // invite, or the last seat was taken, between our reads and the write.
        RepoError::ConditionalCheckFailed => ApiError::new(
            ApiErrorCode::InviteConsumed,
            "invite was consumed by another request",
        ),
        other => {
            tracing::error!(error = ?other, group_id = %invite.group_id, "invite redemption failed");
            ApiError::internal("failed to redeem invite")
        }
    })?;

    if new_user.is_some() {
        auth::cache_user_id(&claims.sub, &user_id);
    }
    tracing::info!(
        group_id = %invite.group_id,
        user_id = %user_id,
        is_new_user = new_user.is_some(),
        "invite redeemed"
    );

    Ok(RedeemResponse {
        group_id: invite.group_id,
        group_name: group.name,
        role: invite.role_on_redeem,
    })
}

fn check_redeemable(invite: &Invite) -> Result<(), ApiError> {
    match invite.status {
        InviteStatus::Consumed => {
            // Covered by the membership short-circuit above in the normal case;
            // reaching here means someone else already used this code.
            Err(ApiError::new(
                ApiErrorCode::InviteConsumed,
                "invite has already been redeemed",
            ))
        }
        InviteStatus::Revoked => Err(ApiError::new(
            ApiErrorCode::InviteInvalid,
            "invite has been revoked",
        )),
        InviteStatus::Pending if invite.expires_at <= Utc::now() => Err(ApiError::new(
            ApiErrorCode::InviteExpired,
            "invite has expired",
        )),
        InviteStatus::Pending => Ok(()),
    }
}

async fn load_invite(state: &AppState, code: &InviteCode) -> Result<Invite, ApiError> {
    invites::get_invite(&state.repo, code)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "invite lookup failed");
            ApiError::internal("failed to load invite")
        })?
        .ok_or_else(|| ApiError::new(ApiErrorCode::InviteInvalid, "invite code is not valid"))
}

async fn load_group(state: &AppState, group_id: &GroupId) -> Result<Group, ApiError> {
    groups::get_group(&state.repo, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, group_id = %group_id, "group lookup failed");
            ApiError::internal("failed to load group")
        })?
        .ok_or_else(|| {
            ApiError::new(
                ApiErrorCode::InviteInvalid,
                "invite points at a group that no longer exists",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite_with(status: InviteStatus, expires_at: chrono::DateTime<Utc>) -> Invite {
        Invite {
            code: InviteCode::new("ABCDEFGHJKMNPQRS"),
            group_id: GroupId::new("01HG2"),
            created_by: UserId::new("01HX1"),
            created_at: Utc::now() - Duration::days(1),
            expires_at,
            status,
            consumed_by: None,
            consumed_at: None,
            role_on_redeem: domain::Role::Member,
        }
    }

    #[test]
    fn a_pending_unexpired_invite_is_redeemable() {
        let invite = invite_with(InviteStatus::Pending, Utc::now() + Duration::days(1));
        assert!(check_redeemable(&invite).is_ok());
    }

    #[test]
    fn an_expired_invite_is_refused() {
        let invite = invite_with(InviteStatus::Pending, Utc::now() - Duration::minutes(1));
        let err = check_redeemable(&invite).expect_err("expired");
        assert_eq!(err.code, ApiErrorCode::InviteExpired);
    }

    #[test]
    fn a_consumed_invite_is_refused() {
        let invite = invite_with(InviteStatus::Consumed, Utc::now() + Duration::days(1));
        let err = check_redeemable(&invite).expect_err("consumed");
        assert_eq!(err.code, ApiErrorCode::InviteConsumed);
    }

    #[test]
    fn a_revoked_invite_is_refused() {
        let invite = invite_with(InviteStatus::Revoked, Utc::now() + Duration::days(1));
        let err = check_redeemable(&invite).expect_err("revoked");
        assert_eq!(err.code, ApiErrorCode::InviteInvalid);
    }
}
