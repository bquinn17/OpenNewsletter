//! Email-bound invites (`docs/design/auth.md` §4.2 step 2,
//! `docs/design/api.md` §3.1, §3.4).
//!
//! Each test uses its own group id: `persistence::auth` keeps process-wide
//! caches, and tests run concurrently.

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{Duration, Utc};
use domain::api::{CreateInviteRequest, RedeemRequest};
use domain::{ApiErrorCode, CognitoSub, GroupId, Invite, InviteCode, InviteStatus, Role, UserId};
use invites::handlers;
use invites::state::AppState;
use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
use persistence::{groups, invites as invite_repo, test_factories, Repo};
use pretty_assertions::assert_eq;
use shared::http::AuthClaims;

/// A group with one admin, `a1`, and a pending invite bound to `bound_email`.
async fn seed(repo: &Repo, group_id: &str, code: &str, bound_email: Option<&str>) {
    groups::put_group(repo, &test_factories::group(group_id, "a1"))
        .await
        .unwrap();
    put_membership(repo, "a1", group_id).await;
    let now = Utc::now();
    let invite = Invite {
        code: InviteCode::new(code),
        group_id: GroupId::new(group_id),
        created_by: UserId::new("a1"),
        created_at: now,
        expires_at: now + Duration::days(7),
        status: InviteStatus::Pending,
        consumed_by: None,
        consumed_at: None,
        role_on_redeem: Role::Admin,
        bound_email: bound_email.map(str::to_owned),
    };
    invite_repo::put_invite(repo, &invite, (now + Duration::days(37)).timestamp())
        .await
        .unwrap();
}

async fn put_membership(repo: &Repo, user_id: &str, group_id: &str) {
    let m = test_factories::membership(user_id, group_id, Role::Admin);
    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&m).unwrap();
    item.insert(attr::PK.into(), AttributeValue::S(user_pk(&m.user_id)));
    item.insert(
        attr::SK.into(),
        AttributeValue::S(membership_sk(&m.group_id)),
    );
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(membership_gsi1pk(&m.group_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(membership_gsi1sk(&m.user_id)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("GroupMembership".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .unwrap();
}

fn claims(sub: &str, email: Option<&str>, email_verified: bool) -> AuthClaims {
    AuthClaims {
        sub: CognitoSub::new(sub),
        email: email.map(str::to_owned),
        email_verified,
        name: None,
    }
}

fn redeem_request(code: &str) -> RedeemRequest {
    RedeemRequest { code: code.into() }
}

async fn invite_status(repo: &Repo, code: &str) -> InviteStatus {
    invite_repo::get_invite(repo, &InviteCode::new(code))
        .await
        .unwrap()
        .unwrap()
        .status
}

const CODE: &str = "ABCDEFGHJKMNPQRS";

#[tokio::test]
async fn a_bound_invite_redeems_for_the_verified_email_in_any_case() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-bound-ok", CODE, Some("quinn@example.com")).await;
    let state = AppState { repo };

    let joined = handlers::redeem(
        &state,
        &claims("sub-ok", Some("Quinn@Example.COM"), true),
        redeem_request(CODE),
    )
    .await
    .expect("a matching, verified email redeems");

    assert_eq!(joined.group_id, GroupId::new("g-bound-ok"));
    assert_eq!(joined.role, Role::Admin);
    assert_eq!(
        invite_status(&state.repo, CODE).await,
        InviteStatus::Consumed
    );
}

#[tokio::test]
async fn a_bound_invite_refuses_a_different_unverified_or_missing_email() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-bound-no", CODE, Some("quinn@example.com")).await;
    let state = AppState { repo };

    for (case, caller) in [
        (
            "different",
            claims("sub-1", Some("other@example.com"), true),
        ),
        (
            "unverified",
            claims("sub-2", Some("quinn@example.com"), false),
        ),
        ("missing", claims("sub-3", None, true)),
    ] {
        let err = handlers::redeem(&state, &caller, redeem_request(CODE))
            .await
            .expect_err(case);
        assert_eq!(err.code, ApiErrorCode::InviteEmailMismatch, "{case}");
        assert_eq!(err.http_status(), 403, "{case}");
    }
    assert_eq!(
        invite_status(&state.repo, CODE).await,
        InviteStatus::Pending
    );
}

#[tokio::test]
async fn an_unbound_invite_ignores_the_email_claims() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-unbound", CODE, None).await;
    let state = AppState { repo };

    handlers::redeem(
        &state,
        &claims("sub-u", Some("anyone@example.com"), false),
        redeem_request(CODE),
    )
    .await
    .expect("unbound invites work as before");

    assert_eq!(
        invite_status(&state.repo, CODE).await,
        InviteStatus::Consumed
    );
}

#[tokio::test]
async fn create_invite_stores_the_bound_email_trimmed_and_lowercased() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-create", CODE, None).await;
    let state = AppState { repo };

    let created = handlers::create_invite(
        &state,
        &UserId::new("a1"),
        CreateInviteRequest {
            group_id: GroupId::new("g-create"),
            ttl_days: None,
            role_on_redeem: Role::Admin,
            bound_email: Some("  Quinn@Example.com ".into()),
        },
    )
    .await
    .unwrap();

    assert_eq!(created.bound_email.as_deref(), Some("quinn@example.com"));
    let listed = handlers::list_invites(&state, &UserId::new("a1"), &GroupId::new("g-create"))
        .await
        .unwrap();
    let mine = listed
        .items
        .iter()
        .find(|i| i.code == created.code)
        .expect("listed");
    assert_eq!(mine.bound_email.as_deref(), Some("quinn@example.com"));
}

#[tokio::test]
async fn create_invite_rejects_a_bound_email_that_is_not_an_address() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-create-bad", CODE, None).await;
    let state = AppState { repo };

    for bad in [
        "not-an-email".to_owned(),
        format!("{}@x.co", "a".repeat(250)),
    ] {
        let err = handlers::create_invite(
            &state,
            &UserId::new("a1"),
            CreateInviteRequest {
                group_id: GroupId::new("g-create-bad"),
                ttl_days: None,
                role_on_redeem: Role::Member,
                bound_email: Some(bad.clone()),
            },
        )
        .await
        .expect_err(&bad);
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        assert_eq!(err.field_errors[0].field, "boundEmail");
    }
}
