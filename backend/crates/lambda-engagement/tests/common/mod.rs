//! Shared test setup for `lambda-engagement` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup and entity factories via
//! the `test-utils` feature rather than duplicating them.
#![allow(dead_code, unused_imports)]

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{TimeZone, Utc};
use domain::{
    CycleId, Group, GroupId, GroupMembership, ImageId, ImageMedia, ImageMimeType, ImagePurpose,
    LockedQuestion, MediaStatus, Newsletter, NewsletterStatus, QuestionId, QuestionKind, Response,
    ResponseId, ResponseStatus, Role, UserId,
};
use engagement::state::AppState;
use persistence::keys::{
    attr, locked_pk, locked_sk, membership_gsi1pk, membership_gsi1sk, membership_sk,
    response_gsi1pk, response_gsi1sk, response_pk, response_sk, user_pk,
};
use persistence::{groups, media, newsletters, test_factories, users, Repo};

pub use test_factories::{group, locked_question, user, voting_newsletter};

pub async fn make_repo() -> (
    testcontainers::ContainerAsync<testcontainers_modules::dynamodb_local::DynamoDb>,
    Repo,
) {
    test_factories::make_test_repo().await
}

pub fn test_state(repo: Repo) -> AppState {
    AppState {
        repo,
        cdn_base_url: "https://cdn.example.com".into(),
    }
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

pub async fn put_user(repo: &Repo, user_id: &str) {
    users::put_user(repo, &test_factories::user(user_id))
        .await
        .expect("user written");
}

/// A `published` newsletter — the status every engagement route requires.
pub fn published_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Published;
    nl.published_at = Some(Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap());
    nl.next_transition_at = None;
    nl
}

/// An `open` newsletter — every engagement route must refuse this with
/// `CYCLE_NOT_PUBLISHED`.
pub fn open_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Open;
    nl
}

/// An `archived` newsletter — every engagement route must refuse this with
/// `NEWSLETTER_ARCHIVED`.
pub fn archived_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Archived;
    nl
}

pub async fn put_newsletter(repo: &Repo, nl: &Newsletter) {
    newsletters::write_status_transition(repo, nl)
        .await
        .expect("newsletter written");
}

pub async fn put_locked_question(repo: &Repo, lq: &LockedQuestion) {
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

/// Writes a published `Response` row directly (bypassing the save/publish
/// transaction) with an explicit, known `responseId` — the engagement routes
/// address answers by `responseId`, so tests need to control it.
#[allow(clippy::too_many_arguments)]
pub async fn put_published_text_answer(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
    response_id: &str,
    body: &str,
) {
    let response = Response {
        response_id: ResponseId::new(response_id),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        status: ResponseStatus::Published,
        body: Some(body.to_owned()),
        poll_option_id: None,
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
        published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
    };
    put_response(repo, &response).await;
}

/// A published **poll** answer — engagement routes must 404 on these
/// (`09-engagement.md` §3, decided M10 #1/#13).
pub async fn put_published_poll_answer(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
    response_id: &str,
) {
    let response = Response {
        response_id: ResponseId::new(response_id),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Poll,
        status: ResponseStatus::Published,
        body: None,
        poll_option_id: Some(domain::PollOptionId::new("opt-a")),
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
        published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
    };
    put_response(repo, &response).await;
}

/// A **draft** (unpublished) text answer — engagement routes must 404 on
/// these too (only published answers are engagement targets).
pub async fn put_draft_text_answer(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
    response_id: &str,
) {
    let response = Response {
        response_id: ResponseId::new(response_id),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        status: ResponseStatus::Draft,
        body: Some("still drafting".into()),
        poll_option_id: None,
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
        published_at: None,
    };
    put_response(repo, &response).await;
}

async fn put_response(repo: &Repo, r: &Response) {
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
    item.insert(attr::ENTITY.into(), AttributeValue::S("Response".into()));
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("response written");
}

/// Writes a `purpose=comment` `ImageMedia` row, ready for a comment to claim.
#[allow(clippy::too_many_arguments)]
pub async fn seed_comment_image(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    image_id: &str,
    user_id: &str,
    status: MediaStatus,
) -> ImageId {
    let img = ImageMedia {
        image_id: ImageId::new(image_id),
        user_id: UserId::new(user_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: Some(QuestionId::new(question_id)),
        purpose: ImagePurpose::Comment,
        mime_type: ImageMimeType::Jpeg,
        original_key: format!(
            "uploads/{group_id}/{cycle_id}/{question_id}/{user_id}/{image_id}.jpg"
        ),
        display_key: Some(format!("processed/{image_id}/display.webp")),
        thumb_key: Some(format!("processed/{image_id}/thumb.webp")),
        status,
        bytes: 123_456,
        width: Some(1200),
        height: Some(800),
        caption: None,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 4, 0, 0, 0).unwrap(),
        processed_at: Some(Utc.with_ymd_and_hms(2026, 6, 4, 0, 1, 0).unwrap()),
        error_message: None,
        attached_comment_id: None,
    };
    media::put_image(repo, &img).await.expect("image written");
    img.image_id
}

pub async fn get_image(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    image_id: &ImageId,
) -> ImageMedia {
    media::get_image(
        repo,
        &GroupId::new(group_id),
        &CycleId::new(cycle_id),
        image_id,
    )
    .await
    .expect("image lookup")
    .expect("image exists")
}
