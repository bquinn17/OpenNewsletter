//! Shared test setup for `lambda-newsletters` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup and entity factories via
//! the `test-utils` feature rather than duplicating them.
#![allow(dead_code, unused_imports)]

use domain::{Group, GroupMembership, Role, UserId};
use persistence::{groups, test_factories, Repo};

pub use test_factories::{
    candidate, group, locked_question, membership, user, vote, voting_newsletter,
};

pub async fn make_repo() -> (
    testcontainers::ContainerAsync<testcontainers_modules::dynamodb_local::DynamoDb>,
    Repo,
) {
    test_factories::make_test_repo().await
}

/// Writes a `Group` row and a `GroupMembership` row for `user_id`, so
/// `auth::require_membership` finds a real membership rather than needing its
/// own direct-write helper.
pub async fn seed_group_and_membership(repo: &Repo, group: &Group, user_id: &str, role: Role) {
    groups::put_group(repo, group).await.expect("group written");
    let m = GroupMembership {
        user_id: UserId::new(user_id),
        group_id: group.group_id.clone(),
        role,
        joined_at: group.created_at,
        editions_answered: 0,
    };
    put_membership(repo, &m).await;
}

async fn put_membership(repo: &Repo, m: &GroupMembership) {
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};

    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(m).expect("membership serializes");
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
        .expect("membership written");
}
