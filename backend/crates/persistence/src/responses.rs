//! Response (answer) drafts, queries, and the save/publish transaction (§4 #4).
//! AP14, AP15, AP16.

use crate::error::{self, RepoError};
use crate::keys::{
    attr, group_pk, index, key_timestamp, membership_sk, newsletter_sk, response_gsi1pk,
    response_gsi1sk, response_pk, response_sk, user_pk,
};
use crate::repo::Repo;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::types::{AttributeValue, ConditionCheck, TransactWriteItem, Update};
use chrono::{DateTime, Utc};
use domain::{
    CycleId, GroupId, ImageId, PollOptionId, QuestionId, QuestionKind, Response, ResponseId, UserId,
};
use serde_dynamo::from_item;

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

/// AP15 — list a user's drafts/publishes for one group's cycle (via GSI1).
/// GSI1's key is `USER#{u}#NL#{cycleId}` with no group, and cycle ids are
/// calendar months shared by every group, so a user in two groups gets both
/// groups' responses back from the query; `group_id` filters them down.
pub async fn list_my_responses_in_cycle(
    repo: &Repo,
    user_id: &UserId,
    group_id: &GroupId,
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
    let all: Vec<Response> = items
        .into_iter()
        .map(|i| from_item::<_, Response>(i).map_err(RepoError::from))
        .collect::<Result<_, _>>()?;
    Ok(all
        .into_iter()
        .filter(|r| &r.group_id == group_id)
        .collect())
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

/// The caller's intended new content for one question — independent of
/// whatever (if anything) the row currently holds. See [`save_response`].
pub struct ResponseSave<'a> {
    pub group_id: &'a GroupId,
    pub cycle_id: &'a CycleId,
    pub question_id: &'a QuestionId,
    pub user_id: &'a UserId,
    pub kind: QuestionKind,
    pub body: Option<&'a str>,
    pub poll_option_id: Option<&'a PollOptionId>,
    pub image_media_ids: &'a [ImageId],
    pub publish: bool,
}

/// Transaction §4 #4 — draft or publish a response, in one `TransactWriteItems`:
///
/// 1. `Update` the response row. Content (`body`/`imageMediaIds` or
///    `pollOptionId`) is a plain overwrite — last-write-wins, no version token
///    (`02-data-model-dynamodb.md` §5). `responseId` is kept stable across saves
///    via `if_not_exists`. `status`/`publishedAt` use DB-side `if_not_exists`
///    tricks so publishing is sticky without needing a pre-read to race against
///    a concurrent save (`03-api-contract.md` §7.3): `publish=true` always sets
///    `status=published` and `publishedAt=if_not_exists(publishedAt, now)`;
///    `publish=false` sets `status=if_not_exists(status, "draft")`, which
///    leaves an already-`published` row published.
/// 2. A `ConditionCheck` on the Newsletter row: `status=open AND
///    responseCloseAt > now`. A cancelled transaction
///    (`RepoError::is_lost_race`) means this failed — the caller maps that to
///    409 `CYCLE_NOT_OPEN`.
/// 3. IFF `bump_editions_answered`, an `Update` incrementing the caller's
///    `GroupMembership.editionsAnswered` and stamping `lastAnsweredCycle`,
///    conditioned on `lastAnsweredCycle` not already being this cycle. The
///    caller's AP15 pre-read ("first publish in this cycle") decides whether
///    to ask for the bump; the marker makes it idempotent when two first
///    publishes race past that pre-read (found M13).
///
/// Cancellations are told apart by their per-item reasons:
/// - **The bump's marker condition failed.** A concurrent publish already
///   counted this cycle, so the transaction is retried without the bump.
/// - **A `TransactionConflict`** with a concurrent write to one of the same
///   items (e.g. the other racing publish's bump). The transaction is retried
///   unchanged, up to [`SAVE_ATTEMPTS`] attempts in total.
/// - **Anything else**, i.e. the cycle check, is returned as a lost race.
pub async fn save_response(
    repo: &Repo,
    save: &ResponseSave<'_>,
    now: DateTime<Utc>,
    bump_editions_answered: bool,
) -> Result<(), RepoError> {
    let mut bump = bump_editions_answered;
    let mut attempt = 1;
    loop {
        match save_response_once(repo, save, now, bump).await {
            Ok(()) => return Ok(()),
            Err(SaveFailure::AlreadyCounted) => bump = false,
            Err(SaveFailure::Conflict(_)) if attempt < SAVE_ATTEMPTS => {}
            Err(SaveFailure::Conflict(e) | SaveFailure::Other(e)) => return Err(e),
        }
        attempt += 1;
    }
}

/// Total tries [`save_response`] makes when transactions keep conflicting.
const SAVE_ATTEMPTS: u32 = 3;

/// Positions in the transaction's item list (response update first).
const CYCLE_CHECK_ITEM_INDEX: usize = 1;
const BUMP_ITEM_INDEX: usize = 2;

enum SaveFailure {
    /// The bump's `lastAnsweredCycle` condition failed: this cycle is already counted.
    AlreadyCounted,
    /// Cancelled only by `TransactionConflict`s; safe to retry as-is.
    Conflict(RepoError),
    Other(RepoError),
}

async fn save_response_once(
    repo: &Repo,
    save: &ResponseSave<'_>,
    now: DateTime<Utc>,
    bump_editions_answered: bool,
) -> Result<(), SaveFailure> {
    let kind_str = match save.kind {
        QuestionKind::Text => "text",
        QuestionKind::Poll => "poll",
    };
    let body_av = match save.body {
        Some(b) => AttributeValue::S(b.to_owned()),
        None => AttributeValue::Null(true),
    };
    let poll_option_av = match save.poll_option_id {
        Some(p) => AttributeValue::S(p.to_string()),
        None => AttributeValue::Null(true),
    };
    let images_av = AttributeValue::L(
        save.image_media_ids
            .iter()
            .map(|i| AttributeValue::S(i.to_string()))
            .collect(),
    );

    let mut set_clauses = vec![
        "response_id = if_not_exists(response_id, :new_response_id)".to_owned(),
        "user_id = :user_id".to_owned(),
        "question_id = :question_id".to_owned(),
        "group_id = :group_id".to_owned(),
        "cycle_id = :cycle_id".to_owned(),
        "kind = :kind".to_owned(),
        "body = :body".to_owned(),
        "poll_option_id = :poll_option_id".to_owned(),
        "image_media_ids = :image_media_ids".to_owned(),
        "updated_at = :updated_at".to_owned(),
        "gsi1pk = :gsi1pk".to_owned(),
        "gsi1sk = :gsi1sk".to_owned(),
        "entity = :entity".to_owned(),
    ];
    if save.publish {
        set_clauses.push("#status = :published".to_owned());
        set_clauses.push("published_at = if_not_exists(published_at, :updated_at)".to_owned());
    } else {
        set_clauses.push("#status = if_not_exists(#status, :draft)".to_owned());
    }

    let mut response_update = Update::builder()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(save.group_id, save.cycle_id, save.question_id)),
        )
        .key(attr::SK, AttributeValue::S(response_sk(save.user_id)))
        .update_expression(format!("SET {}", set_clauses.join(", ")))
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(
            ":new_response_id",
            AttributeValue::S(ResponseId::generate().to_string()),
        )
        .expression_attribute_values(":user_id", AttributeValue::S(save.user_id.to_string()))
        .expression_attribute_values(
            ":question_id",
            AttributeValue::S(save.question_id.to_string()),
        )
        .expression_attribute_values(":group_id", AttributeValue::S(save.group_id.to_string()))
        .expression_attribute_values(":cycle_id", AttributeValue::S(save.cycle_id.to_string()))
        .expression_attribute_values(":kind", AttributeValue::S(kind_str.into()))
        .expression_attribute_values(":body", body_av)
        .expression_attribute_values(":poll_option_id", poll_option_av)
        .expression_attribute_values(":image_media_ids", images_av)
        .expression_attribute_values(":updated_at", AttributeValue::S(now.to_rfc3339()))
        .expression_attribute_values(
            ":gsi1pk",
            AttributeValue::S(response_gsi1pk(save.user_id, save.cycle_id)),
        )
        .expression_attribute_values(
            ":gsi1sk",
            AttributeValue::S(response_gsi1sk(save.question_id)),
        )
        .expression_attribute_values(":entity", AttributeValue::S("Response".into()));

    response_update = if save.publish {
        response_update
            .expression_attribute_values(":published", AttributeValue::S("published".into()))
    } else {
        response_update.expression_attribute_values(":draft", AttributeValue::S("draft".into()))
    };

    let response_update = response_update
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let cycle_check = ConditionCheck::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(save.group_id)))
        .key(attr::SK, AttributeValue::S(newsletter_sk(save.cycle_id)))
        .condition_expression("#s = :open AND response_close_at > :now")
        .expression_attribute_names("#s", "status")
        .expression_attribute_values(":open", AttributeValue::S("open".into()))
        .expression_attribute_values(":now", AttributeValue::S(key_timestamp(now)))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let mut items = vec![
        TransactWriteItem::builder().update(response_update).build(),
        TransactWriteItem::builder()
            .condition_check(cycle_check)
            .build(),
    ];

    if bump_editions_answered {
        let bump = Update::builder()
            .table_name(&repo.table)
            .key(attr::PK, AttributeValue::S(user_pk(save.user_id)))
            .key(attr::SK, AttributeValue::S(membership_sk(save.group_id)))
            .update_expression(
                "SET editions_answered = editions_answered + :one, last_answered_cycle = :cycle",
            )
            .condition_expression(
                "attribute_not_exists(last_answered_cycle) OR last_answered_cycle <> :cycle",
            )
            .expression_attribute_values(":one", AttributeValue::N("1".into()))
            .expression_attribute_values(":cycle", AttributeValue::S(save.cycle_id.to_string()))
            .build()
            .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
        items.push(TransactWriteItem::builder().update(bump).build());
    }

    let mut req = repo.client.transact_write_items();
    for i in items {
        req = req.transact_items(i);
    }
    let err = match req.send().await {
        Ok(_) => return Ok(()),
        Err(err) => err,
    };
    let codes: Vec<Option<String>> = match err.as_service_error() {
        Some(TransactWriteItemsError::TransactionCanceledException(e)) => e
            .cancellation_reasons()
            .iter()
            .map(|r| r.code().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    let err = error::from_transact_write_error(err);
    let code_at = |i: usize| codes.get(i).and_then(|c| c.as_deref());
    if bump_editions_answered
        && code_at(BUMP_ITEM_INDEX) == Some("ConditionalCheckFailed")
        && code_at(CYCLE_CHECK_ITEM_INDEX) != Some("ConditionalCheckFailed")
    {
        return Err(SaveFailure::AlreadyCounted);
    }
    let conflict_only = codes
        .iter()
        .any(|c| c.as_deref() == Some("TransactionConflict"))
        && codes.iter().all(|c| {
            matches!(
                c.as_deref(),
                None | Some("None") | Some("TransactionConflict")
            )
        });
    if conflict_only {
        Err(SaveFailure::Conflict(err))
    } else {
        Err(SaveFailure::Other(err))
    }
}

impl From<RepoError> for SaveFailure {
    fn from(e: RepoError) -> Self {
        SaveFailure::Other(e)
    }
}
