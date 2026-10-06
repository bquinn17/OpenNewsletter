//! Shared test setup for `lambda-media` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup via the `test-utils`
//! feature rather than duplicating it.
#![allow(dead_code, unused_imports)]

use chrono::{TimeZone, Utc};
use domain::{
    AvatarId, AvatarMedia, CycleId, Group, GroupId, GroupMembership, ImageId, ImageMedia,
    ImageMimeType, ImagePurpose, LockedQuestion, MediaStatus, Newsletter, NewsletterStatus,
    QuestionId, QuestionKind, Response, ResponseId, ResponseStatus, Role, UserId,
};
use media::state::AppState;
use persistence::{groups, media as media_persist, test_factories, Repo};
use rsa::RsaPrivateKey;

pub use test_factories::{candidate, group, locked_question, membership, user, vote};

/// A throwaway 2048-bit RSA key (PKCS#1 PEM), generated solely for this test
/// suite — never used outside it, and never a production credential.
const TEST_SIGNING_KEY_PEM: &str = include_str!("../fixtures/test_signing_key.pem");

pub async fn make_repo() -> (
    testcontainers::ContainerAsync<testcontainers_modules::dynamodb_local::DynamoDb>,
    Repo,
) {
    test_factories::make_test_repo().await
}

pub fn test_signing_key() -> RsaPrivateKey {
    media::cloudfront::parse_private_key(TEST_SIGNING_KEY_PEM).expect("valid test signing key")
}

/// An `AppState` wired for offline tests: a real `aws-sdk-s3` client built
/// with static test credentials (presigning needs no network — it only
/// builds and signs a request locally), a Secrets Manager client that is
/// never actually called because the signing key is injected directly, and
/// a non-empty `MEDIA_COOKIE_DOMAIN` so `/media-cookie` emits `Set-Cookie`
/// headers by default (`test_state_without_cookie_domain` covers the dev
/// case where it doesn't).
pub fn test_state(repo: Repo) -> AppState {
    let state = AppState::new(
        repo,
        test_s3_client(),
        test_secrets_client(),
        "test-media-originals".into(),
        "test-avatars-originals".into(),
        "https://cdn.example.com".into(),
        "KTESTKEYPAIR".into(),
        "arn:aws:secretsmanager:us-east-1:123456789012:secret:unused".into(),
        ".example.com".into(),
    );
    state.set_signing_key_for_test(test_signing_key());
    state
}

/// Same as [`test_state`] but with `MEDIA_COOKIE_DOMAIN` empty, matching dev
/// (`08-media-uploads.md` §4.5) — `/media-cookie` must emit no `Set-Cookie`
/// headers in this configuration.
pub fn test_state_without_cookie_domain(repo: Repo) -> AppState {
    let state = AppState::new(
        repo,
        test_s3_client(),
        test_secrets_client(),
        "test-media-originals".into(),
        "test-avatars-originals".into(),
        "https://cdn.example.com".into(),
        "KTESTKEYPAIR".into(),
        "arn:aws:secretsmanager:us-east-1:123456789012:secret:unused".into(),
        String::new(),
    );
    state.set_signing_key_for_test(test_signing_key());
    state
}

fn test_credentials() -> aws_sdk_s3::config::Credentials {
    aws_sdk_s3::config::Credentials::new("test", "test", None, None, "test")
}

fn test_s3_client() -> aws_sdk_s3::Client {
    let config = aws_sdk_s3::Config::builder()
        .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .credentials_provider(test_credentials())
        .request_checksum_calculation(aws_sdk_s3::config::RequestChecksumCalculation::WhenRequired)
        .build();
    aws_sdk_s3::Client::from_conf(config)
}

fn test_secrets_client() -> aws_sdk_secretsmanager::Client {
    let config = aws_sdk_secretsmanager::Config::builder()
        .behavior_version(aws_sdk_secretsmanager::config::BehaviorVersion::latest())
        .region(aws_sdk_secretsmanager::config::Region::new("us-east-1"))
        .credentials_provider(aws_sdk_secretsmanager::config::Credentials::new(
            "test", "test", None, None, "test",
        ))
        .build();
    aws_sdk_secretsmanager::Client::from_conf(config)
}

/// Writes a `Group` row and a `GroupMembership` row for `user_id`, so
/// `auth::require_membership` finds a real membership.
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

pub fn voting_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    test_factories::voting_newsletter(group_id, cycle_id)
}

/// An `open` newsletter — the status `POST /uploads` requires for
/// `purpose=response`.
pub fn open_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = test_factories::voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Open;
    nl
}

/// A `published` newsletter — the status `POST /uploads` requires for
/// `purpose=comment`.
pub fn published_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = open_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Published;
    nl.published_at = Some(Utc::now());
    nl
}

pub fn archived_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = open_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Archived;
    nl
}

/// Writes a `LockedQuestion` row directly — there's no "promote candidates"
/// shortcut for tests that just need a question locked into a cycle.
pub async fn put_locked_question(repo: &Repo, lq: &LockedQuestion) {
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, locked_pk, locked_sk};

    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(lq).expect("locked question serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(locked_pk(&lq.group_id, &lq.cycle_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(locked_sk(&lq.question_id)),
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

pub fn text_locked_question(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    submitter: &str,
) -> LockedQuestion {
    test_factories::locked_question(group_id, cycle_id, question_id, submitter)
}

#[allow(clippy::too_many_arguments)]
pub async fn seed_image(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    image_id: &str,
    user_id: &str,
    status: MediaStatus,
    purpose: ImagePurpose,
) -> ImageId {
    let img = ImageMedia {
        image_id: ImageId::new(image_id),
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: Some(QuestionId::new(question_id)),
        purpose,
        mime_type: ImageMimeType::Jpeg,
        original_key: format!(
            "uploads/{group_id}/{cycle_id}/{question_id}/{user_id}/{image_id}.jpg"
        ),
        display_key: None,
        thumb_key: None,
        status,
        bytes: 123_456,
        width: None,
        height: None,
        caption: None,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
        processed_at: None,
        error_message: None,
        attached_comment_id: None,
    };
    media_persist::put_image(repo, &img)
        .await
        .expect("image written");
    img.image_id
}

/// Claims a `purpose=comment` image for a comment, bypassing the comment
/// create/patch transaction — just enough for `lambda-media`'s `DELETE
/// /uploads/{imageId}` `IMAGE_IN_USE` check (`03-api-contract.md` §9.4) to
/// have something to find.
pub async fn attach_image_to_comment(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    image_id: &ImageId,
    comment_id: &str,
) {
    let mut img = media_persist::get_image(
        repo,
        &GroupId::new(group_id),
        &CycleId::new(cycle_id),
        image_id,
    )
    .await
    .expect("image lookup")
    .expect("image exists");
    img.attached_comment_id = Some(domain::CommentId::new(comment_id));
    media_persist::put_image(repo, &img)
        .await
        .expect("image re-written with attached_comment_id");
}

/// Writes a `Response` row directly (bypassing the publish transaction) so
/// `DELETE /uploads/{imageId}`'s `IMAGE_IN_USE` check has a published answer
/// to find.
pub async fn put_published_response(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
    image_media_ids: &[&str],
) {
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, response_gsi1pk, response_gsi1sk, response_pk, response_sk};

    let response = Response {
        response_id: ResponseId::generate(),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        status: ResponseStatus::Published,
        body: Some("Lake 22, hands down.".into()),
        poll_option_id: None,
        image_media_ids: image_media_ids.iter().map(|i| ImageId::new(*i)).collect(),
        updated_at: Utc::now(),
        published_at: Some(Utc::now()),
    };

    let mut item: std::collections::HashMap<String, AttributeValue> =
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

pub async fn seed_avatar(
    repo: &Repo,
    user_id: &str,
    avatar_id: &str,
    status: MediaStatus,
) -> AvatarId {
    let avatar = AvatarMedia {
        avatar_id: AvatarId::new(avatar_id),
        user_id: UserId::new(user_id),
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("uploads/{user_id}/{avatar_id}.jpg"),
        display_key: Some(format!("avatar/{avatar_id}/display.webp")),
        status,
        bytes: 65_536,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
        processed_at: Some(Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 1).unwrap()),
        error_message: None,
    };
    media_persist::put_avatar(repo, &avatar)
        .await
        .expect("avatar written");
    avatar.avatar_id
}
