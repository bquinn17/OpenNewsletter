//! Response (answer) drafts, queries, and the publish transaction (§4 #4).
//! AP14, AP15, AP16.

use crate::error::RepoError;
use crate::keys::{
    attr, index, membership_sk, response_gsi1pk, response_gsi1sk, response_pk, response_sk, user_pk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, TransactWriteItem, Update};
use domain::{CycleId, GroupId, QuestionId, Response, UserId};
use serde_dynamo::{from_item, to_item};

/// AP14 — list all answers to a question (post-publish).
pub async fn list_answers(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Vec<Response>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(response_pk(group_id, cycle_id, question_id)),
        )
        .expression_attribute_values(":prefix", AttributeValue::S("A#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Response>(i).map_err(RepoError::from))
        .collect()
}

/// AP15 — list a user's drafts/publishes for a cycle (via GSI1).
pub async fn list_my_responses_in_cycle(
    repo: &Repo,
    user_id: &UserId,
    cycle_id: &CycleId,
) -> Result<Vec<Response>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_values(":pk", AttributeValue::S(response_gsi1pk(user_id, cycle_id)))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, Response>(i).map_err(RepoError::from))
        .collect()
}

/// AP16 — get my response to a question.
pub async fn get_my_response(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    user_id: &UserId,
) -> Result<Option<Response>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(group_id, cycle_id, question_id)),
        )
        .key(attr::SK, AttributeValue::S(response_sk(user_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// Last-write-wins draft save. Unconditional Put per `02-data-model-dynamodb.md` §5.
pub async fn save_draft(repo: &Repo, r: &Response) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(r)?;
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
        .await?;
    Ok(())
}

/// Transaction §4 #4 — publish response. Flips status `draft → published`, sets
/// `publishedAt`, and bumps `editionsAnswered` IFF this is the user's first
/// publish in this cycle (caller validates that precondition via AP15).
pub async fn publish_response_tx(
    repo: &Repo,
    r: &Response,
    published_at_iso: &str,
    is_first_publish_in_cycle: bool,
) -> Result<(), RepoError> {
    let flip = Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(&r.group_id, &r.cycle_id, &r.question_id)),
        )
        .key(attr::SK, AttributeValue::S(response_sk(&r.user_id)))
        .update_expression("SET #s = :pub, published_at = :at, updated_at = :at")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":pub", AttributeValue::S("published".into()))
        .expression_attribute_values(":at", AttributeValue::S(published_at_iso.into()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let mut items = vec![TransactWriteItem::builder().update(flip).build()];

    if is_first_publish_in_cycle {
        let bump = Update::builder()
            .table_name(&repo.table)
            .key(attr::PK, AttributeValue::S(user_pk(&r.user_id)))
            .key(attr::SK, AttributeValue::S(membership_sk(&r.group_id)))
            .update_expression("SET editions_answered = editions_answered + :one")
            .expression_attribute_values(":one", AttributeValue::N("1".into()))
            .build()
            .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
        items.push(TransactWriteItem::builder().update(bump).build());
    }

    let mut req = repo.client.transact_write_items();
    for i in items {
        req = req.transact_items(i);
    }
    req.send().await?;
    Ok(())
}
