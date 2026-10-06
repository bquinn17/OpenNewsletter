//! Comments and reactions (AP18, AP19; `plans/03-api-contract.md` §8).

use crate::error::{self, RepoError};
use crate::keys::{attr, comment_sk, engagement_pk, reaction_sk, COMMENT_SK_PREFIX};
use crate::media;
use crate::repo::Repo;
use aws_sdk_dynamodb::types::builders::UpdateBuilder;
use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem, Update};
use chrono::{DateTime, Utc};
use domain::{Comment, CommentId, CycleId, GroupId, ImageId, QuestionId, Reaction, UserId};
use serde_dynamo::{from_item, to_item};
use std::collections::HashMap;

/// AP18 — every comment on an answer, oldest first. Unpaginated; used by the
/// inline published-newsletter view (`03` §5.2), which always wants the
/// whole thread. The dedicated `GET .../comments` route uses
/// [`list_comments_page`] instead.
pub async fn list_comments(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
) -> Result<Vec<Comment>, RepoError> {
    let resp = repo
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
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
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

/// Finds one comment by id (decision M10, `09-engagement.md` §1.4): the
/// engagement partition has no index on `comment_id`, so this walks the `C#`
/// range, following `LastEvaluatedKey`, filtering server-side. Thread sizes
/// are expected to stay small (`09-engagement.md` §7 — unbounded but
/// activity-driven), so a full scan of the partition is acceptable.
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
/// (`03-api-contract.md` §8.2).
pub struct NewComment<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub question_id: &'a QuestionId,
    pub answer_user_id: &'a UserId,
    pub comment_id: &'a CommentId,
    pub author_user_id: &'a UserId,
    pub body: &'a str,
    pub image_media_id: Option<&'a ImageId>,
    pub created_at: DateTime<Utc>,
}

/// `POST .../comments` (§8.2) — puts the comment row and, if an image is
/// attached, claims it in the same `TransactWriteItems` (decided M10). A
/// cancelled transaction (`RepoError::is_lost_race`) means the image lost a
/// race to another comment — the caller maps that to 422 on `imageMediaId`.
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
    let mut items = vec![TransactWriteItem::builder().put(put).build()];

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

    send_transaction(repo, items).await?;
    Ok(comment)
}

/// Addresses one existing comment row — shared by [`update_comment`] and
/// [`soft_delete_comment`].
pub struct CommentLocation<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub question_id: &'a QuestionId,
    pub answer_user_id: &'a UserId,
    pub comment_id: &'a CommentId,
    pub created_at: DateTime<Utc>,
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
    edited_at: DateTime<Utc>,
    current_image_id: Option<&ImageId>,
    detach_image_id: Option<&ImageId>,
    attach_image_id: Option<&ImageId>,
) -> Result<(), RepoError> {
    let mut set_clauses = vec!["edited_at = :edited_at".to_owned()];
    let mut remove_clauses: Vec<String> = Vec::new();
    let mut builder = comment_update_builder(repo, loc)
        .expression_attribute_values(":edited_at", AttributeValue::S(edited_at.to_rfc3339()));

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

    let mut expression = format!("SET {}", set_clauses.join(", "));
    if !remove_clauses.is_empty() {
        expression.push_str(&format!(" REMOVE {}", remove_clauses.join(", ")));
    }
    // A concurrent soft delete must win: without this, an edit landing just
    // after it would write a body back into the `[deleted]` row. `deleted_at`
    // is stored as an explicit NULL by `to_item`, hence both forms.
    let (image_condition, expected_image) = image_unchanged_condition(current_image_id);
    let comment_update = builder
        .update_expression(expression)
        .condition_expression(format!(
            "attribute_exists(#pk) AND (attribute_not_exists(deleted_at) OR deleted_at = :null) \
             AND {image_condition}"
        ))
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(":null", AttributeValue::Null(true))
        .expression_attribute_values(":expected_image", expected_image)
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

    send_transaction(repo, items).await
}

/// `DELETE .../comments/{commentId}` (§8.4) — soft delete. `image_id` is the
/// image the handler read on the comment (if any), released in the same
/// transaction. The delete only applies while the comment still holds
/// exactly that image; a cancelled transaction means an edit changed it (or
/// another delete already landed) and the handler should re-read and retry.
pub async fn soft_delete_comment(
    repo: &Repo,
    loc: &CommentLocation<'_>,
    deleted_at: DateTime<Utc>,
    image_id: Option<&ImageId>,
) -> Result<(), RepoError> {
    let (image_condition, expected_image) = image_unchanged_condition(image_id);
    let comment_update = comment_update_builder(repo, loc)
        .update_expression("SET deleted_at = :t, body = :empty REMOVE image_media_id")
        .condition_expression(format!("attribute_exists(#pk) AND {image_condition}"))
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(":t", AttributeValue::S(deleted_at.to_rfc3339()))
        .expression_attribute_values(":empty", AttributeValue::S("".into()))
        .expression_attribute_values(":expected_image", expected_image)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let mut items = vec![TransactWriteItem::builder().update(comment_update).build()];
    if let Some(image_id) = image_id {
        let detach =
            media::detach_image_from_comment_update(repo, loc.group_id, loc.cycle_id, image_id)?;
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
    let resp = repo
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
        .expression_attribute_values(":prefix", AttributeValue::S("R#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Reaction>(i).map_err(RepoError::from))
        .collect()
}

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
