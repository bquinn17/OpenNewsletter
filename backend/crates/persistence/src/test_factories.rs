//! Test factories. Available to downstream crates via the `test-utils` feature.

use chrono::{TimeZone, Utc};
use domain::*;

/// Where the integration tests find DynamoDB Local when
/// `DYNAMODB_LOCAL_ENDPOINT` is unset: the shared container `make
/// dynamodb-local` starts (`docs/development/testing-and-ci.md` §2.4).
pub const DEFAULT_DYNAMODB_LOCAL_ENDPOINT: &str = "http://127.0.0.1:8000";

/// Creates a uniquely-named table shaped like `OpenNewsletter` (base table +
/// `gsi1` + `gsi2`) on the shared DynamoDB Local at `DYNAMODB_LOCAL_ENDPOINT`.
/// Mirrors `persistence/tests/common/mod.rs::make_repo`, exposed here so
/// downstream crates' integration tests (e.g. `lambda-questions/tests/`) don't
/// duplicate the table-shape setup. The caller must keep the returned
/// [`TestTable`] alive for the duration of the test (prefix it with `_`); the
/// table is deleted when it drops.
pub async fn make_test_repo() -> (TestTable, crate::Repo) {
    use aws_sdk_dynamodb::types::{
        AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType,
        Projection, ProjectionType, ScalarAttributeType,
    };

    let endpoint = std::env::var("DYNAMODB_LOCAL_ENDPOINT")
        .ok()
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| DEFAULT_DYNAMODB_LOCAL_ENDPOINT.to_owned());
    let client = local_client(&endpoint);
    if let Err(e) = client.list_tables().limit(1).send().await {
        panic!(
            "DynamoDB Local isn't answering at {endpoint}: start it with `make dynamodb-local`, \
             or set DYNAMODB_LOCAL_ENDPOINT ({e:?})"
        );
    }

    let table = format!("test-{}", uuid::Uuid::new_v4());

    fn s_attr(name: &str) -> AttributeDefinition {
        AttributeDefinition::builder()
            .attribute_name(name)
            .attribute_type(ScalarAttributeType::S)
            .build()
            .unwrap()
    }
    fn hash(name: &str) -> KeySchemaElement {
        KeySchemaElement::builder()
            .attribute_name(name)
            .key_type(KeyType::Hash)
            .build()
            .unwrap()
    }
    fn range(name: &str) -> KeySchemaElement {
        KeySchemaElement::builder()
            .attribute_name(name)
            .key_type(KeyType::Range)
            .build()
            .unwrap()
    }
    let all = Projection::builder()
        .projection_type(ProjectionType::All)
        .build();

    client
        .create_table()
        .table_name(&table)
        .attribute_definitions(s_attr("pk"))
        .attribute_definitions(s_attr("sk"))
        .attribute_definitions(s_attr("gsi1pk"))
        .attribute_definitions(s_attr("gsi1sk"))
        .attribute_definitions(s_attr("gsi2pk"))
        .attribute_definitions(s_attr("gsi2sk"))
        .billing_mode(BillingMode::PayPerRequest)
        .key_schema(hash("pk"))
        .key_schema(range("sk"))
        .global_secondary_indexes(
            GlobalSecondaryIndex::builder()
                .index_name("gsi1")
                .key_schema(hash("gsi1pk"))
                .key_schema(range("gsi1sk"))
                .projection(all.clone())
                .build()
                .unwrap(),
        )
        .global_secondary_indexes(
            GlobalSecondaryIndex::builder()
                .index_name("gsi2")
                .key_schema(hash("gsi2pk"))
                .key_schema(range("gsi2sk"))
                .projection(all)
                .build()
                .unwrap(),
        )
        .send()
        .await
        .expect("create table");

    let guard = TestTable {
        endpoint,
        table: table.clone(),
    };
    (guard, crate::Repo::new(client, table))
}

/// One test's table on the shared DynamoDB Local, deleted when this drops
/// (including when the test panics), so the in-memory database doesn't grow
/// across runs.
pub struct TestTable {
    endpoint: String,
    table: String,
}

impl Drop for TestTable {
    fn drop(&mut self) {
        let endpoint = std::mem::take(&mut self.endpoint);
        let table = std::mem::take(&mut self.table);
        // `drop` runs inside the test's runtime, which can't block on a
        // future itself. A runtime of its own on another thread can, with a
        // fresh client, since the test's client keeps connections bound to
        // the test's runtime.
        let deleted = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build the table-cleanup runtime");
            runtime.block_on(async {
                local_client(&endpoint)
                    .delete_table()
                    .table_name(&table)
                    .send()
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("{table}: {e:?}"))
            })
        })
        .join();
        // Best effort: a leftover table only costs memory until DynamoDB
        // Local restarts, so it never fails the test.
        if let Ok(Err(e)) = deleted {
            eprintln!("could not delete test table {e}");
        }
    }
}

fn local_client(endpoint: &str) -> aws_sdk_dynamodb::Client {
    use aws_sdk_dynamodb::config::{timeout::TimeoutConfig, BehaviorVersion, Credentials, Region};

    let config = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .endpoint_url(endpoint)
        .region(Region::new("us-east-1"))
        .credentials_provider(Credentials::new("fake", "fake", None, None, "test"))
        // No SDK default: a port that accepts connections but never answers
        // would otherwise hang the test forever.
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .operation_attempt_timeout(std::time::Duration::from_secs(10))
                .build(),
        )
        .build();
    aws_sdk_dynamodb::Client::from_conf(config)
}

pub fn user(user_id: &str) -> User {
    User {
        user_id: UserId::new(user_id),
        cognito_sub: CognitoSub::new(format!("sub-{user_id}")),
        email: format!("{user_id}@example.com"),
        display_name: format!("User {user_id}"),
        avatar_color: "grape".into(),
        avatar_media_id: None,
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        last_login_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    }
}

pub fn group(group_id: &str, created_by: &str) -> Group {
    Group {
        group_id: GroupId::new(group_id),
        name: format!("Group {group_id}"),
        timezone: shared::config::DEFAULT_TIMEZONE.into(),
        cycle_settings: CycleSettings {
            questions_per_cycle: shared::config::DEFAULT_QUESTIONS_PER_CYCLE,
            votes_per_user_per_cycle: shared::config::DEFAULT_VOTES_PER_USER_PER_CYCLE,
            response_window_days: shared::config::DEFAULT_RESPONSE_WINDOW_DAYS,
            auto_publish: true,
        },
        notification_settings: NotificationSettings {
            offsets_hours_before_close: shared::config::DEFAULT_NOTIFY_OFFSETS_HOURS.to_vec(),
            on_cycle_open: true,
        },
        member_count: 1,
        member_soft_cap: shared::config::DEFAULT_MEMBER_SOFT_CAP,
        gradient: "grape-sky".into(),
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        created_by: UserId::new(created_by),
    }
}

pub fn membership(user_id: &str, group_id: &str, role: Role) -> GroupMembership {
    GroupMembership {
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        role,
        joined_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        editions_answered: 0,
    }
}

pub fn voting_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let open = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    let close = Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap();
    Newsletter {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        status: NewsletterStatus::Voting,
        vote_window_open_at: Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap(),
        vote_window_close_at: open,
        response_open_at: open,
        response_close_at: close,
        published_at: None,
        next_transition_at: Some(open),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
    }
}

pub fn candidate(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    submitter: &str,
    votes: u32,
) -> CandidateQuestion {
    CandidateQuestion {
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        next_cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        prompt: "What's your favorite hike?".into(),
        poll_options: None,
        vote_count: votes,
        submitted_by: UserId::new(submitter),
        is_anonymous: false,
        submitted_at: Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap(),
    }
}

pub fn vote(user_id: &str, group_id: &str, cycle_id: &str, question_id: &str) -> CandidateVote {
    CandidateVote {
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        voted_at: Utc.with_ymd_and_hms(2026, 5, 15, 12, 0, 0).unwrap(),
    }
}

pub fn locked_question(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    submitter: &str,
) -> LockedQuestion {
    LockedQuestion {
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        prompt: "What's your favorite hike?".into(),
        poll_options: None,
        display_order: 0,
        submitted_by: UserId::new(submitter),
        is_anonymous: false,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    }
}
