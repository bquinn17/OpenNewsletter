mod common;

use domain::{GroupId, InviteCode, InviteStatus, Role};
use persistence::invites;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_puts_and_gets_invite() {
    let (_c, repo) = common::make_repo().await;
    let inv = common::invite("CODE1234", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();
    let got = invites::get_invite(&repo, &inv.code)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, inv);
}

#[tokio::test]
async fn it_returns_none_for_missing_invite() {
    let (_c, repo) = common::make_repo().await;
    let result = invites::get_invite(&repo, &InviteCode::new("NO_SUCH"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_lists_invites_for_group_via_gsi1() {
    let (_c, repo) = common::make_repo().await;
    let inv1 = common::invite("CODE0001", "g1", "u1");
    let inv2 = common::invite("CODE0002", "g1", "u1");
    let other = common::invite("CODE0003", "g2", "u1");
    invites::put_invite(&repo, &inv1, 9999999999).await.unwrap();
    invites::put_invite(&repo, &inv2, 9999999999).await.unwrap();
    invites::put_invite(&repo, &other, 9999999999)
        .await
        .unwrap();

    let mut got = invites::list_invites_for_group(&repo, &GroupId::new("g1"))
        .await
        .unwrap();
    got.sort_by(|a, b| a.code.as_str().cmp(b.code.as_str()));
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], inv1);
    assert_eq!(got[1], inv2);
}

#[tokio::test]
async fn it_revokes_an_invite() {
    let (_c, repo) = common::make_repo().await;
    let inv = common::invite("REVOKE01", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();

    invites::revoke(&repo, &inv.code).await.unwrap();

    let got = invites::get_invite(&repo, &inv.code)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, InviteStatus::Revoked);
}

#[tokio::test]
async fn join_via_invite_tx_creates_membership_and_bumps_member_count() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 1;
    persistence::groups::put_group(&repo, &g).await.unwrap();

    let inv = common::invite("JOIN0001", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();

    let membership = common::membership("u2", "g1", Role::Member);
    invites::join_via_invite_tx(&repo, &inv, &membership, None, "2026-06-10T00:00:00Z", 20)
        .await
        .unwrap();

    // Invite must now be consumed.
    let updated_inv = invites::get_invite(&repo, &inv.code)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_inv.status, InviteStatus::Consumed);

    // Membership must exist.
    let got_m =
        persistence::groups::get_membership(&repo, &membership.user_id, &membership.group_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(got_m.user_id, membership.user_id);

    // Group memberCount must be bumped.
    let updated_g = persistence::groups::get_group(&repo, &g.group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_g.member_count, 2);
}

#[tokio::test]
async fn join_via_invite_tx_fails_when_invite_already_consumed() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 1;
    persistence::groups::put_group(&repo, &g).await.unwrap();

    let inv = common::invite("USED0001", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();

    // First join succeeds.
    let m1 = common::membership("u2", "g1", Role::Member);
    invites::join_via_invite_tx(&repo, &inv, &m1, None, "2026-06-10T00:00:00Z", 20)
        .await
        .unwrap();

    // Second join with same invite code must fail.
    let m2 = common::membership("u3", "g1", Role::Member);
    let result =
        invites::join_via_invite_tx(&repo, &inv, &m2, None, "2026-06-10T01:00:00Z", 20).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn revoking_a_consumed_invite_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 1;
    persistence::groups::put_group(&repo, &g).await.unwrap();

    let inv = common::invite("REVOKE02", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();
    let membership = common::membership("u2", "g1", Role::Member);
    invites::join_via_invite_tx(&repo, &inv, &membership, None, "2026-06-10T00:00:00Z", 20)
        .await
        .unwrap();

    let result = invites::revoke(&repo, &inv.code).await;

    assert!(matches!(
        result,
        Err(persistence::RepoError::ConditionalCheckFailed)
    ));
}

#[tokio::test]
async fn first_redemption_creates_the_user_and_sub_lookup() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 1;
    persistence::groups::put_group(&repo, &g).await.unwrap();

    let inv = common::invite("NEWUSER1", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();

    let new_user = common::user("u2");
    let membership = common::membership("u2", "g1", Role::Member);
    invites::join_via_invite_tx(
        &repo,
        &inv,
        &membership,
        Some(&new_user),
        "2026-06-10T00:00:00Z",
        20,
    )
    .await
    .unwrap();

    let stored = persistence::users::get_user(&repo, &new_user.user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored, new_user);

    let resolved = persistence::users::resolve_cognito_sub(&repo, &new_user.cognito_sub)
        .await
        .unwrap();
    assert_eq!(resolved, Some(new_user.user_id));
}

#[tokio::test]
async fn redemption_is_refused_once_the_group_is_at_its_member_cap() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 20;
    persistence::groups::put_group(&repo, &g).await.unwrap();

    let inv = common::invite("ATCAP001", "g1", "u1");
    invites::put_invite(&repo, &inv, 9999999999).await.unwrap();

    let membership = common::membership("u2", "g1", Role::Member);
    let result =
        invites::join_via_invite_tx(&repo, &inv, &membership, None, "2026-06-10T00:00:00Z", 20)
            .await;

    assert!(matches!(
        result,
        Err(persistence::RepoError::ConditionalCheckFailed)
    ));
}
