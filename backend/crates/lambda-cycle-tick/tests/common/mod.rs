//! Shared DynamoDB Local setup for `lambda-cycle-tick` integration tests.
#![allow(dead_code, unused_imports)]

use aws_sdk_dynamodb::types::AttributeValue;
use domain::{
    CycleId, Group, GroupId, Newsletter, NewsletterStatus, PollOptionId, QuestionId, QuestionKind,
    Role, SignedDuration, Timestamp, UserId,
};
use persistence::keys::{
    attr, membership_gsi1pk, membership_gsi1sk, membership_sk, response_pk, response_sk, user_pk,
};
use persistence::responses::{self, ResponseSave};
use persistence::{groups, media, newsletters, test_factories, Repo};
use shared::cycle_time::CycleCadence;

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

/// A group, and an `open` monthly cycle with `locked` questions that closes
/// in five days, dated from the real clock (the tick reads it).
pub async fn seed_open_cycle(repo: &Repo, group_id: &str, locked: &[&str]) -> Newsletter {
    seed_open_cycle_with_cadence(repo, group_id, locked, CycleCadence::Monthly).await
}

/// [`seed_open_cycle`], with the cycle id `cadence` gives today.
pub async fn seed_open_cycle_with_cadence(
    repo: &Repo,
    group_id: &str,
    locked: &[&str],
    cadence: CycleCadence,
) -> Newsletter {
    let group: Group = group(group_id, "u1");
    groups::put_group(repo, &group).await.unwrap();
    let now = Timestamp::now();
    let tz = shared::cycle_time::time_zone(&group.timezone).unwrap();
    let nl = Newsletter {
        group_id: GroupId::new(group_id),
        cycle_id: shared::cycle_time::cycle_id_for(cadence, now, &tz),
        status: NewsletterStatus::Open,
        vote_window_open_at: now - SignedDuration::from_hours(30 * 24),
        vote_window_close_at: now - SignedDuration::from_hours(24),
        response_open_at: now - SignedDuration::from_hours(24),
        response_close_at: now + SignedDuration::from_hours(5 * 24),
        published_at: None,
        next_transition_at: Some(now + SignedDuration::from_hours(5 * 24)),
        locked_question_ids: locked.iter().map(|q| QuestionId::new(*q)).collect(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
        questions_per_cycle: None,
        votes_per_user_per_cycle: None,
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

/// Save an answer through the real save transaction, as `PUT my-response` does.
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
    responses::save_response(repo, &save, Timestamp::now())
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
        uploaded_at: Timestamp::now(),
        processed_at: Some(Timestamp::now()),
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
    newsletters::rewind_active_cycle_deadline(
        repo,
        &nl.group_id,
        SignedDuration::from_hours(6 * 24),
    )
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

/// The member's `MemberStats.editions_answered`; 0 when they have no stats row.
pub async fn editions_answered(repo: &Repo, user_id: &str, group_id: &str) -> i64 {
    persistence::member_stats::list_stats_for_group(repo, &GroupId::new(group_id))
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.user_id == UserId::new(user_id))
        .map_or(0, |s| s.editions_answered)
}

pub async fn status(repo: &Repo, group_id: &GroupId, cycle_id: &CycleId) -> NewsletterStatus {
    newsletters::get_newsletter(repo, group_id, cycle_id)
        .await
        .unwrap()
        .expect("cycle exists")
        .status
}

pub fn seconds(ts: Timestamp) -> i64 {
    ts.as_unix_seconds()
}

/// A member's `MemberStats` row, or `None` if it has never been written.
pub async fn member_stats(
    repo: &Repo,
    group_id: &str,
    user_id: &str,
) -> Option<domain::MemberStats> {
    persistence::member_stats::list_stats_for_group(repo, &GroupId::new(group_id))
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.user_id == UserId::new(user_id))
}

/// Seeds a stats row at an already-settled streak, as if an earlier edition's
/// settlement had run — the fields `settle_streaks` itself writes. Written
/// directly, so it also works for a departed member, whom settlement refuses.
pub async fn seed_streak(
    repo: &Repo,
    group_id: &str,
    user_id: &str,
    current_streak: i64,
    longest_streak: i64,
    settled_cycle_id: &str,
) {
    test_factories::put_member_stats(
        repo,
        &domain::MemberStats {
            group_id: GroupId::new(group_id),
            user_id: UserId::new(user_id),
            editions_answered: 0,
            reactions_received: 0,
            reactions_given: 0,
            comments_written: 0,
            questions_asked: 0,
            current_streak,
            longest_streak,
            streak_cycle: Some(CycleId::new(settled_cycle_id)),
        },
    )
    .await;
}
