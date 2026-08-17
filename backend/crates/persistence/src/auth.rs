//! Caller resolution and tenant authorization (`plans/05-auth-flow.md` §6, §11).
//!
//! Every group-scoped handler calls [`require_membership`] before touching any
//! group data. Both lookups are memoized for the life of a warm Lambda container,
//! bounded by [`MEMBERSHIP_CACHE_TTL_SECONDS`].

use crate::error::RepoError;
use crate::repo::Repo;
use crate::{groups, users};
use dashmap::DashMap;
use domain::{ApiError, CognitoSub, GroupId, GroupMembership, Role, UserId};
use once_cell::sync::Lazy;
use shared::config::MEMBERSHIP_CACHE_TTL_SECONDS;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(MEMBERSHIP_CACHE_TTL_SECONDS);

#[derive(Debug, Clone)]
struct Cached<T> {
    value: T,
    cached_at: Instant,
}

impl<T> Cached<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            cached_at: Instant::now(),
        }
    }

    fn fresh(&self) -> bool {
        self.cached_at.elapsed() < CACHE_TTL
    }
}

static SUB_LOOKUP_CACHE: Lazy<DashMap<CognitoSub, Cached<UserId>>> = Lazy::new(DashMap::new);
static MEMBERSHIP_CACHE: Lazy<DashMap<(UserId, GroupId), Cached<GroupMembership>>> =
    Lazy::new(DashMap::new);

/// Resolve a Cognito `sub` claim to our internal `UserId`.
///
/// `Ok(None)` means the JWT is valid but the account has never redeemed an invite.
/// `GET /config` renders that as an empty membership list; every other handler
/// treats it as `UNAUTHENTICATED`.
pub async fn resolve_user_id(repo: &Repo, sub: &CognitoSub) -> Result<Option<UserId>, RepoError> {
    if let Some(cached) = SUB_LOOKUP_CACHE.get(sub) {
        if cached.fresh() {
            return Ok(Some(cached.value.clone()));
        }
    }

    let Some(user_id) = users::resolve_cognito_sub(repo, sub).await? else {
        return Ok(None);
    };
    SUB_LOOKUP_CACHE.insert(sub.clone(), Cached::new(user_id.clone()));
    Ok(Some(user_id))
}

/// Resolve the caller, refusing anonymous or un-onboarded accounts.
pub async fn require_user_id(repo: &Repo, sub: &CognitoSub) -> Result<UserId, ApiError> {
    resolve_user_id(repo, sub)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "cognito sub lookup failed");
            ApiError::internal("failed to resolve caller")
        })?
        .ok_or_else(|| ApiError::unauthenticated("caller has not redeemed an invite"))
}

/// The tenant-isolation gate. Returns the caller's membership, or `FORBIDDEN`
/// when they are not in the group (or not an admin, when `require_admin`).
pub async fn require_membership(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
    require_admin: bool,
) -> Result<GroupMembership, ApiError> {
    let key = (user_id.clone(), group_id.clone());
    if let Some(cached) = MEMBERSHIP_CACHE.get(&key) {
        if cached.fresh() {
            return enforce(cached.value.clone(), require_admin);
        }
    }

    let membership = groups::get_membership(repo, user_id, group_id)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, user_id = %user_id, group_id = %group_id, "membership lookup failed");
            ApiError::internal("failed to resolve membership")
        })?
        .ok_or_else(|| ApiError::forbidden(format!("not a member of group {group_id}")))?;

    MEMBERSHIP_CACHE.insert(key, Cached::new(membership.clone()));
    enforce(membership, require_admin)
}

/// Drop cached state for a caller whose membership or identity just changed, so the
/// next request in the same warm container does not authorize against a stale row.
pub fn invalidate_membership(user_id: &UserId, group_id: &GroupId) {
    MEMBERSHIP_CACHE.remove(&(user_id.clone(), group_id.clone()));
}

pub fn cache_user_id(sub: &CognitoSub, user_id: &UserId) {
    SUB_LOOKUP_CACHE.insert(sub.clone(), Cached::new(user_id.clone()));
}

fn enforce(membership: GroupMembership, require_admin: bool) -> Result<GroupMembership, ApiError> {
    if require_admin && membership.role != Role::Admin {
        return Err(ApiError::forbidden(format!(
            "admin role required in group {}",
            membership.group_id
        )));
    }
    Ok(membership)
}
