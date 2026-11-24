//! Response (answer) drafts, queries, and the save/publish transaction (§4 #4).
//! AP14, AP15, AP16.

use crate::error::{self, RepoError};
use crate::keys::{
    attr, group_pk, index, newsletter_sk, reacted_gsi2pk, response_gsi1pk, response_gsi1sk,
    response_pk, response_sk,
};
use crate::query::query_all;
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, ConditionCheck, TransactWriteItem, Update};
use domain::{
    CycleId, GroupId, ImageId, MediaStatus, PollOptionId, QuestionId, QuestionKind, Response,
    ResponseId, Timestamp, UserId,
};
use serde_dynamo::from_item;
use shared::config::{
    LEADERBOARD_TOP_ANSWERS_MAX, LEADERBOARD_TOP_ANSWERS_PAGE, LEADERBOARD_TOP_ANSWER_RANKS,
};
use std::collections::HashMap;

/// AP14 — list all answers to a question (post-publish). Follows
/// `LastEvaluatedKey`: a member-capped group's 20k-char answers can exceed one
/// 1 MB query page, and the cycle-close auto-publish sweep must see every row.
pub async fn list_answers(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Vec<Response>, RepoError> {
    query_answer_items(repo, group_id, cycle_id, question_id)
        .await?
        .into_iter()
        .map(|item| Ok(from_item::<_, Response>(item)?))
        .collect()
}

/// An answer row that wouldn't deserialize, identified by its sort key.
#[derive(Debug)]
pub struct UnreadableAnswer {
    pub sk: String,
    pub error: String,
}

/// [`list_answers`] for the close-time auto-publish sweep: a row that doesn't
/// deserialize comes back in the second list instead of failing the whole
/// listing, so one corrupt row can't hold back every other answer to the
/// question (`docs/design/lifecycle.md` §5.3).
pub async fn list_answers_lenient(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<(Vec<Response>, Vec<UnreadableAnswer>), RepoError> {
    let mut answers = Vec::new();
    let mut unreadable = Vec::new();
    for item in query_answer_items(repo, group_id, cycle_id, question_id).await? {
        let sk = match item.get(attr::SK) {
            Some(AttributeValue::S(sk)) => sk.clone(),
            _ => String::new(),
        };
        match from_item::<_, Response>(item) {
            Ok(answer) => answers.push(answer),
            Err(e) => unreadable.push(UnreadableAnswer {
                sk,
                error: e.to_string(),
            }),
        }
    }
    Ok((answers, unreadable))
}

async fn query_answer_items(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
) -> Result<Vec<HashMap<String, AttributeValue>>, RepoError> {
    let request = repo
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
        .expression_attribute_values(":prefix", AttributeValue::S("A#".into()));
    query_all(request).await
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
    let request = repo
        .client
        .query()
        .table_name(&repo.table)
        .index_name(index::GSI1)
        .key_condition_expression("#pk = :pk")
        .expression_attribute_names("#pk", attr::GSI1PK)
        .expression_attribute_values(":pk", AttributeValue::S(response_gsi1pk(user_id, cycle_id)));
    let all: Vec<Response> = query_all(request)
        .await?
        .into_iter()
        .map(|i| from_item::<_, Response>(i).map_err(RepoError::from))
        .collect::<Result<_, _>>()?;
    Ok(all
        .into_iter()
        .filter(|r| &r.group_id == group_id)
        .collect())
}

/// AP24 — the group's most-reacted published text answers that `keep`
/// accepts, most reactions first (`docs/design/data-model.md` §2.9, §2.17;
/// `docs/design/api.md` §4.4): every answer whose count is one of the top
/// [`LEADERBOARD_TOP_ANSWER_RANKS`] distinct counts above 0, so ties are
/// never split. Capped at [`LEADERBOARD_TOP_ANSWERS_MAX`] rows by dropping
/// whole tie groups from the bottom; only the first group is ever cut.
///
/// Follows `LastEvaluatedKey` a page at a time, but stops as soon as the
/// answer is known: at the first row past the last rank, the first row at 0
/// (GSI2 sorts by count), or the first row past the cap. So it reads the top
/// of the group's history, not all of it.
pub async fn list_top_reacted_answers(
    repo: &Repo,
    group_id: &GroupId,
    keep: impl Fn(&Response) -> bool,
) -> Result<Vec<Response>, RepoError> {
    let mut groups: Vec<Vec<Response>> = Vec::new();
    let mut total = 0;
    let mut start_key = None;
    'pages: loop {
        let resp = repo
            .client
            .query()
            .table_name(&repo.table)
            .index_name(index::GSI2)
            .key_condition_expression("#pk = :pk")
            .expression_attribute_names("#pk", attr::GSI2PK)
            .expression_attribute_values(":pk", AttributeValue::S(reacted_gsi2pk(group_id)))
            .scan_index_forward(false)
            .limit(LEADERBOARD_TOP_ANSWERS_PAGE)
            .set_exclusive_start_key(start_key)
            .send()
            .await?;
        for item in resp.items.unwrap_or_default() {
            let answer: Response = from_item(item)?;
            if answer.reaction_count == 0 {
                break 'pages;
            }
            if !keep(&answer) {
                continue;
            }
            let ties_last = groups
                .last()
                .is_some_and(|group| group[0].reaction_count == answer.reaction_count);
            if ties_last {
                groups.last_mut().expect("checked above").push(answer);
            } else if groups.len() == LEADERBOARD_TOP_ANSWER_RANKS {
                break 'pages;
            } else {
                groups.push(vec![answer]);
            }
            total += 1;
            if total > LEADERBOARD_TOP_ANSWERS_MAX {
                break 'pages;
            }
        }
        start_key = resp.last_evaluated_key;
        if start_key.is_none() {
            break;
        }
    }

    let mut out = Vec::new();
    for (i, group) in groups.into_iter().enumerate() {
        if i == 0 {
            out.extend(group.into_iter().take(LEADERBOARD_TOP_ANSWERS_MAX));
        } else if out.len() + group.len() <= LEADERBOARD_TOP_ANSWERS_MAX {
            out.extend(group);
        } else {
            break;
        }
    }
    Ok(out)
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

/// An answer's current `reaction_count` (absent reads as 0), read strongly
/// consistent. The reaction transaction's retry (`docs/design/data-model.md`
/// §4 #8) re-reads with this after losing a count race: an eventually
/// consistent read could return the same stale count and lose again.
pub async fn get_reaction_count_consistent(
    repo: &Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    question_id: &QuestionId,
    user_id: &UserId,
) -> Result<Option<u32>, RepoError> {
    let resp = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(group_id, cycle_id, question_id)),
        )
        .key(attr::SK, AttributeValue::S(response_sk(user_id)))
        .consistent_read(true)
        .projection_expression("reaction_count")
        .send()
        .await?;
    let Some(item) = resp.item else {
        return Ok(None);
    };
    match item.get("reaction_count") {
        None => Ok(Some(0)),
        Some(AttributeValue::N(n)) => n
            .parse()
            .map(Some)
            .map_err(|e| RepoError::Dynamo(format!("bad reaction_count {n:?}: {e}"))),
        Some(other) => Err(RepoError::Dynamo(format!(
            "reaction_count is not a number: {other:?}"
        ))),
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
///    (`docs/design/data-model.md` §5). `responseId` is kept stable across saves
///    via `if_not_exists`. `status`/`publishedAt` use DB-side `if_not_exists`
///    tricks so publishing is sticky without needing a pre-read to race against
///    a concurrent save (`docs/design/api.md` §7.3): `publish=true` always sets
///    `status=published` and `publishedAt=if_not_exists(publishedAt, now)`;
///    `publish=false` sets `status=if_not_exists(status, "draft")`, which
///    leaves an already-`published` row published.
/// 2. A `ConditionCheck` on the Newsletter row: `status=open AND
///    responseCloseAt > now`. A cancelled transaction
///    (`RepoError::is_lost_race`) means this failed — the caller maps that to
///    409 `CYCLE_NOT_OPEN`.
///
/// `MemberStats.editionsAnswered` is not touched here: it's decided once, at
/// cycle close, by `lambda-cycle-tick`'s sweep (`docs/design/lifecycle.md`
/// §5.3, §4 #4a) over every member's answers as they stand at that moment.
///
/// A cancellation is a `TransactionConflict` with a concurrent write to the
/// response row, retried unchanged up to [`SAVE_ATTEMPTS`] attempts in total;
/// anything else, i.e. the cycle check, is returned as a lost race.
pub async fn save_response(
    repo: &Repo,
    save: &ResponseSave<'_>,
    now: Timestamp,
) -> Result<(), RepoError> {
    let mut attempt = 1;
    loop {
        match save_response_once(repo, save, now).await {
            Ok(()) => return Ok(()),
            Err(SaveFailure::Conflict(_)) if attempt < SAVE_ATTEMPTS => {}
            Err(SaveFailure::Conflict(e) | SaveFailure::Other(e)) => return Err(e),
        }
        attempt += 1;
    }
}

/// Total tries [`save_response`] makes when transactions keep conflicting.
const SAVE_ATTEMPTS: u32 = 3;

enum SaveFailure {
    /// Cancelled only by `TransactionConflict`s; safe to retry as-is.
    Conflict(RepoError),
    Other(RepoError),
}

async fn save_response_once(
    repo: &Repo,
    save: &ResponseSave<'_>,
    now: Timestamp,
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
    let images_av = image_ids_av(save.image_media_ids.iter());

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
        .expression_attribute_values(":updated_at", AttributeValue::S(now.to_string()))
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
        // `response_close_at` is a plain data attribute (not a key), so this
        // compares against its fixed-width millisecond `Timestamp` form, not
        // `key_timestamp`'s whole-second key form.
        .expression_attribute_values(":now", AttributeValue::S(now.to_string()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let items = vec![
        TransactWriteItem::builder().update(response_update).build(),
        TransactWriteItem::builder()
            .condition_check(cycle_check)
            .build(),
    ];

    let mut req = repo.client.transact_write_items();
    for i in items {
        req = req.transact_items(i);
    }
    let err = match req.send().await {
        Ok(_) => return Ok(()),
        Err(err) => err,
    };
    let codes = error::cancellation_codes(&err);
    let err = error::from_transact_write_error(err);
    if error::is_conflict_only(&codes) {
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

/// What [`auto_publish_draft`] did with one draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoPublishOutcome {
    /// The draft is now published.
    Published,
    /// The row was no longer a draft: an earlier pass (or the author) already
    /// published it.
    AlreadyPublished,
}

/// §4 #4a — auto-publish one non-empty draft at cycle close: `Update` the
/// response row, conditioned on `status = draft`, setting `status =
/// published` and `published_at = published_at` (the cycle's
/// `responseCloseAt`). Every other attribute a published row carries
/// (`response_id`, GSI1 keys, `entity`) was already written by the draft
/// save, exactly as [`save_response`] writes them.
///
/// A failed condition means the row was no longer a draft — an earlier pass
/// (or the author) already published it — and comes back as
/// [`AutoPublishOutcome::AlreadyPublished`], not an error. `MemberStats` is
/// not touched here: `lambda-cycle-tick`'s sweep bumps `editions_answered`
/// separately, once per answerer, after every draft in the cycle has been
/// swept (`docs/design/lifecycle.md` §5.3).
pub async fn auto_publish_draft(
    repo: &Repo,
    draft: &Response,
    published_at: Timestamp,
) -> Result<AutoPublishOutcome, RepoError> {
    let result = repo
        .client
        .update_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(
                &draft.group_id,
                &draft.cycle_id,
                &draft.question_id,
            )),
        )
        .key(attr::SK, AttributeValue::S(response_sk(&draft.user_id)))
        .update_expression("SET #status = :published, published_at = :published_at")
        .condition_expression("#status = :draft")
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(":published", AttributeValue::S("published".into()))
        .expression_attribute_values(":draft", AttributeValue::S("draft".into()))
        .expression_attribute_values(":published_at", AttributeValue::S(published_at.to_string()))
        .send()
        .await;
    match result {
        Ok(_) => Ok(AutoPublishOutcome::Published),
        Err(err) => match error::from_update_item_error(err) {
            RepoError::ConditionalCheckFailed => Ok(AutoPublishOutcome::AlreadyPublished),
            other => Err(other),
        },
    }
}

/// `DELETE /uploads/{imageId}` (`docs/design/api.md` §9.4) — drop `image_id`
/// from the caller's draft before the image itself is deleted, so the draft
/// never lists an image that no longer exists.
///
/// One conditional write on the response row, so it's atomic on its own:
/// conditioned on the row still being a draft and its `image_media_ids` still
/// being exactly what `draft` read. A concurrent save, publish or auto-publish
/// in between fails it with `RepoError::ConditionalCheckFailed`; the caller
/// rereads and decides again (a now-published response refuses the delete).
pub async fn remove_image_from_draft(
    repo: &Repo,
    draft: &Response,
    image_id: &ImageId,
    now: Timestamp,
) -> Result<(), RepoError> {
    let expected = image_ids_av(draft.image_media_ids.iter());
    let remaining = image_ids_av(draft.image_media_ids.iter().filter(|i| *i != image_id));
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(
            attr::PK,
            AttributeValue::S(response_pk(
                &draft.group_id,
                &draft.cycle_id,
                &draft.question_id,
            )),
        )
        .key(attr::SK, AttributeValue::S(response_sk(&draft.user_id)))
        .update_expression("SET image_media_ids = :remaining, updated_at = :now")
        .condition_expression("#status = :draft AND image_media_ids = :expected")
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(":draft", AttributeValue::S("draft".into()))
        .expression_attribute_values(":expected", expected)
        .expression_attribute_values(":remaining", remaining)
        .expression_attribute_values(":now", AttributeValue::S(now.to_string()))
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}

/// `image_media_ids` as stored: a list of image id strings, in order.
fn image_ids_av<'a>(ids: impl Iterator<Item = &'a ImageId>) -> AttributeValue {
    AttributeValue::L(ids.map(|i| AttributeValue::S(i.to_string())).collect())
}

/// [`Response::is_non_empty`], checked against the images the response
/// lists. `is_non_empty` trusts `image_media_ids`, but a draft can still list
/// a deleted image: `DELETE /uploads` drops the id from the caller's draft
/// ([`remove_image_from_draft`]), but a save that validated the image just
/// before the delete can re-list it. So a text response with no body counts only if at least one of
/// its images still exists and hasn't failed processing. The close-time
/// auto-publish sweep and the deadline reminder's answered count both use
/// this, so a draft that won't publish never counts as answered
/// (`docs/design/lifecycle.md` §5.3, `docs/design/notifications.md` §7.2).
pub async fn has_live_content(repo: &Repo, response: &Response) -> Result<bool, RepoError> {
    if !response.is_non_empty() {
        return Ok(false);
    }
    if response.kind != QuestionKind::Text || response.has_text_body() {
        return Ok(true);
    }
    let images = crate::media::get_images_batch(
        repo,
        &response.group_id,
        &response.cycle_id,
        &response.image_media_ids,
    )
    .await?;
    Ok(images.values().any(|i| i.status != MediaStatus::Failed))
}
