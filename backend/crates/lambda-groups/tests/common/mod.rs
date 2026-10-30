//! Shared setup for `lambda-groups` handler tests.
#![allow(dead_code)]

use aws_sdk_dynamodb::types::AttributeValue;
use domain::{GroupMembership, Role};
use groups::state::AppState;
use media_delete::fakes::{fake_deleter, RecordingObjectStore};
use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
use persistence::{groups as group_repo, test_factories, users, Repo};
use std::sync::Arc;

pub const AVATARS_ORIGINALS: &str = "test-avatars-originals";
pub const PROCESSED: &str = "test-processed";

pub fn state(repo: Repo) -> AppState {
    state_with_fakes(repo).0
}

/// An `AppState` whose avatar deleter records into the returned fake.
pub fn state_with_fakes(repo: Repo) -> (AppState, Arc<RecordingObjectStore>) {
    let (avatar_deleter, store) = fake_deleter(AVATARS_ORIGINALS, PROCESSED);
    let state = AppState {
        repo,
        group_defaults: serde_json::json!({}),
        vapid_public_key: String::new(),
        media_urls: media_urls::MediaUrls::for_tests(PROCESSED),
        avatar_deleter,
    };
    (state, store)
}

/// A group whose members are `(user_id, role)`, each with a profile row.
pub async fn seed(repo: &Repo, group_id: &str, members: &[(&str, Role)]) {
    group_repo::put_group(repo, &test_factories::group(group_id, members[0].0))
        .await
        .unwrap();
    for (user_id, role) in members {
        users::put_user(repo, &test_factories::user(user_id))
            .await
            .unwrap();
        put_membership(repo, &test_factories::membership(user_id, group_id, *role)).await;
    }
}

async fn put_membership(repo: &Repo, m: &GroupMembership) {
    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(m).unwrap();
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
