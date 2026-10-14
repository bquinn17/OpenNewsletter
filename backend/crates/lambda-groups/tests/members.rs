//! Handler-level tests for the last-admin guard on member removal and demotion
//! (`docs/design/api.md` §4, `LAST_ADMIN`).
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`, and tests run concurrently.

mod common;

use common::{seed, state};
use domain::api::PatchMemberRequest;
use domain::{ApiErrorCode, GroupId, Role, UserId};
use groups::group_routes;
use persistence::{groups as group_repo, test_factories, Repo};

async fn admin_count(repo: &Repo, group_id: &str) -> usize {
    group_repo::list_members_for_group(repo, &GroupId::new(group_id))
        .await
        .unwrap()
        .iter()
        .filter(|m| m.role == Role::Admin)
        .count()
}

#[tokio::test]
async fn two_admins_demoting_each_other_concurrently_leave_one_admin() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(
        &repo,
        "gmutual",
        &[("a1", Role::Admin), ("a2", Role::Admin)],
    )
    .await;
    let state = state(repo);
    let g = GroupId::new("gmutual");
    let (a1, a2) = (UserId::new("a1"), UserId::new("a2"));
    let demote = || PatchMemberRequest { role: Role::Member };

    let (r1, r2) = tokio::join!(
        group_routes::patch_member(&state, &a1, &g, &a2, demote()),
        group_routes::patch_member(&state, &a2, &g, &a1, demote()),
    );

    // Exactly one wins. The loser either lost the transaction race (LAST_ADMIN)
    // or, if the calls happened to run one after the other, was no longer an
    // admin by the time it was authorized (FORBIDDEN).
    assert_eq!(r1.is_ok() as u8 + r2.is_ok() as u8, 1, "{r1:?} / {r2:?}");
    let loser = r1.err().or(r2.err()).unwrap();
    assert!(
        matches!(
            loser.code,
            ApiErrorCode::LastAdmin | ApiErrorCode::Forbidden
        ),
        "{loser:?}"
    );
    assert_eq!(admin_count(&state.repo, "gmutual").await, 1);
}

#[tokio::test]
async fn the_only_admin_cannot_leave() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "gsole", &[("a1", Role::Admin), ("m1", Role::Member)]).await;
    let state = state(repo);
    let a1 = UserId::new("a1");

    let err = group_routes::remove_member(&state, &a1, &GroupId::new("gsole"), &a1)
        .await
        .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::LastAdmin);
    assert_eq!(admin_count(&state.repo, "gsole").await, 1);
}

#[tokio::test]
async fn an_admin_can_leave_when_another_admin_remains() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "gpair", &[("a1", Role::Admin), ("a2", Role::Admin)]).await;
    let state = state(repo);
    let a1 = UserId::new("a1");

    group_routes::remove_member(&state, &a1, &GroupId::new("gpair"), &a1)
        .await
        .unwrap();
    assert_eq!(admin_count(&state.repo, "gpair").await, 1);
}

#[tokio::test]
async fn an_admin_can_demote_a_co_admin() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "gco", &[("a1", Role::Admin), ("a2", Role::Admin)]).await;
    let state = state(repo);

    let member = group_routes::patch_member(
        &state,
        &UserId::new("a1"),
        &GroupId::new("gco"),
        &UserId::new("a2"),
        PatchMemberRequest { role: Role::Member },
    )
    .await
    .unwrap();
    assert_eq!(member.role, Role::Member);
}
