//! Candidate questions, candidate votes, locked questions.
//! AP10, AP11, AP12, AP13 plus transactions §4 #1, #2, #3.

use crate::error::{self, RepoError};
use crate::keys::{
    attr, candidate_gsi1pk, candidate_gsi1sk, candidate_pk, candidate_sk, candidate_vote_pk,
    candidate_vote_sk, group_pk, index, locked_pk, locked_sk, newsletter_gsi2pk, newsletter_gsi2sk,
    newsletter_gsi2sk_sentinel, newsletter_sk, CANDIDATE_VOTE_SK_PREFIX, VOTER_TALLY_SK,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::types::{AttributeValue, Delete, Put, TransactWriteItem, Update};
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
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(candidate_pk(group_id, next_cycle_id)),
        )
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
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :vote)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(
            ":pk",
            AttributeValue::S(candidate_vote_pk(group_id, next_cycle_id, user_id)),
        )
        .expression_attribute_values(
            ":vote",
            AttributeValue::S(CANDIDATE_VOTE_SK_PREFIX.to_owned()),
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

/// Single-item fetch of one locked question, for handlers that already know
/// which question they need (e.g. validating a response save) and don't need
/// the whole newsletter's question list.
pub async fn get_locked_question(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Option<LockedQuestion>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(locked_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(locked_sk(question_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

pub async fn put_candidate(repo: &Repo, q: &CandidateQuestion) -> Result<(), RepoError> {
    let mut item: std::collections::HashMap<String, AttributeValue> = to_item(q)?;
    item.insert(
        attr::PK.into(),
        AttributeValue::S(candidate_pk(&q.group_id, &q.next_cycle_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(candidate_sk(&q.question_id)),
    );
    item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(candidate_gsi1pk(&q.group_id, &q.next_cycle_id)),
    );
    item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(candidate_gsi1sk(q.vote_count, &q.question_id)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("CandidateQuestion".into()),
    );

    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await?;
    Ok(())
}

/// Single-item fetch, for handlers that already know the candidate's cycle
/// (e.g. resolving a vote or an admin delete) and don't need the whole pool.
pub async fn get_candidate(
    repo: &Repo,
    group_id: &GroupId,
    next_cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Option<CandidateQuestion>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(candidate_pk(group_id, next_cycle_id)),
        )
        .key(attr::SK, AttributeValue::S(candidate_sk(question_id)))
        .send()
        .await?;
    match resp.item {
        Some(item) => Ok(Some(from_item(item)?)),
        None => Ok(None),
    }
}

/// Cascading part of admin candidate deletion (`03-api-contract.md` §6.5).
/// `02-data-model-dynamodb.md` has no access pattern for "list voters of
/// question X" — `CandidateVote` is keyed by voter, not by question — so
/// callers delete speculatively for every group member (bounded by
/// `memberSoftCap`) rather than scanning the table. Deleting an absent item is
/// a harmless no-op.
pub async fn delete_vote_if_present(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    voter_user_id: &UserId,
    question_id: &QuestionId,
) -> Result<(), RepoError> {
    // The vote and the voter's tally move together, so deleting a vote hands
    // the voter their slot back.
    let delete = vote_delete(repo, group_id, cycle_id, voter_user_id, question_id)?;
    let tally = tally_update(
        repo,
        group_id,
        cycle_id,
        voter_user_id,
        TallyChange::Release,
    )?;
    let result = repo
        .client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(tally).build())
        .send()
        .await;
    match result.map_err(|e| classify_vote_tx_error(e, [VoteTxError::VoteRow, VoteTxError::Tally]))
    {
        Ok(_) | Err(VoteTxError::VoteRow) => Ok(()),
        Err(VoteTxError::Repo(e)) => Err(e),
        Err(other) => Err(RepoError::TransactionCancelled(format!("{other:?}"))),
    }
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
        .key(
            attr::PK,
            AttributeValue::S(candidate_pk(group_id, next_cycle_id)),
        )
        .key(attr::SK, AttributeValue::S(candidate_sk(question_id)))
        .send()
        .await?;
    Ok(())
}

/// Why a vote transaction (§4 #1, #2) was cancelled, by which item's condition
/// failed. Callers turn each into a distinct API outcome.
#[derive(Debug)]
pub enum VoteTxError {
    /// The `CandidateVote` row: already present on cast, already gone on
    /// withdraw. A concurrent duplicate request won, so callers treat this as
    /// idempotent success.
    VoteRow,
    /// The candidate's `vote_count` changed since it was read, or the
    /// transaction conflicted with another. Retry against a fresh read.
    CountRace,
    /// The voter's tally: at `votesPerUserPerCycle` on cast (`VOTE_CAP_REACHED`),
    /// already zero on withdraw.
    Tally,
    Repo(RepoError),
}

/// Transaction §4 #1 — cast a vote.
///
/// - Put `CandidateVote` (must not already exist)
/// - Update `CandidateQuestion`: `vote_count += 1` and refresh the GSI1 sort key
/// - Update the voter's `VoterTally`: `votes_cast += 1`, conditioned on
///   `votes_cast < votes_cap`. This is the cap check `02` §4 #1 requires; a
///   pre-read alone lets concurrent votes on different candidates exceed it.
pub async fn cast_vote_tx(
    repo: &Repo,
    vote: &CandidateVote,
    new_vote_count: u32,
    votes_cap: u32,
) -> Result<(), VoteTxError> {
    let mut vote_item: std::collections::HashMap<String, AttributeValue> =
        to_item(vote).map_err(|e| VoteTxError::Repo(e.into()))?;
    vote_item.insert(
        attr::PK.into(),
        AttributeValue::S(candidate_vote_pk(
            &vote.group_id,
            &vote.cycle_id,
            &vote.user_id,
        )),
    );
    vote_item.insert(
        attr::SK.into(),
        AttributeValue::S(candidate_vote_sk(&vote.question_id)),
    );
    vote_item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("CandidateVote".into()),
    );

    let put_vote = Put::builder()
        .table_name(&repo.table)
        .set_item(Some(vote_item))
        .condition_expression("attribute_not_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| VoteTxError::Repo(RepoError::Dynamo(format!("{e:?}"))))?;
    let bump = count_update(repo, vote, new_vote_count, 1).map_err(VoteTxError::Repo)?;
    let tally = tally_update(
        repo,
        &vote.group_id,
        &vote.cycle_id,
        &vote.user_id,
        TallyChange::Take { cap: votes_cap },
    )
    .map_err(VoteTxError::Repo)?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().put(put_vote).build())
        .transact_items(TransactWriteItem::builder().update(bump).build())
        .transact_items(TransactWriteItem::builder().update(tally).build())
        .send()
        .await
        .map_err(|e| {
            classify_vote_tx_error(
                e,
                [
                    VoteTxError::VoteRow,
                    VoteTxError::CountRace,
                    VoteTxError::Tally,
                ],
            )
        })?;
    Ok(())
}

/// Transaction §4 #2 — withdraw a vote: delete the `CandidateVote`, decrement
/// the candidate's `vote_count`, and give the voter's tally slot back.
pub async fn withdraw_vote_tx(
    repo: &Repo,
    vote: &CandidateVote,
    new_vote_count: u32,
) -> Result<(), VoteTxError> {
    let delete = vote_delete(
        repo,
        &vote.group_id,
        &vote.cycle_id,
        &vote.user_id,
        &vote.question_id,
    )
    .map_err(VoteTxError::Repo)?;
    let dec = count_update(repo, vote, new_vote_count, -1).map_err(VoteTxError::Repo)?;
    let tally = tally_update(
        repo,
        &vote.group_id,
        &vote.cycle_id,
        &vote.user_id,
        TallyChange::Release,
    )
    .map_err(VoteTxError::Repo)?;

    repo.client
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().delete(delete).build())
        .transact_items(TransactWriteItem::builder().update(dec).build())
        .transact_items(TransactWriteItem::builder().update(tally).build())
        .send()
        .await
        .map_err(|e| {
            classify_vote_tx_error(
                e,
                [
                    VoteTxError::VoteRow,
                    VoteTxError::CountRace,
                    VoteTxError::Tally,
                ],
            )
        })?;
    Ok(())
}

/// `Delete` of one `CandidateVote`, conditioned on it existing.
fn vote_delete(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    voter_user_id: &UserId,
    question_id: &QuestionId,
) -> Result<Delete, RepoError> {
    Delete::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(candidate_vote_pk(group_id, cycle_id, voter_user_id)),
        )
        .key(attr::SK, AttributeValue::S(candidate_vote_sk(question_id)))
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// Optimistic `vote_count` change on the candidate: `delta` is +1 or -1 and
/// `new_vote_count` the expected result, so the condition is the prior value.
fn count_update(
    repo: &Repo,
    vote: &CandidateVote,
    new_vote_count: u32,
    delta: i64,
) -> Result<Update, RepoError> {
    let prev = i64::from(new_vote_count) - delta;
    Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(candidate_pk(&vote.group_id, &vote.cycle_id)),
        )
        .key(attr::SK, AttributeValue::S(candidate_sk(&vote.question_id)))
        .update_expression("SET vote_count = vote_count + :delta, #g1sk = :sk")
        .condition_expression("vote_count = :prev")
        .expression_attribute_names("#g1sk", attr::GSI1SK)
        .expression_attribute_values(":delta", AttributeValue::N(delta.to_string()))
        .expression_attribute_values(":prev", AttributeValue::N(prev.to_string()))
        .expression_attribute_values(
            ":sk",
            AttributeValue::S(candidate_gsi1sk(new_vote_count, &vote.question_id)),
        )
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

enum TallyChange {
    /// Use one of the voter's `cap` votes.
    Take { cap: u32 },
    /// Give one back.
    Release,
}

/// `Update` of the voter's `VoterTally` row (created on first vote).
fn tally_update(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    voter_user_id: &UserId,
    change: TallyChange,
) -> Result<Update, RepoError> {
    let builder = Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(candidate_vote_pk(group_id, cycle_id, voter_user_id)),
        )
        .key(attr::SK, AttributeValue::S(VOTER_TALLY_SK.to_owned()))
        .expression_attribute_names("#entity", attr::ENTITY)
        .expression_attribute_values(":entity", AttributeValue::S("VoterTally".into()));
    let builder = match change {
        TallyChange::Take { cap } => builder
            .update_expression("SET #entity = :entity ADD votes_cast :one")
            .condition_expression("attribute_not_exists(votes_cast) OR votes_cast < :cap")
            .expression_attribute_values(":one", AttributeValue::N("1".into()))
            .expression_attribute_values(":cap", AttributeValue::N(cap.to_string())),
        TallyChange::Release => builder
            .update_expression("SET #entity = :entity ADD votes_cast :minus_one")
            .condition_expression("votes_cast > :zero")
            .expression_attribute_values(":minus_one", AttributeValue::N("-1".into()))
            .expression_attribute_values(":zero", AttributeValue::N("0".into())),
    };
    builder
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// Map a vote transaction's failure to the item whose condition failed.
/// `labels[i]` names the outcome for transact item `i`; a transaction
/// conflict with a concurrent writer counts as a retryable [`VoteTxError::CountRace`].
fn classify_vote_tx_error<R: std::fmt::Debug, const N: usize>(
    err: SdkError<TransactWriteItemsError, R>,
    labels: [VoteTxError; N],
) -> VoteTxError {
    let Some(TransactWriteItemsError::TransactionCanceledException(cancel)) =
        err.as_service_error()
    else {
        return VoteTxError::Repo(error::from_transact_write_error(err));
    };
    let reasons = cancel.cancellation_reasons();
    if let Some(i) = reasons
        .iter()
        .position(|r| r.code() == Some("ConditionalCheckFailed"))
    {
        if let Some(label) = labels.into_iter().nth(i) {
            return label;
        }
    }
    if reasons
        .iter()
        .any(|r| r.code() == Some("TransactionConflict"))
    {
        return VoteTxError::CountRace;
    }
    VoteTxError::Repo(RepoError::TransactionCancelled(format!("{cancel:?}")))
}

// `questionsPerCycle` is capped at `MAX_QUESTIONS_PER_CYCLE` (20), so a
// promotion transaction (locked-question puts + 1 status-flip update) never
// comes close to TransactWriteItems' 100-item limit. If that config bound
// ever grows past this, `promote_candidates_tx`'s own `locked.len() > 99`
// guard below still catches it at runtime — this assertion just makes the
// invariant visible at compile time.
const _: () = assert!(
    shared::config::MAX_QUESTIONS_PER_CYCLE < 100,
    "promote_candidates_tx's TransactWriteItems must stay under DynamoDB's 100-item cap"
);

/// Transaction §4 #3 — promote candidates into locked questions and flip the
/// newsletter status `voting → open`.
///
/// Returns `Err(RepoError::TransactionCancelled)` when the flip lost a race
/// against another tick — the caller should log that at INFO and move on
/// rather than treating it as a failure (`plans/06-newsletter-lifecycle.md` §8).
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
        item.insert(
            attr::PK.into(),
            AttributeValue::S(locked_pk(&q.group_id, &q.cycle_id)),
        );
        item.insert(
            attr::SK.into(),
            AttributeValue::S(locked_sk(&q.question_id)),
        );
        item.insert(
            attr::ENTITY.into(),
            AttributeValue::S("LockedQuestion".into()),
        );
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
    let gsi2sk = match newsletter.next_transition_at {
        Some(d) => newsletter_gsi2sk(d, &newsletter.group_id, &newsletter.cycle_id),
        None => newsletter_gsi2sk_sentinel(&newsletter.group_id, &newsletter.cycle_id),
    };

    let flip = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(&newsletter.group_id)))
        .key(
            attr::SK,
            AttributeValue::S(newsletter_sk(&newsletter.cycle_id)),
        )
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
        .expression_attribute_values(":g2sk", AttributeValue::S(gsi2sk))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    items.push(TransactWriteItem::builder().update(flip).build());

    let mut req = repo.client.transact_write_items();
    for i in items {
        req = req.transact_items(i);
    }
    req.send().await.map_err(error::from_transact_write_error)?;
    Ok(())
}
