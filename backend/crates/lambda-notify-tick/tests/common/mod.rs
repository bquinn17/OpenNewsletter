//! Shared test setup for `lambda-notify-tick` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup and entity factories via
//! the `test-utils` feature, and a scripted [`FakePushSender`] so no test
//! ever makes a real network call.
#![allow(dead_code, unused_imports)]

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use domain::{GroupId, PushSubscription, UserId};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use persistence::keys::attr;
use persistence::{groups, newsletters, test_factories, users, Repo};
use push::sender::{PushSender, SendResult};
use push::state::AppState;
use rand::RngCore;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

pub use test_factories::{group, locked_question, user, voting_newsletter};

pub async fn make_repo() -> (
    testcontainers::ContainerAsync<testcontainers_modules::dynamodb_local::DynamoDb>,
    Repo,
) {
    test_factories::make_test_repo().await
}

pub fn test_state(repo: Repo, sender: Arc<FakePushSender>, env: &str) -> AppState {
    let state = AppState::new(
        repo,
        dummy_secrets_client(),
        "arn:aws:secretsmanager:us-east-1:111111111111:secret:unused".to_owned(),
        sender,
        env.to_owned(),
    );
    state.set_vapid_keys_for_test(make_test_vapid_keys());
    state
}

fn dummy_secrets_client() -> aws_sdk_secretsmanager::Client {
    use aws_sdk_secretsmanager::config::{BehaviorVersion, Credentials, Region};
    let config = aws_sdk_secretsmanager::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new("fake", "fake", None, None, "test"))
        .build();
    aws_sdk_secretsmanager::Client::from_conf(config)
}

pub fn make_test_vapid_keys() -> push::webpush::VapidKeys {
    let secret = p256::SecretKey::random(&mut rand::thread_rng());
    let public_point = secret.public_key().to_encoded_point(false);
    push::webpush::VapidKeys::parse(
        &URL_SAFE_NO_PAD.encode(public_point.as_bytes()),
        &URL_SAFE_NO_PAD.encode(secret.to_bytes()),
        "mailto:admin@opennewsletter.example.com".to_owned(),
    )
    .expect("valid test vapid keypair")
}

pub fn make_subscriber_keys() -> (String, String) {
    let secret = p256::SecretKey::random(&mut rand::thread_rng());
    let public = secret.public_key().to_encoded_point(false);
    let p256dh = URL_SAFE_NO_PAD.encode(public.as_bytes());
    let mut auth_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut auth_bytes);
    let auth = URL_SAFE_NO_PAD.encode(auth_bytes);
    (p256dh, auth)
}

#[derive(Default)]
pub struct FakePushSender {
    scripts: Mutex<HashMap<String, VecDeque<SendResult>>>,
    calls: Mutex<Vec<String>>,
}

impl FakePushSender {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl PushSender for FakePushSender {
    async fn send(
        &self,
        endpoint: &str,
        _headers: &[(String, String)],
        _body: Vec<u8>,
    ) -> SendResult {
        self.calls.lock().unwrap().push(endpoint.to_owned());
        SendResult::Status(201)
    }
}

pub async fn seed_group_and_membership(
    repo: &Repo,
    group: &domain::Group,
    user_id: &str,
    role: domain::Role,
) {
    use persistence::keys::{membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};

    groups::put_group(repo, group).await.expect("group written");
    let m = domain::GroupMembership {
        user_id: UserId::new(user_id),
        group_id: group.group_id.clone(),
        role,
        joined_at: group.created_at,
        editions_answered: 0,
    };
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&m).expect("membership serializes");
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

pub async fn put_user(repo: &Repo, user_id: &str) {
    users::put_user(repo, &test_factories::user(user_id))
        .await
        .expect("user written");
}

pub async fn put_cognito_sub_lookup(repo: &Repo, sub: &str, user_id: &str) {
    use persistence::keys::{cognito_sub_pk, COGNITO_SUB_SK};
    let lookup = domain::CognitoSubLookup {
        cognito_sub: domain::CognitoSub::new(sub),
        user_id: UserId::new(user_id),
    };
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&lookup).expect("lookup serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(cognito_sub_pk(&lookup.cognito_sub)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(COGNITO_SUB_SK.to_owned()),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("CognitoSubLookup".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("lookup written");
}

pub async fn put_newsletter(repo: &Repo, nl: &domain::Newsletter) {
    newsletters::write_status_transition(repo, nl)
        .await
        .expect("newsletter written");
}

/// An `open` newsletter with the given response window and locked question
/// ids, for deadline-reminder tests.
pub fn open_newsletter(
    group_id: &str,
    cycle_id: &str,
    response_open_at: DateTime<Utc>,
    response_close_at: DateTime<Utc>,
    locked_question_ids: Vec<domain::QuestionId>,
) -> domain::Newsletter {
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = domain::NewsletterStatus::Open;
    nl.response_open_at = response_open_at;
    nl.response_close_at = response_close_at;
    nl.vote_window_close_at = response_open_at;
    nl.next_transition_at = Some(response_close_at);
    nl.locked_question_ids = locked_question_ids;
    nl
}

#[allow(clippy::too_many_arguments)]
pub async fn put_subscription(
    repo: &Repo,
    user_id: &str,
    endpoint_hash: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: &str,
    created_at: DateTime<Utc>,
) {
    let sub = PushSubscription {
        user_id: UserId::new(user_id),
        endpoint_hash: endpoint_hash.to_owned(),
        endpoint: endpoint.to_owned(),
        p256dh: p256dh.to_owned(),
        auth: auth.to_owned(),
        created_at,
        last_success_at: None,
        failure_count: 0,
        user_agent: user_agent.to_owned(),
    };
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&sub).expect("subscription serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(persistence::keys::user_pk(&sub.user_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(persistence::keys::push_sk(&sub.endpoint_hash)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("PushSubscription".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("subscription written");
}

/// Writes a published text `Response` row directly (bypassing the
/// save/publish transaction), for "done member" tests.
pub async fn put_published_text_answer(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
    response_id: &str,
) {
    use chrono::TimeZone;
    use persistence::keys::{response_gsi1pk, response_gsi1sk, response_pk, response_sk};

    let response = domain::Response {
        response_id: domain::ResponseId::new(response_id),
        user_id: UserId::new(user_id),
        question_id: domain::QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: domain::CycleId::new(cycle_id),
        kind: domain::QuestionKind::Text,
        status: domain::ResponseStatus::Published,
        body: Some("An answer".to_owned()),
        poll_option_id: None,
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
        published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
    };
    let mut item: HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&response).expect("response serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(response_pk(
            &response.group_id,
            &response.cycle_id,
            &response.question_id,
        )),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(response_sk(&response.user_id)),
    );
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(response_gsi1pk(&response.user_id, &response.cycle_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(response_gsi1sk(&response.question_id)),
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
