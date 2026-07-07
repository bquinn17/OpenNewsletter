//! Shared helpers for DDB-local integration tests.
#![allow(dead_code)]
//!
//! Each test calls `make_repo()` which starts a fresh DynamoDB-local container
//! and creates a uniquely-named table. The caller must keep the returned
//! `ContainerAsync` alive for the duration of the test (prefix it with `_`).

use aws_sdk_dynamodb::{
    config::{BehaviorVersion, Credentials, Region},
    types::{
        AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType,
        Projection, ProjectionType, ScalarAttributeType,
    },
    Client,
};
use chrono::{Duration, TimeZone, Utc};
use domain::*;
use persistence::Repo;
use testcontainers::ContainerAsync;
use testcontainers_modules::dynamodb_local::DynamoDb;
use uuid::Uuid;

pub async fn make_repo() -> (ContainerAsync<DynamoDb>, Repo) {
    use testcontainers::runners::AsyncRunner;
    let container = DynamoDb::default()
        .start()
        .await
        .expect("DynamoDB local started");
    let port = container
        .get_host_port_ipv4(8000)
        .await
        .expect("DDB port available");

    let endpoint = format!("http://localhost:{port}");
    let creds = Credentials::new("fake", "fake", None, None, "test");
    let config = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .endpoint_url(&endpoint)
        .region(Region::new("us-east-1"))
        .credentials_provider(creds)
        .build();
    let client = Client::from_conf(config);

    let table = format!("test-{}", Uuid::new_v4());
    create_table(&client, &table).await;
    (container, Repo::new(client, table))
}

async fn create_table(client: &Client, table: &str) {
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
        .table_name(table)
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
}

// ── Factories ────────────────────────────────────────────────────────────────

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
        timezone: "America/New_York".into(),
        cycle_settings: CycleSettings {
            questions_per_cycle: 5,
            votes_per_user_per_cycle: 3,
            response_window_days: 4,
            auto_publish: true,
        },
        notification_settings: NotificationSettings {
            offsets_hours_before_close: vec![24, 2],
            on_cycle_open: true,
        },
        member_count: 1,
        member_soft_cap: 20,
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

pub fn invite(code: &str, group_id: &str, created_by: &str) -> Invite {
    Invite {
        code: InviteCode::new(code),
        group_id: GroupId::new(group_id),
        created_by: UserId::new(created_by),
        created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        expires_at: Utc.with_ymd_and_hms(2026, 6, 8, 0, 0, 0).unwrap(),
        status: InviteStatus::Pending,
        consumed_by: None,
        consumed_at: None,
        role_on_redeem: Role::Member,
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

pub fn draft_response(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
) -> Response {
    Response {
        response_id: ResponseId::new("resp-01"),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        status: ResponseStatus::Draft,
        body: Some("My draft answer".into()),
        poll_option_id: None,
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
        published_at: None,
    }
}

pub fn comment(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    answer_user_id: &str,
    author_user_id: &str,
    comment_id: &str,
) -> Comment {
    Comment {
        comment_id: CommentId::new(comment_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        answer_user_id: UserId::new(answer_user_id),
        author_user_id: UserId::new(author_user_id),
        body: "Nice answer!".into(),
        image_media_id: None,
        created_at: Utc.with_ymd_and_hms(2026, 6, 6, 10, 0, 0).unwrap(),
        edited_at: None,
        deleted_at: None,
    }
}

pub fn reaction(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    answer_user_id: &str,
    reactor_user_id: &str,
) -> Reaction {
    Reaction {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        answer_user_id: UserId::new(answer_user_id),
        reactor_user_id: UserId::new(reactor_user_id),
        emoji: "❤️".into(),
        created_at: Utc.with_ymd_and_hms(2026, 6, 6, 10, 0, 0).unwrap(),
    }
}

pub fn image_media(group_id: &str, cycle_id: &str, image_id: &str, user_id: &str) -> ImageMedia {
    ImageMedia {
        image_id: ImageId::new(image_id),
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: None,
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("images/{image_id}/orig.jpg"),
        display_key: None,
        thumb_key: None,
        status: MediaStatus::Pending,
        bytes: 12345,
        width: None,
        height: None,
        caption: None,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap(),
        processed_at: None,
    }
}

pub fn avatar_media(user_id: &str, avatar_id: &str) -> AvatarMedia {
    AvatarMedia {
        avatar_id: AvatarId::new(avatar_id),
        user_id: UserId::new(user_id),
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("avatars/{avatar_id}/orig.jpg"),
        display_key: None,
        status: MediaStatus::Pending,
        bytes: 5678,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 12, 0, 0).unwrap(),
        processed_at: None,
    }
}

pub fn push_subscription(user_id: &str, endpoint_hash: &str) -> PushSubscription {
    PushSubscription {
        user_id: UserId::new(user_id),
        endpoint_hash: endpoint_hash.into(),
        endpoint: format!("https://push.example.com/{endpoint_hash}"),
        p256dh: "abc123".into(),
        auth: "secret".into(),
        created_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
        last_success_at: None,
        failure_count: 0,
        user_agent: "Mozilla/5.0".into(),
    }
}

pub fn notification_pref(user_id: &str, group_id: &str) -> NotificationPref {
    NotificationPref {
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_open: true,
        deadline_reminders: true,
    }
}

// Suppress "unused import" warnings when only some factories are used per file.
#[allow(dead_code)]
pub fn _unused() {
    let _ = Duration::days(1);
}
