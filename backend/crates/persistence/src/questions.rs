//! Candidate questions, candidate votes, locked questions.
//! AP10, AP11, AP12, AP13 plus transactions §4 #1, #2, #3.

use crate::error::RepoError;
use crate::keys::{
    attr, candidate_gsi1pk, candidate_gsi1sk, candidate_pk, candidate_sk, candidate_vote_pk,
    candidate_vote_sk, group_pk, index, locked_pk, locked_sk, newsletter_gsi2pk,
    newsletter_gsi2sk, newsletter_sk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem, Update};
use domain::{
    CandidateQuestion, CandidateVote, CycleId, GroupId, LockedQuestion, Newsletter,
    NewsletterStatus, QuestionId, UserId,
};
use serde_dynamo::{from_item, to_item};

/// AP10 — list candidate questions for the next cycle.
pub async fn list_candidates(
    repo: &Repo,
    group_id: &GroupId,
    next_cycle_id: &CycleId,
) -> Result<Vec<CandidateQuestion>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(candidate_pk(group_id, next_cycle_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("QC#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, CandidateQuestion>(i).map_err(RepoError::from))
        .collect()
}

/// AP11 — top N candidates by vote count.
pub async fn list_top_candidates_by_votes(
    repo: &Repo,
    group_id: &GroupId,
    next_cycle_id: &CycleId,
    limit: i32,
) -> Result<Vec<CandidateQuestion>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(candidate_gsi1pk(group_id, next_cycle_id)),
        )
        .scan_index_forward(false)
        .limit(limit)
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, CandidateQuestion>(i).map_err(RepoError::from))
        .collect()
}

/// AP12 — list a voter's votes for one cycle.
pub async fn list_my_votes(
    repo: &Repo,
    group_id: &GroupId,
    next_cycle_id: &CycleId,
    user_id: &UserId,
) -> Result<Vec<CandidateVote>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(candidate_vote_pk(group_id, next_cycle_id, user_id)),
        )
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, CandidateVote>(i).map_err(RepoError::from))
        .collect()
}

/// AP13 — list locked questions for a published/open newsletter.
pub async fn list_locked_questions(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
) -> Result<Vec<LockedQuestion>, RepoError> {
    let resp = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(locked_pk(group_id, cycle_id)))
        .expression_attribute_values(":prefix", AttributeValue::S("Q#".into()))
        .send()
        .await?;
    let items = resp.items.unwrap_or_default();
    items
        .into_iter()
        .map(|i| from_item::<_, LockedQuestion>(i).map_err(RepoError::from))
        .collect()
}

pub async fn put_candidate(repo: &Repo, q: &CandidateQuestion) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(q)?;
    item.insert(attr::PK.into(), AttributeValue::S(candidate_pk(&q.group_id, &q.next_cycle_id)));
    item.insert(attr::SK.into(), AttributeValue::S(candidate_sk(&q.question_id)));
    item.insert(attr::GSI1PK.into(), AttributeValue::S(candidate_gsi1pk(&q.group_id, &q.next_cycle_id)));
    item.insert(attr::GSI1SK.into(), AttributeValue::S(candidate_gsi1sk(q.vote_count, &q.question_id)));
    item.insert(attr::ENTITY.into(), AttributeValue::S("CandidateQuestion".into()));

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

pub async fn delete_candidate(
    repo: &Repo,
    group_id: &GroupId,
    next_cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<(), RepoError> {
    repo.client
        .delete_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(candidate_pk(group_id, next_cycle_id)))
        .key(attr::SK, AttributeValue::S(candidate_sk(question_id)))
        .send()
        .await?;
    Ok(())
}

/// Transaction §4 #1 — cast a vote.
///
/// - Put `CandidateVote` (must not already exist)
/// - Update `CandidateQuestion`: voteCount += 1 and refresh GSI1 sort key
pub async fn cast_vote_tx(
    repo: &Repo,
    vote: &CandidateVote,
    new_vote_count: u32,
) -> Result<(), RepoError> {
    let mut vote_item: std::collections::HashMap<String, AttributeValue> = to_item(vote)?;
    vote_item.insert(
        attr::PK.into(),
        AttributeValue::S(candidate_vote_pk(&vote.group_id, &vote.cycle_id, &vote.user_id)),
    );
    vote_item.insert(attr::SK.into(), AttributeValue::S(candidate_vote_sk(&vote.question_id)));
    vote_item.insert(attr::ENTITY.into(), AttributeValue::S("CandidateVote".into()));

    let put_vote = Put::builder()
        .table_name(&repo.table)
        .set_item(Some(vote_item))
        .condition_expression("attribute_not_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let new_gsi1sk = candidate_gsi1sk(new_vote_count, &vote.question_id);
    let bump = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(candidate_pk(&vote.group_id, &vote.cycle_id)))
        .key(attr::SK, AttributeValue::S(candidate_sk(&vote.question_id)))
        .update_expression("SET vote_count = vote_count + :one, #g1sk = :sk")
        .condition_expression("vote_count = :prev")
        .expression_attribute_names("#g1sk", attr::GSI1SK)
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .expression_attribute_values(
            ":prev",
            AttributeValue::N((new_vote_count.saturating_sub(1)).to_string()),
        )
        .expression_attribute_values(":sk", AttributeValue::S(new_gsi1sk))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().put(put_vote).build())
        .transact_items(TransactWriteItem::builder().update(bump).build())
        .send()
        .await?;
    Ok(())
}

/// Transaction §4 #2 — withdraw a vote.
pub async fn withdraw_vote_tx(
    repo: &Repo,
    vote: &CandidateVote,
    new_vote_count: u32,
) -> Result<(), RepoError> {
    use aws_sdk_dynamodb::types::Delete;
    let delete = Delete::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(candidate_vote_pk(&vote.group_id, &vote.cycle_id, &vote.user_id)),
        )
        .key(attr::SK, AttributeValue::S(candidate_vote_sk(&vote.question_id)))
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let new_gsi1sk = candidate_gsi1sk(new_vote_count, &vote.question_id);
    let dec = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(candidate_pk(&vote.group_id, &vote.cycle_id)))
        .key(attr::SK, AttributeValue::S(candidate_sk(&vote.question_id)))
        .update_expression("SET vote_count = vote_count - :one, #g1sk = :sk")
        .condition_expression("vote_count = :prev")
        .expression_attribute_names("#g1sk", attr::GSI1SK)
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .expression_attribute_values(":prev", AttributeValue::N((new_vote_count + 1).to_string()))
        .expression_attribute_values(":sk", AttributeValue::S(new_gsi1sk))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(dec).build())
        .send()
        .await?;
    Ok(())
}

/// Transaction §4 #3 — promote candidates into locked questions and flip the
/// newsletter status `voting → open`.
pub async fn promote_candidates_tx(
    repo: &Repo,
    locked: &[LockedQuestion],
    newsletter: &Newsletter,
) -> Result<(), RepoError> {
    if locked.len() > 99 {
        return Err(RepoError::Dynamo("transact_write_items cap is 100".into()));
    }

    let mut items: Vec<TransactWriteItem> = Vec::with_capacity(locked.len() + 1);

    for q in locked {
        let mut item: std::collections::HashMap<String, AttributeValue> = to_item(q)?;
        item.insert(attr::PK.into(), AttributeValue::S(locked_pk(&q.group_id, &q.cycle_id)));
        item.insert(attr::SK.into(), AttributeValue::S(locked_sk(&q.question_id)));
        item.insert(attr::ENTITY.into(), AttributeValue::S("LockedQuestion".into()));
        let put = Put::builder()
            .table_name(&repo.table)
            .set_item(Some(item))
            .build()
            .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
        items.push(TransactWriteItem::builder().put(put).build());
    }

    let ids_av: Vec<AttributeValue> = newsletter
        .locked_question_ids
        .iter()
        .map(|id| AttributeValue::S(id.to_string()))
        .collect();
    let when = newsletter
        .next_transition_at
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| "9999-12-31T00:00:00Z".to_owned());

    let flip = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(&newsletter.group_id)))
        .key(attr::SK, AttributeValue::S(newsletter_sk(&newsletter.cycle_id)))
        .update_expression(
            "SET #s = :open, locked_question_ids = :ids, #g2pk = :g2pk, #g2sk = :g2sk",
        )
        .condition_expression("#s = :voting")
        .expression_attribute_names("#s", "status")
        .expression_attribute_names("#g2pk", attr::GSI2PK)
        .expression_attribute_names("#g2sk", attr::GSI2SK)
        .expression_attribute_values(":voting", AttributeValue::S("voting".into()))
        .expression_attribute_values(":open", AttributeValue::S("open".into()))
        .expression_attribute_values(":ids", AttributeValue::L(ids_av))
        .expression_attribute_values(
            ":g2pk",
            AttributeValue::S(newsletter_gsi2pk(NewsletterStatus::Open)),
        )
        .expression_attribute_values(
            ":g2sk",
            AttributeValue::S(newsletter_gsi2sk(&when, &newsletter.group_id, &newsletter.cycle_id)),
        )
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    items.push(TransactWriteItem::builder().update(flip).build());

    let mut req = repo.client.transact_write_items();
    for i in items {
        req = req.transact_items(i);
    }
    req.send().await?;
    Ok(())
}
