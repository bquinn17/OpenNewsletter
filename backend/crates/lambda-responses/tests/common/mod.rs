//! Shared test setup for `lambda-responses` integration tests. Reuses
//! `persistence`'s DDB-local container/table setup and entity factories via
//! the `test-utils` feature rather than duplicating them.
#![allow(dead_code, unused_imports)]

use chrono::{Duration, TimeZone, Utc};
use domain::{
    CycleId, Group, GroupId, GroupMembership, ImageId, ImageMedia, ImageMimeType, ImagePurpose,
    LockedQuestion, MediaStatus, Newsletter, NewsletterStatus, PollOption, PollOptionId,
    QuestionId, QuestionKind, Role, UserId,
};
use persistence::{groups, media, test_factories, Repo};

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

/// An `open` newsletter with a response window comfortably in the future,
/// relative to the real wall clock — `handlers::save_response` calls
/// `Utc::now()` directly (like every other handler's timestamps in this
/// codebase), so fixed calendar dates would go stale the day this test suite
/// outlives them.
pub fn open_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let now = Utc::now();
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Open;
    nl.response_open_at = now - Duration::days(1);
    nl.response_close_at = now + Duration::days(4);
    nl.next_transition_at = Some(nl.response_close_at);
    nl
}

/// An `open` newsletter whose `responseCloseAt` has already passed — the
/// ≤5-minute window before `lambda-cycle-tick` next runs and flips it to
/// `published` (`03-api-contract.md` §7.3).
pub fn open_newsletter_past_deadline(group_id: &str, cycle_id: &str) -> Newsletter {
    let now = Utc::now();
    let mut nl = voting_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Open;
    nl.response_open_at = now - Duration::days(5);
    nl.response_close_at = now - Duration::hours(1);
    nl.next_transition_at = Some(nl.response_close_at);
    nl
}

/// A `published` newsletter — writes to it must be refused with `CYCLE_NOT_OPEN`.
pub fn published_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = open_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Published;
    nl.published_at = Some(Utc::now());
    nl
}

/// An `archived` newsletter — every §7 route must refuse it with `NEWSLETTER_ARCHIVED`.
pub fn archived_newsletter(group_id: &str, cycle_id: &str) -> Newsletter {
    let mut nl = open_newsletter(group_id, cycle_id);
    nl.status = NewsletterStatus::Archived;
    nl
}

pub fn poll_locked_question(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    submitter: &str,
    option_ids: &[&str],
) -> LockedQuestion {
    LockedQuestion {
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Poll,
        prompt: "Which trailhead should we carpool from?".into(),
        poll_options: Some(
            option_ids
                .iter()
                .map(|id| PollOption {
                    option_id: PollOptionId::new(*id),
                    label: format!("Option {id}"),
                })
                .collect(),
        ),
        display_order: 0,
        submitted_by: UserId::new(submitter),
        is_anonymous: false,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    }
}

/// Writes a `LockedQuestion` row directly — there is no `promote_candidates_tx`
/// shortcut for tests that just need a question locked into a cycle without
/// simulating the full voting-to-open promotion.
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

/// Writes an `ImageMedia` row via the production `media::put_image` writer, so
/// tests can control ownership/status/purpose/scope independently to exercise
/// each image-validation failure mode.
#[allow(clippy::too_many_arguments)]
pub async fn seed_image(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
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
        question_id: None,
        purpose,
        mime_type: ImageMimeType::Jpeg,
        original_key: format!("uploads/{group_id}/{cycle_id}/{image_id}.jpg"),
        display_key: Some(format!("processed/{image_id}/display.webp")),
        thumb_key: Some(format!("processed/{image_id}/thumb.webp")),
        status,
        bytes: 123_456,
        width: Some(1200),
        height: Some(800),
        caption: None,
        uploaded_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
        processed_at: Some(Utc.with_ymd_and_hms(2026, 6, 2, 0, 1, 0).unwrap()),
    };
    media::put_image(repo, &img).await.expect("image written");
    img.image_id
}

pub async fn seed_ready_response_image(
    repo: &Repo,
    group_id: &str,
    cycle_id: &str,
    image_id: &str,
    user_id: &str,
) -> ImageId {
    seed_image(
        repo,
        group_id,
        cycle_id,
        image_id,
        user_id,
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await
}
