//! Comments and reactions (AP18, AP19; `docs/design/api.md` §8).

use crate::error::{self, RepoError};
use crate::keys::{
    attr, comment_sk, engagement_pk, reacted_gsi2pk, reacted_gsi2sk, reaction_sk, response_pk,
    response_sk, COMMENT_SK_PREFIX, REACTION_SK_PREFIX,
};
use crate::media;
use crate::member_stats;
use crate::query::query_all;
use crate::repo::Repo;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::types::builders::UpdateBuilder;
use aws_sdk_dynamodb::types::{AttributeValue, Delete, Put, TransactWriteItem, Update};
use domain::{
    AudioId, Comment, CommentId, CycleId, GroupId, ImageId, QuestionId, Reaction, ResponseId,
    Timestamp, UserId,
};
use serde_dynamo::{from_item, to_item};
use shared::config::COMMENT_WRITE_ATTEMPTS;
use std::collections::HashMap;

/// AP18 — every comment on an answer, oldest first. Unpaginated; used by the
/// inline published-newsletter view (`docs/design/api.md` §5.2), which always wants the
/// whole thread. The dedicated `GET .../comments` route uses
/// [`list_comments_page`] instead.
pub async fn list_comments(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> Result<Vec<Comment>, RepoError> {
    let request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(engagement_pk(
                group_id,
                cycle_id,
                question_id,
                answer_user_id,
            )),
        )
        .expression_attribute_values(":prefix", AttributeValue::S(COMMENT_SK_PREFIX.into()));
    query_all(request)
        .await?
        .into_iter()
        .map(|i| from_item::<_, Comment>(i).map_err(RepoError::from))
        .collect()
}

/// Unconditioned direct write of a whole `Comment` row, bypassing the
/// create/claim transaction. Used by test fixtures that need a comment row
/// in place without exercising `create_comment`'s image-attach semantics;
/// production code addressing a *new* comment should use [`create_comment`]
/// instead, so an attached image gets claimed.
pub async fn put_comment(repo: &Repo, c: &Comment) -> Result<(), RepoError> {
    let mut item: HashMap<String, AttributeValue> = to_item(c)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(engagement_pk(
            &c.group_id,
            &c.cycle_id,
            &c.question_id,
            &c.answer_user_id,
        )),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(comment_sk(c.created_at, &c.comment_id)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Comment".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// `GET .../comments` (§8.1) — one page, oldest first.
pub async fn list_comments_page(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    limit: i32,
    exclusive_start_key: Option<HashMap<String, AttributeValue>>,
) -> Result<(Vec<Comment>, Option<HashMap<String, AttributeValue>>), RepoError> {
    let mut request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(engagement_pk(
                group_id,
                cycle_id,
                question_id,
                answer_user_id,
            )),
        )
        .expression_attribute_values(":prefix", AttributeValue::S(COMMENT_SK_PREFIX.into()))
        .limit(limit);
    if let Some(key) = exclusive_start_key {
        request = request.set_exclusive_start_key(Some(key));
    }

    let resp = request.send().await?;
    let items = resp.items.unwrap_or_default();
    let comments = items
        .into_iter()
        .map(|i| from_item::<_, Comment>(i).map_err(RepoError::from))
        .collect::<Result<Vec<_>, _>>()?;
    let next = resp.last_evaluated_key.filter(|k| !k.is_empty());
    Ok((comments, next))
}

/// Finds one comment by id (decision M10, `docs/design/engagement.md` §1.4): the
/// engagement partition has no index on `comment_id`, so this walks the `C#`
/// range, following `LastEvaluatedKey`, filtering server-side. Thread sizes
/// are expected to stay small (`docs/design/engagement.md` §7 — unbounded but
/// activity-driven), so a full scan of the partition is acceptable.
///
/// Strongly consistent: the edit and delete handlers re-read through this
/// after their transaction loses a race, and a stale row would have them retry
/// against state that's already gone (a delete would never see the concurrent
/// delete that beat it, and would 500 instead of the idempotent 204).
pub async fn get_comment_by_id(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    comment_id: &CommentId,
) -> Result<Option<Comment>, RepoError> {
    let mut exclusive_start_key: Option<HashMap<String, AttributeValue>> = None;
    loop {
        let mut request = repo
            .client
            .query()
            .table_name(&repo.table)
            .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
            .filter_expression("comment_id = :cid")
            .consistent_read(true)
            .expression_attribute_names("#pk", attr::PK)
            .expression_attribute_names("#sk", attr::SK)
            .expression_attribute_values(
                ":pk",
                AttributeValue::S(engagement_pk(
                    group_id,
                    cycle_id,
                    question_id,
                    answer_user_id,
                )),
            )
            .expression_attribute_values(":prefix", AttributeValue::S(COMMENT_SK_PREFIX.into()))
            .expression_attribute_values(":cid", AttributeValue::S(comment_id.to_string()));
        if let Some(key) = exclusive_start_key.take() {
            request = request.set_exclusive_start_key(Some(key));
        }

        let resp = request.send().await?;
        if let Some(item) = resp.items.unwrap_or_default().into_iter().next() {
            return Ok(Some(from_item(item)?));
        }

        match resp.last_evaluated_key {
            Some(key) if !key.is_empty() => exclusive_start_key = Some(key),
            _ => return Ok(None),
        }
    }
}

/// A new comment's content, independent of how it's addressed
/// (`docs/design/api.md` §8.2).
pub struct NewComment<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub question_id: &'a QuestionId,
    pub answer_user_id: &'a UserId,
    pub comment_id: &'a CommentId,
    pub author_user_id: &'a UserId,
    pub body: &'a str,
    pub image_media_id: Option<&'a ImageId>,
    /// At most one voice clip (`docs/design/media.md` §12).
    pub audio_media_id: Option<&'a AudioId>,
    pub created_at: Timestamp,
}

/// `POST .../comments` (§8.2) — puts the comment row, bumps the author's
/// `comments_written` (§2.17, §4 #9) and, if an image and/or voice clip are
/// attached, claims each, all in the same `TransactWriteItems` (the image
/// claim decided M10; the clip claim mirrors it). Only the attachment claims
/// carry a condition, so `RepoError::TransactionCancelled` means one of them
/// lost a race to another comment — the caller maps that to 422 on
/// `imageMediaId`/`audioMediaId`. A `TransactionConflict` with a concurrent
/// writer (most likely another write to the author's `MemberStats` row)
/// applied nothing, so it is retried here, up to `COMMENT_WRITE_ATTEMPTS`
/// attempts in all.
pub async fn create_comment(repo: &Repo, new: &NewComment<'_>) -> Result<Comment, RepoError> {
    let comment = Comment {
        comment_id: new.comment_id.clone(),
        group_id: new.group_id.clone(),
        cycle_id: new.cycle_id.clone(),
        question_id: new.question_id.clone(),
        answer_user_id: new.answer_user_id.clone(),
        author_user_id: new.author_user_id.clone(),
        body: new.body.to_owned(),
        image_media_id: new.image_media_id.cloned(),
        audio_media_id: new.audio_media_id.cloned(),
        created_at: new.created_at,
        edited_at: None,
        deleted_at: None,
    };

    let mut item: HashMap<String, AttributeValue> = to_item(&comment)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(engagement_pk(
            new.group_id,
            new.cycle_id,
            new.question_id,
            new.answer_user_id,
        )),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(comment_sk(new.created_at, new.comment_id)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Comment".into()));

    let put = Put::builder()
        .table_name(&repo.table)
        .set_item(Some(item))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
    let bump = member_stats::add_update(
        repo,
        new.group_id,
        new.author_user_id,
        "comments_written",
        1,
    )?;
    let mut items = vec![
        TransactWriteItem::builder().put(put).build(),
        TransactWriteItem::builder().update(bump).build(),
    ];

    if let Some(image_id) = new.image_media_id {
        let attach = media::attach_image_to_comment_update(
            repo,
            new.group_id,
            new.cycle_id,
            image_id,
            new.comment_id,
        )?;
        items.push(TransactWriteItem::builder().update(attach).build());
    }
    if let Some(audio_id) = new.audio_media_id {
        let attach = media::attach_audio_to_comment_update(
            repo,
            new.group_id,
            new.cycle_id,
            audio_id,
            new.comment_id,
        )?;
        items.push(TransactWriteItem::builder().update(attach).build());
    }
    // Items: the comment put and the stats bump (no conditions, positions 0-1),
    // then the image claim and/or the audio claim, whichever are present, in
    // that order.
    let has_image = new.image_media_id.is_some();
    let has_audio = new.audio_media_id.is_some();

    for attempt in 1..=COMMENT_WRITE_ATTEMPTS {
        let sent = repo
            .client
            .transact_write_items()
            .set_transact_items(Some(items.clone()))
            .send()
            .await;
        let Err(err) = sent else {
            return Ok(comment);
        };
        let outcome = match (has_image, has_audio) {
            (false, false) => error::classify_transact_write_error(
                err,
                [CommentTxError::Unexpected, CommentTxError::Unexpected],
                CommentTxError::Conflict,
                CommentTxError::Repo,
            ),
            (true, false) => error::classify_transact_write_error(
                err,
                [
                    CommentTxError::Unexpected,
                    CommentTxError::Unexpected,
                    CommentTxError::ImageLost,
                ],
                CommentTxError::Conflict,
                CommentTxError::Repo,
            ),
            (false, true) => error::classify_transact_write_error(
                err,
                [
                    CommentTxError::Unexpected,
                    CommentTxError::Unexpected,
                    CommentTxError::AudioLost,
                ],
                CommentTxError::Conflict,
                CommentTxError::Repo,
            ),
            (true, true) => error::classify_transact_write_error(
                err,
                [
                    CommentTxError::Unexpected,
                    CommentTxError::Unexpected,
                    CommentTxError::ImageLost,
                    CommentTxError::AudioLost,
                ],
                CommentTxError::Conflict,
                CommentTxError::Repo,
            ),
        };
        match outcome {
            CommentTxError::Conflict if attempt < COMMENT_WRITE_ATTEMPTS => {}
            CommentTxError::Conflict => {
                return Err(RepoError::TransactionConflict(format!(
                    "comment create, after {COMMENT_WRITE_ATTEMPTS} attempts"
                )))
            }
            CommentTxError::ImageLost => {
                return Err(RepoError::TransactionCancelled("image claim lost".into()))
            }
            CommentTxError::AudioLost => {
                return Err(RepoError::TransactionCancelled("audio claim lost".into()))
            }
            CommentTxError::Unexpected => {
                return Err(RepoError::Dynamo(
                    "comment create failed an unconditioned item's condition".into(),
                ))
            }
            CommentTxError::Repo(e) => return Err(e),
        }
    }
    unreachable!("the loop above always returns within COMMENT_WRITE_ATTEMPTS attempts")
}

/// [`create_comment`]'s reading of its transaction's cancellation reasons.
enum CommentTxError {
    ImageLost,
    AudioLost,
    Conflict,
    /// A condition failed on an item that has none; can't happen.
    Unexpected,
    Repo(RepoError),
}

/// Addresses one existing comment row — shared by [`update_comment`] and
/// [`soft_delete_comment`].
pub struct CommentLocation<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub question_id: &'a QuestionId,
    pub answer_user_id: &'a UserId,
    pub comment_id: &'a CommentId,
    pub created_at: Timestamp,
}

fn comment_update_builder(repo: &Repo, loc: &CommentLocation<'_>) -> UpdateBuilder {
    Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(engagement_pk(
                loc.group_id,
                loc.cycle_id,
                loc.question_id,
                loc.answer_user_id,
            )),
        )
        .key(
            attr::SK,
            AttributeValue::S(comment_sk(loc.created_at, loc.comment_id)),
        )
}

/// Condition (plus its placeholder value) that the comment row still holds
/// `expected` as its image. Both comment writes release the image they read
/// earlier with an unconditioned detach, so without this a write built on a
/// stale read would release an image the comment no longer holds — and
/// leave the one it does hold claimed by a `[deleted]` row forever.
/// `image_media_id` is stored as an explicit NULL by `to_item` when absent,
/// hence both forms for `None`.
fn image_unchanged_condition(expected: Option<&ImageId>) -> (&'static str, AttributeValue) {
    match expected {
        Some(id) => (
            "image_media_id = :expected_image",
            AttributeValue::S(id.to_string()),
        ),
        None => (
            "(attribute_not_exists(image_media_id) OR image_media_id = :expected_image)",
            AttributeValue::Null(true),
        ),
    }
}

/// Mirrors [`image_unchanged_condition`] for `audio_media_id`.
fn audio_unchanged_condition(expected: Option<&AudioId>) -> (&'static str, AttributeValue) {
    match expected {
        Some(id) => (
            "audio_media_id = :expected_audio",
            AttributeValue::S(id.to_string()),
        ),
        None => (
            "(attribute_not_exists(audio_media_id) OR audio_media_id = :expected_audio)",
            AttributeValue::Null(true),
        ),
    }
}

/// `PATCH .../comments/{commentId}` (§8.3). `body` absent leaves it
/// unchanged. `current_image_id` is the image the handler read on the
/// comment; the write only applies if that's still the case.
/// `detach_image_id`/`attach_image_id` are resolved by the handler (which
/// already knows the comment's previous image and the validated new one) —
/// this just builds the transaction: the comment update, plus a release on
/// the old image and/or a conditioned claim on the new one, whichever apply.
/// A cancelled transaction means the new image lost a race to another
/// comment, the comment was soft-deleted concurrently, or its image changed
/// since the handler read it; the handler re-reads the comment to tell which.
#[allow(clippy::too_many_arguments)]
pub async fn update_comment(
    repo: &Repo,
    loc: &CommentLocation<'_>,
    body: Option<&str>,
    set_image_media_id: Option<Option<&ImageId>>,
    set_audio_media_id: Option<Option<&AudioId>>,
    edited_at: Timestamp,
    current_image_id: Option<&ImageId>,
    detach_image_id: Option<&ImageId>,
    attach_image_id: Option<&ImageId>,
    current_audio_id: Option<&AudioId>,
    detach_audio_id: Option<&AudioId>,
    attach_audio_id: Option<&AudioId>,
) -> Result<(), RepoError> {
    let mut set_clauses = vec!["edited_at = :edited_at".to_owned()];
    let mut remove_clauses: Vec<String> = Vec::new();
    let mut builder = comment_update_builder(repo, loc)
        .expression_attribute_values(":edited_at", AttributeValue::S(edited_at.to_string()));

    if let Some(body) = body {
        set_clauses.push("body = :body".to_owned());
        builder = builder.expression_attribute_values(":body", AttributeValue::S(body.to_owned()));
    }
    match set_image_media_id {
        Some(Some(image_id)) => {
            set_clauses.push("image_media_id = :image_media_id".to_owned());
            builder = builder.expression_attribute_values(
                ":image_media_id",
                AttributeValue::S(image_id.to_string()),
            );
        }
        Some(None) => remove_clauses.push("image_media_id".to_owned()),
        None => {}
    }
    match set_audio_media_id {
        Some(Some(audio_id)) => {
            set_clauses.push("audio_media_id = :audio_media_id".to_owned());
            builder = builder.expression_attribute_values(
                ":audio_media_id",
                AttributeValue::S(audio_id.to_string()),
            );
        }
        Some(None) => remove_clauses.push("audio_media_id".to_owned()),
        None => {}
    }

    let mut expression = format!("SET {}", set_clauses.join(", "));
    if !remove_clauses.is_empty() {
        expression.push_str(&format!(" REMOVE {}", remove_clauses.join(", ")));
    }
    // A concurrent soft delete must win: without this, an edit landing just
    // after it would write a body back into the `[deleted]` row. `deleted_at`
    // is stored as an explicit NULL by `to_item`, hence both forms.
    let (image_condition, expected_image) = image_unchanged_condition(current_image_id);
    let (audio_condition, expected_audio) = audio_unchanged_condition(current_audio_id);
    let comment_update = builder
        .update_expression(expression)
        .condition_expression(format!(
            "attribute_exists(#pk) AND (attribute_not_exists(deleted_at) OR deleted_at = :null) \
             AND {image_condition} AND {audio_condition}"
        ))
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(":null", AttributeValue::Null(true))
        .expression_attribute_values(":expected_image", expected_image)
        .expression_attribute_values(":expected_audio", expected_audio)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let mut items = vec![TransactWriteItem::builder().update(comment_update).build()];
    if let Some(image_id) = detach_image_id {
        let detach =
            media::detach_image_from_comment_update(repo, loc.group_id, loc.cycle_id, image_id)?;
        items.push(TransactWriteItem::builder().update(detach).build());
    }
    if let Some(image_id) = attach_image_id {
        let attach = media::attach_image_to_comment_update(
            repo,
            loc.group_id,
            loc.cycle_id,
            image_id,
            loc.comment_id,
        )?;
        items.push(TransactWriteItem::builder().update(attach).build());
    }
    if let Some(audio_id) = detach_audio_id {
        let detach =
            media::detach_audio_from_comment_update(repo, loc.group_id, loc.cycle_id, audio_id)?;
        items.push(TransactWriteItem::builder().update(detach).build());
    }
    if let Some(audio_id) = attach_audio_id {
        let attach = media::attach_audio_to_comment_update(
            repo,
            loc.group_id,
            loc.cycle_id,
            audio_id,
            loc.comment_id,
        )?;
        items.push(TransactWriteItem::builder().update(attach).build());
    }

    send_transaction(repo, items).await
}

/// `DELETE .../comments/{commentId}` (§8.4) — soft delete. `image_id` is the
/// image the handler read on the comment (if any), released in the same
/// transaction. The delete only applies while the comment still holds
/// exactly that image AND isn't already deleted — the latter condition is
/// what makes a repeated delete decrement `comments_written` (§2.17, §4 #9)
/// at most once, even if two deletes of the same comment land concurrently. A
/// cancelled transaction means an edit changed the image, the comment was
/// already deleted, or another delete just landed; the handler should
/// re-read and decide which (its own idempotent-204 check, re-run against
/// the fresh row, tells it apart from a genuine image race).
pub async fn soft_delete_comment(
    repo: &Repo,
    loc: &CommentLocation<'_>,
    author_user_id: &UserId,
    deleted_at: Timestamp,
    image_id: Option<&ImageId>,
    audio_id: Option<&AudioId>,
) -> Result<(), RepoError> {
    let (image_condition, expected_image) = image_unchanged_condition(image_id);
    let (audio_condition, expected_audio) = audio_unchanged_condition(audio_id);
    let comment_update = comment_update_builder(repo, loc)
        .update_expression(
            "SET deleted_at = :t, body = :empty REMOVE image_media_id, audio_media_id",
        )
        .condition_expression(format!(
            "attribute_exists(#pk) AND (attribute_not_exists(deleted_at) OR deleted_at = :null) \
             AND {image_condition} AND {audio_condition}"
        ))
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(":t", AttributeValue::S(deleted_at.to_string()))
        .expression_attribute_values(":empty", AttributeValue::S("".into()))
        .expression_attribute_values(":null", AttributeValue::Null(true))
        .expression_attribute_values(":expected_image", expected_image)
        .expression_attribute_values(":expected_audio", expected_audio)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
    let bump =
        member_stats::add_update(repo, loc.group_id, author_user_id, "comments_written", -1)?;

    let mut items = vec![
        TransactWriteItem::builder().update(comment_update).build(),
        TransactWriteItem::builder().update(bump).build(),
    ];
    if let Some(image_id) = image_id {
        let detach =
            media::detach_image_from_comment_update(repo, loc.group_id, loc.cycle_id, image_id)?;
        items.push(TransactWriteItem::builder().update(detach).build());
    }
    if let Some(audio_id) = audio_id {
        let detach =
            media::detach_audio_from_comment_update(repo, loc.group_id, loc.cycle_id, audio_id)?;
        items.push(TransactWriteItem::builder().update(detach).build());
    }

    send_transaction(repo, items).await
}

async fn send_transaction(repo: &Repo, items: Vec<TransactWriteItem>) -> Result<(), RepoError> {
    let mut req = repo.client.transact_write_items();
    for item in items {
        req = req.transact_items(item);
    }
    req.send().await.map_err(error::from_transact_write_error)?;
    Ok(())
}

/// AP19 — reactions on an answer.
pub async fn list_reactions(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> Result<Vec<Reaction>, RepoError> {
    let request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(engagement_pk(
                group_id,
                cycle_id,
                question_id,
                answer_user_id,
            )),
        )
        .expression_attribute_values(":prefix", AttributeValue::S(REACTION_SK_PREFIX.into()));
    query_all(request)
        .await?
        .into_iter()
        .map(|i| from_item::<_, Reaction>(i).map_err(RepoError::from))
        .collect()
}

/// One paginated query of the engagement partition shared by a question's
/// comments (`C#`) and reactions (`R#`) — see [`engagement_pk`]. Replaces a
/// separate [`list_comments`] + [`list_reactions`] (two round trips) with
/// one; used by `lambda-newsletters`' published-edition read. Rows sort `C#`
/// before `R#` lexicographically, so comments come back oldest-first exactly
/// as [`list_comments`] returns them. A row whose sk starts with neither
/// prefix is skipped defensively — nothing else is known to live in this
/// partition today (see `keys.rs`).
pub async fn list_engagement(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> Result<(Vec<Comment>, Vec<Reaction>), RepoError> {
    let request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(engagement_pk(
                group_id,
                cycle_id,
                question_id,
                answer_user_id,
            )),
        );

    let mut comments = Vec::new();
    let mut reactions = Vec::new();
    for item in query_all(request).await? {
        match item.get(attr::SK).and_then(|v| v.as_s().ok()) {
            Some(sk) if sk.starts_with(COMMENT_SK_PREFIX) => {
                comments.push(from_item::<_, Comment>(item)?);
            }
            Some(sk) if sk.starts_with(REACTION_SK_PREFIX) => {
                reactions.push(from_item::<_, Reaction>(item)?);
            }
            _ => {}
        }
    }
    Ok((comments, reactions))
}

/// Plain, uncounted Put of a reaction row — used only for a **self-reaction**
/// (reactor == answer author), which counts nowhere (`docs/design/data-model.md`
/// §4 #8). Every other reaction goes through [`put_reaction_counted`].
pub async fn put_reaction(repo: &Repo, r: &Reaction) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(r)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(engagement_pk(
            &r.group_id,
            &r.cycle_id,
            &r.question_id,
            &r.answer_user_id,
        )),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(reaction_sk(&r.reactor_user_id, &r.emoji)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Reaction".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// Plain, uncounted Delete of a reaction row — the self-reaction counterpart
/// of [`put_reaction`]. Every other reaction removal goes through
/// [`delete_reaction_counted`].
pub async fn delete_reaction(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    reactor_user_id: &UserId,
    emoji: &str,
) -> Result<(), RepoError> {
    repo.client
        .delete_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(engagement_pk(
                group_id,
                cycle_id,
                question_id,
                answer_user_id,
            )),
        )
        .key(
            attr::SK,
            AttributeValue::S(reaction_sk(reactor_user_id, emoji)),
        )
        .send()
        .await?;
    Ok(())
}

/// Why a counted-reaction transaction (§4 #8) was cancelled. Mirrors
/// `questions::VoteTxError`.
#[derive(Debug)]
pub enum ReactionTxError {
    /// The `Reaction` row's condition failed: a duplicate PUT, or a DELETE of
    /// a reaction that isn't there. Both are idempotent successes.
    Duplicate,
    /// The answer's `reaction_count` condition failed: lost race against
    /// another counted write to the same answer. Re-read the answer and retry
    /// with the fresh count.
    CountRace,
    /// Only `TransactionConflict`s, no condition failure: safe to retry
    /// unchanged.
    Conflict,
    Repo(RepoError),
}

/// Builds the Response-row `Update` shared by [`put_reaction_counted`] and
/// [`delete_reaction_counted`]: sets `reaction_count` to `new_count` and
/// rewrites the GSI2 most-reacted ranking (§2.9), conditioned on the row
/// existing and its prior count being `old_count` (or absent, when
/// `old_count == 0`). Without the existence check, an answer deleted since the
/// caller read it would come back as a keys-only row in the GSI2 ranking.
#[allow(clippy::too_many_arguments)]
fn reaction_count_update(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    response_id: &ResponseId,
    old_count: u32,
    new_count: u32,
) -> Result<Update, RepoError> {
    Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(group_id, cycle_id, question_id)),
        )
        .key(attr::SK, AttributeValue::S(response_sk(answer_user_id)))
        .update_expression("SET reaction_count = :new, #g2pk = :g2pk, #g2sk = :g2sk")
        .condition_expression(
            "attribute_exists(#pk) AND \
             (attribute_not_exists(reaction_count) OR reaction_count = :old)",
        )
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#g2pk", attr::GSI2PK)
        .expression_attribute_names("#g2sk", attr::GSI2SK)
        .expression_attribute_values(":new", AttributeValue::N(new_count.to_string()))
        .expression_attribute_values(":old", AttributeValue::N(old_count.to_string()))
        .expression_attribute_values(":g2pk", AttributeValue::S(reacted_gsi2pk(group_id)))
        .expression_attribute_values(
            ":g2sk",
            AttributeValue::S(reacted_gsi2sk(new_count, response_id)),
        )
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// Transaction §4 #8, add — another member's reaction on a published text
/// answer:
/// 1. Put `Reaction`, conditioned on `attribute_not_exists(sk)`.
/// 2. Update the Response row: `reaction_count = old_count + 1` plus the GSI2
///    rewrite, conditioned on the row's prior count being `old_count`.
/// 3. `ADD reactions_received 1` on the author's `MemberStats`.
/// 4. `ADD reactions_given 1` on the reactor's `MemberStats`.
///
/// `old_count` is the caller's last-known `reaction_count` for the answer —
/// on [`ReactionTxError::CountRace`], the caller re-reads the answer and
/// retries with its fresh count (`docs/design/data-model.md` §4 #8; 3
/// attempts total, mirroring the vote transaction).
pub async fn put_reaction_counted(
    repo: &Repo,
    reaction: &Reaction,
    response_id: &ResponseId,
    old_count: u32,
) -> Result<(), ReactionTxError> {
    let mut item: HashMap<String, AttributeValue> =
        to_item(reaction).map_err(|e| ReactionTxError::Repo(e.into()))?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(engagement_pk(
            &reaction.group_id,
            &reaction.cycle_id,
            &reaction.question_id,
            &reaction.answer_user_id,
        )),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(reaction_sk(&reaction.reactor_user_id, &reaction.emoji)),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("Reaction".into()));

    let put_reaction = Put::builder()
        .table_name(&repo.table)
        .set_item(Some(item))
        .condition_expression("attribute_not_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| ReactionTxError::Repo(RepoError::Dynamo(format!("{e:?}"))))?;

    let count_update = reaction_count_update(
        repo,
        &reaction.group_id,
        &reaction.cycle_id,
        &reaction.question_id,
        &reaction.answer_user_id,
        response_id,
        old_count,
        old_count + 1,
    )
    .map_err(ReactionTxError::Repo)?;

    let author_bump = member_stats::add_update(
        repo,
        &reaction.group_id,
        &reaction.answer_user_id,
        "reactions_received",
        1,
    )
    .map_err(ReactionTxError::Repo)?;
    let reactor_bump = member_stats::add_update(
        repo,
        &reaction.group_id,
        &reaction.reactor_user_id,
        "reactions_given",
        1,
    )
    .map_err(ReactionTxError::Repo)?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().put(put_reaction).build())
        .transact_items(TransactWriteItem::builder().update(count_update).build())
        .transact_items(TransactWriteItem::builder().update(author_bump).build())
        .transact_items(TransactWriteItem::builder().update(reactor_bump).build())
        .send()
        .await
        .map_err(classify_reaction_tx_error)?;
    Ok(())
}

/// Transaction §4 #8, remove — the counterpart of [`put_reaction_counted`]:
/// Delete `Reaction` (conditioned on `attribute_exists(sk)`), decrement the
/// Response row's `reaction_count` (never below 0) with the same GSI2
/// rewrite, and `ADD -1` to both members' `MemberStats`. `old_count` is the
/// caller's last-known count, used the same way as in `put_reaction_counted`.
pub async fn delete_reaction_counted(
    repo: &Repo,
    reaction: &Reaction,
    response_id: &ResponseId,
    old_count: u32,
) -> Result<(), ReactionTxError> {
    let delete = Delete::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(engagement_pk(
                &reaction.group_id,
                &reaction.cycle_id,
                &reaction.question_id,
                &reaction.answer_user_id,
            )),
        )
        .key(
            attr::SK,
            AttributeValue::S(reaction_sk(&reaction.reactor_user_id, &reaction.emoji)),
        )
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| ReactionTxError::Repo(RepoError::Dynamo(format!("{e:?}"))))?;

    let new_count = old_count.saturating_sub(1);
    let count_update = reaction_count_update(
        repo,
        &reaction.group_id,
        &reaction.cycle_id,
        &reaction.question_id,
        &reaction.answer_user_id,
        response_id,
        old_count,
        new_count,
    )
    .map_err(ReactionTxError::Repo)?;

    let author_bump = member_stats::add_update(
        repo,
        &reaction.group_id,
        &reaction.answer_user_id,
        "reactions_received",
        -1,
    )
    .map_err(ReactionTxError::Repo)?;
    let reactor_bump = member_stats::add_update(
        repo,
        &reaction.group_id,
        &reaction.reactor_user_id,
        "reactions_given",
        -1,
    )
    .map_err(ReactionTxError::Repo)?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(count_update).build())
        .transact_items(TransactWriteItem::builder().update(author_bump).build())
        .transact_items(TransactWriteItem::builder().update(reactor_bump).build())
        .send()
        .await
        .map_err(classify_reaction_tx_error)?;
    Ok(())
}

/// Map a counted-reaction transaction's failure to the item whose condition
/// failed: the reaction row (item 0) is a [`ReactionTxError::Duplicate`], the
/// count update (item 1) a [`ReactionTxError::CountRace`]. The two
/// `MemberStats` bumps (items 2, 3) carry no condition, but they can still
/// conflict with a concurrent writer, which is a [`ReactionTxError::Conflict`].
fn classify_reaction_tx_error<R: std::fmt::Debug>(
    err: SdkError<TransactWriteItemsError, R>,
) -> ReactionTxError {
    error::classify_transact_write_error(
        err,
        [ReactionTxError::Duplicate, ReactionTxError::CountRace],
        ReactionTxError::Conflict,
        ReactionTxError::Repo,
    )
}
