//! `persistence::auth::require_membership` (`docs/design/auth.md` §11).
//!
//! Each test uses its own group id: the membership cache is process-wide.

mod common;

use domain::{ApiErrorCode, Role};
use persistence::{auth, groups};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn admin_checks_skip_the_cache_but_member_checks_use_it() {
    let (_c, repo) = common::make_repo().await;
    let m = common::membership("u1", "gauth-demote", Role::Admin);
    common::put_membership(&repo, &m).await;

    auth::require_membership(&repo, &m.user_id, &m.group_id, true)
        .await
        .expect("u1 starts as an admin");

    // A demotion made by another Lambda: nothing evicts this process's cache.
    groups::update_membership_role(&repo, &m.user_id, &m.group_id, Role::Member, None)
        .await
        .unwrap();

    let err = auth::require_membership(&repo, &m.user_id, &m.group_id, true)
        .await
        .expect_err("the admin check reads the demotion");
    assert_eq!(err.code, ApiErrorCode::Forbidden);

    let cached = auth::require_membership(&repo, &m.user_id, &m.group_id, false)
        .await
        .expect("still a member");
    assert_eq!(
        cached.role,
        Role::Member,
        "the uncached read refreshed the cache"
    );
}

#[tokio::test]
async fn a_removed_admin_is_refused_admin_access_at_once() {
    let (_c, repo) = common::make_repo().await;
    groups::put_group(&repo, &common::group("gauth-remove", "u1"))
        .await
        .unwrap();
    let m = common::membership("u1", "gauth-remove", Role::Admin);
    common::put_membership(&repo, &m).await;

    auth::require_membership(&repo, &m.user_id, &m.group_id, true)
        .await
        .expect("u1 starts as an admin");

    groups::leave_group_tx(&repo, &m.user_id, &m.group_id, None)
        .await
        .unwrap();

    let err = auth::require_membership(&repo, &m.user_id, &m.group_id, true)
        .await
        .expect_err("the admin check sees the removal");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}
