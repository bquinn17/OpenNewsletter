//! Shared setup for `lambda-groups` handler tests.
#![allow(dead_code)]

use aws_sdk_dynamodb::types::AttributeValue;
use domain::{GroupMembership, LockedQuestion, Response, Role};
use groups::state::AppState;
use media_delete::fakes::{fake_deleter, RecordingObjectStore};
use persistence::keys::{
    attr, locked_pk, locked_sk, membership_gsi1pk, membership_gsi1sk, membership_sk,
    reacted_gsi2pk, reacted_gsi2sk, response_gsi1pk, response_gsi1sk, response_pk, response_sk,
    user_pk,
};
use persistence::{groups as group_repo, test_factories, users, Repo};
use shared::cycle_time::CycleCadence;
use std::sync::Arc;

pub const AVATARS_ORIGINALS: &str = "test-avatars-originals";
pub const PROCESSED: &str = "test-processed";

pub fn state(repo: Repo) -> AppState {
    state_with_fakes(repo).0
}

/// A monthly `state`, but with its cadence overridden — e.g. to weekly, for
/// `docs/design/lifecycle.md` §13 tests.
pub fn state_with_cadence(repo: Repo, cadence: CycleCadence) -> AppState {
    AppState {
        cadence,
        ..state(repo)
    }
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
        cadence: CycleCadence::Monthly,
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

/// Writes a published text `Response` row with its `reaction_count` and GSI2
/// "most reacted" keys already set, the shape the counted-reaction
/// transaction leaves behind (`docs/design/data-model.md` §2.9) — for
/// leaderboard `topAnswers` fixtures, which read GSI2 directly rather than
/// exercising the reaction transaction itself.
pub async fn put_reacted_response(repo: &Repo, r: &Response) {
    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(r).expect("response serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(response_pk(&r.group_id, &r.cycle_id, &r.question_id)),
    );
    item.insert(attr::SK.into(), AttributeValue::S(response_sk(&r.user_id)));
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(response_gsi1pk(&r.user_id, &r.cycle_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(response_gsi1sk(&r.question_id)),
    );
    item.insert(
        attr::GSI2PK.into(),
        AttributeValue::S(reacted_gsi2pk(&r.group_id)),
    );
    item.insert(
        attr::GSI2SK.into(),
        AttributeValue::S(reacted_gsi2sk(r.reaction_count, &r.response_id)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Response".into()));
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("response written");
}

/// Writes a `LockedQuestion` row so a `topAnswers` entry can hydrate its
/// `questionPrompt`.
pub async fn put_locked_question(repo: &Repo, q: &LockedQuestion) {
    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(q).expect("locked question serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(locked_pk(&q.group_id, &q.cycle_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(locked_sk(&q.question_id)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("LockedQuestion".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("locked question written");
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
