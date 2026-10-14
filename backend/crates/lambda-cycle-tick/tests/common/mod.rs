//! Shared DynamoDB Local setup for `lambda-cycle-tick` integration tests.
#![allow(dead_code, unused_imports)]

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{DateTime, Duration, Utc};
use domain::{
    CycleId, Group, GroupId, Newsletter, NewsletterStatus, PollOptionId, QuestionId, QuestionKind,
    Role, UserId,
};
use persistence::keys::{
    attr, membership_gsi1pk, membership_gsi1sk, membership_sk, response_pk, response_sk, user_pk,
};
use persistence::responses::{self, ResponseSave};
use persistence::{groups, media, newsletters, test_factories, Repo};

pub use test_factories::{candidate, group, make_test_repo, membership};

pub async fn put_membership(repo: &Repo, user_id: &str, group_id: &str) {
    let m = membership(user_id, group_id, Role::Member);
    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(&m).unwrap();
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

/// A group, and an `open` cycle with `locked` questions that closes in five
/// days, dated from the real clock (the tick reads it).
pub async fn seed_open_cycle(repo: &Repo, group_id: &str, locked: &[&str]) -> Newsletter {
    let group: Group = group(group_id, "u1");
    groups::put_group(repo, &group).await.unwrap();
    let now = Utc::now();
    let tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let nl = Newsletter {
        group_id: GroupId::new(group_id),
        cycle_id: shared::cycle_time::cycle_id_for(now, tz),
        status: NewsletterStatus::Open,
        vote_window_open_at: now - Duration::days(30),
        vote_window_close_at: now - Duration::days(1),
        response_open_at: now - Duration::days(1),
        response_close_at: now + Duration::days(5),
        published_at: None,
        next_transition_at: Some(now + Duration::days(5)),
        locked_question_ids: locked.iter().map(|q| QuestionId::new(*q)).collect(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
    };
    newsletters::write_status_transition(repo, &nl)
        .await
        .unwrap();
    nl
}

/// What one member saved for one question.
pub enum Answer<'a> {
    Text {
        body: &'a str,
        images: &'a [&'a str],
    },
    Poll(&'a str),
}

/// Save an answer through the real save transaction, as `PUT my-response`
/// does: a draft, or a publish with the first-publish bump.
pub async fn save(
    repo: &Repo,
    nl: &Newsletter,
    user_id: &str,
    question_id: &str,
    answer: Answer<'_>,
    publish: bool,
) {
    let user = UserId::new(user_id);
    let question = QuestionId::new(question_id);
    let (kind, body, poll, images): (_, _, _, Vec<domain::ImageId>) = match answer {
        Answer::Text { body, images } => (
            QuestionKind::Text,
            Some(body),
            None,
            images.iter().map(|i| domain::ImageId::new(*i)).collect(),
        ),
        Answer::Poll(option) => (
            QuestionKind::Poll,
            None,
            Some(PollOptionId::new(option)),
            Vec::new(),
        ),
    };
    let save = ResponseSave {
        group_id: &nl.group_id,
        cycle_id: &nl.cycle_id,
        question_id: &question,
        user_id: &user,
        kind,
        body,
        poll_option_id: poll.as_ref(),
        image_media_ids: &images,
        publish,
    };
    responses::save_response(repo, &save, Utc::now(), publish)
        .await
        .unwrap();
}

/// A `ready` response image owned by `user_id`, so a draft listing it has
/// live content at close.
pub async fn put_ready_image(
    repo: &Repo,
    nl: &Newsletter,
    user_id: &str,
    question_id: &str,
    image_id: &str,
) {
    let prefix = format!(
        "{}/{}/{question_id}/{user_id}/{image_id}",
        nl.group_id, nl.cycle_id
    );
    let image = domain::ImageMedia {
        image_id: domain::ImageId::new(image_id),
        user_id: UserId::new(user_id),
        group_id: nl.group_id.clone(),
        cycle_id: nl.cycle_id.clone(),
        question_id: Some(QuestionId::new(question_id)),
        purpose: domain::ImagePurpose::Response,
        mime_type: domain::ImageMimeType::Jpeg,
        original_key: format!("uploads/{prefix}.jpg"),
        display_key: Some(format!("img/{prefix}/display.webp")),
        thumb_key: Some(format!("img/{prefix}/thumb.webp")),
        status: domain::MediaStatus::Ready,
        bytes: 1024,
        width: Some(100),
        height: Some(100),
        caption: None,
        uploaded_at: Utc::now(),
        processed_at: Some(Utc::now()),
        error_message: None,
        attached_comment_id: None,
    };
    media::put_image(repo, &image).await.unwrap();
}

/// An `A#` row under `question_id` that won't deserialize as a `Response`:
/// corrupt data the auto-publish sweep must survive.
pub async fn put_unreadable_answer(repo: &Repo, nl: &Newsletter, question_id: &str, user_id: &str) {
    repo.client
        .put_item()
        .table_name(&repo.table)
        .item(
            attr::PK,
            AttributeValue::S(response_pk(
                &nl.group_id,
                &nl.cycle_id,
                &QuestionId::new(question_id),
            )),
        )
        .item(
            attr::SK,
            AttributeValue::S(response_sk(&UserId::new(user_id))),
        )
        .item("status", AttributeValue::S("draft".into()))
        .send()
        .await
        .unwrap();
}

/// Move the open cycle's close into the past, as the dev fast-forward does,
/// and return the row as the tick will now see it.
pub async fn close_cycle(repo: &Repo, nl: &Newsletter) -> Newsletter {
    newsletters::rewind_active_cycle_deadline(repo, &nl.group_id, Duration::days(6))
        .await
        .unwrap()
        .expect("the group has an open cycle")
}

pub async fn response(
    repo: &Repo,
    nl: &Newsletter,
    user_id: &str,
    question_id: &str,
) -> domain::Response {
    responses::get_my_response(
        repo,
        &nl.group_id,
        &nl.cycle_id,
        &QuestionId::new(question_id),
        &UserId::new(user_id),
    )
    .await
    .unwrap()
    .expect("response exists")
}

pub async fn editions_answered(repo: &Repo, user_id: &str, group_id: &str) -> Option<u32> {
    groups::get_membership(repo, &UserId::new(user_id), &GroupId::new(group_id))
        .await
        .unwrap()
        .map(|m| m.editions_answered)
}

pub async fn status(repo: &Repo, group_id: &GroupId, cycle_id: &CycleId) -> NewsletterStatus {
    newsletters::get_newsletter(repo, group_id, cycle_id)
        .await
        .unwrap()
        .expect("cycle exists")
        .status
}

pub fn seconds(dt: DateTime<Utc>) -> i64 {
    dt.timestamp()
}
