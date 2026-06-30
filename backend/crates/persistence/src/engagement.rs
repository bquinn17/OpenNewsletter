//! Comments and reactions (AP18, AP19).

use crate::error::RepoError;
use crate::keys::{attr, comment_sk, engagement_pk, reaction_sk};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::AttributeValue;
use domain::{Comment, CycleId, GroupId, QuestionId, Reaction, UserId};
use serde_dynamo::{from_item, to_item};

/// AP18 — comments on an answer, oldest first.
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
            AttributeValue::S(engagement_pk(group_id, cycle_id, question_id, answer_user_id)),
        )
        .expression_attribute_values(":prefix", AttributeValue::S("C#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Comment>(i).map_err(RepoError::from))
        .collect()
}

pub async fn put_comment(repo: &Repo, c: &Comment) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(c)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(engagement_pk(&c.group_id, &c.cycle_id, &c.question_id, &c.answer_user_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(comment_sk(&c.created_at.to_rfc3339(), &c.comment_id)),
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

pub async fn soft_delete_comment(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    answer_user_id: &UserId,
    created_at_iso: &str,
    comment_id: &domain::CommentId,
    deleted_at_iso: &str,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(engagement_pk(group_id, cycle_id, question_id, answer_user_id)),
        )
        .key(
            attr::SK,
            AttributeValue::S(comment_sk(created_at_iso, comment_id)),
        )
        .update_expression("SET deleted_at = :t, body = :empty REMOVE image_media_id")
        .expression_attribute_values(":t", AttributeValue::S(deleted_at_iso.into()))
        .expression_attribute_values(":empty", AttributeValue::S("".into()))
        .send()
        .await?;
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
            AttributeValue::S(engagement_pk(group_id, cycle_id, question_id, answer_user_id)),
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
        AttributeValue::S(engagement_pk(&r.group_id, &r.cycle_id, &r.question_id, &r.answer_user_id)),
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
            AttributeValue::S(engagement_pk(group_id, cycle_id, question_id, answer_user_id)),
        )
        .key(attr::SK, AttributeValue::S(reaction_sk(reactor_user_id, emoji)))
        .send()
        .await?;
    Ok(())
}
