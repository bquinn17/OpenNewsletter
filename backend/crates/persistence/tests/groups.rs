mod common;

use domain::{GroupId, Role, UserId};
use persistence::groups;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_puts_and_gets_group() {
    let (_c, repo) = common::make_repo().await;
    let g = common::group("g1", "creator");
    groups::put_group(&repo, &g).await.unwrap();
    let got = groups::get_group(&repo, &g.group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, g);
}

#[tokio::test]
async fn it_returns_none_for_missing_group() {
    let (_c, repo) = common::make_repo().await;
    let result = groups::get_group(&repo, &GroupId::new("no-such-group"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_lists_memberships_for_user() {
    let (_c, repo) = common::make_repo().await;
    let g1 = common::group("g1", "u1");
    let g2 = common::group("g2", "u1");
    groups::put_group(&repo, &g1).await.unwrap();
    groups::put_group(&repo, &g2).await.unwrap();

    let m1 = common::membership("u1", "g1", Role::Admin);
    let m2 = common::membership("u1", "g2", Role::Member);
    // Write memberships using the join-invite transaction helper indirectly —
    // but since we have no invite, use get_membership+put_membership pattern.
    // The persistence crate has no bare put_membership, so we call groups directly.
    // We write membership items via the DDB client to keep this test self-contained.
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
    use serde_dynamo::to_item;

    for m in [&m1, &m2] {
        let mut item: std::collections::HashMap<String, AttributeValue> = to_item(m).unwrap();
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

    let user_id = UserId::new("u1");
    let mut memberships = groups::list_memberships_for_user(&repo, &user_id)
        .await
        .unwrap();
    memberships.sort_by(|a, b| a.group_id.as_str().cmp(b.group_id.as_str()));
    assert_eq!(memberships.len(), 2);
    assert_eq!(memberships[0], m1);
    assert_eq!(memberships[1], m2);
}

#[tokio::test]
async fn it_lists_members_for_group_via_gsi1() {
    let (_c, repo) = common::make_repo().await;
    let g = common::group("g1", "u1");
    groups::put_group(&repo, &g).await.unwrap();

    let m1 = common::membership("u1", "g1", Role::Admin);
    let m2 = common::membership("u2", "g1", Role::Member);

    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
    use serde_dynamo::to_item;

    for m in [&m1, &m2] {
        let mut item: std::collections::HashMap<String, AttributeValue> = to_item(m).unwrap();
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

    let mut members = groups::list_members_for_group(&repo, &GroupId::new("g1"))
        .await
        .unwrap();
    members.sort_by(|a, b| a.user_id.as_str().cmp(b.user_id.as_str()));
    assert_eq!(members.len(), 2);
    assert_eq!(members[0], m1);
    assert_eq!(members[1], m2);
}

#[tokio::test]
async fn leave_group_tx_deletes_membership_and_decrements_count() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g1", "u1");
    g.member_count = 2;
    groups::put_group(&repo, &g).await.unwrap();

    let m = common::membership("u2", "g1", Role::Member);
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
    use serde_dynamo::to_item;

    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(&m).unwrap();
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

    groups::leave_group_tx(&repo, &m.user_id, &m.group_id)
        .await
        .unwrap();

    let gone = groups::get_membership(&repo, &m.user_id, &m.group_id)
        .await
        .unwrap();
    assert_eq!(gone, None);

    let updated_group = groups::get_group(&repo, &GroupId::new("g1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_group.member_count, 1);
}

#[tokio::test]
async fn leave_group_tx_fails_when_membership_absent() {
    let (_c, repo) = common::make_repo().await;
    let g = common::group("g1", "u1");
    groups::put_group(&repo, &g).await.unwrap();

    let result = groups::leave_group_tx(&repo, &UserId::new("ghost"), &GroupId::new("g1")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_group_patches_only_the_supplied_fields() {
    let (_c, repo) = common::make_repo().await;
    let original = common::group("g1", "u1");
    groups::put_group(&repo, &original).await.unwrap();

    let patch = groups::GroupPatch {
        name: Some("Renamed Crew".into()),
        member_soft_cap: Some(75),
        ..Default::default()
    };
    groups::update_group(&repo, &original.group_id, &patch)
        .await
        .unwrap();

    let got = groups::get_group(&repo, &original.group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.name, "Renamed Crew");
    assert_eq!(got.member_soft_cap, 75);
    assert_eq!(got.timezone, original.timezone);
    assert_eq!(got.gradient, original.gradient);
    assert_eq!(got.cycle_settings, original.cycle_settings);
}

#[tokio::test]
async fn update_group_leaves_member_count_untouched() {
    let (_c, repo) = common::make_repo().await;
    let mut original = common::group("g1", "u1");
    original.member_count = 7;
    groups::put_group(&repo, &original).await.unwrap();

    let patch = groups::GroupPatch {
        timezone: Some("Europe/Dublin".into()),
        ..Default::default()
    };
    groups::update_group(&repo, &original.group_id, &patch)
        .await
        .unwrap();

    let got = groups::get_group(&repo, &original.group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.member_count, 7);
}

#[tokio::test]
async fn update_membership_role_promotes_a_member() {
    let (_c, repo) = common::make_repo().await;
    let m = common::membership("u2", "g1", Role::Member);
    common::put_membership(&repo, &m).await;

    groups::update_membership_role(&repo, &m.user_id, &m.group_id, Role::Admin)
        .await
        .unwrap();

    let got = groups::get_membership(&repo, &m.user_id, &m.group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.role, Role::Admin);
}

#[tokio::test]
async fn update_membership_role_fails_when_membership_absent() {
    let (_c, repo) = common::make_repo().await;
    let result = groups::update_membership_role(
        &repo,
        &UserId::new("ghost"),
        &GroupId::new("g1"),
        Role::Admin,
    )
    .await;
    assert!(result.is_err());
}
