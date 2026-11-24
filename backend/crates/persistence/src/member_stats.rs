//! `MemberStats` upserts and the all-members leaderboard query
//! (`docs/design/data-model.md` §2.17; transactions §4 #3, #4a, #6, #8, #9).
//!
//! Every write here is an upsert `UpdateItem` — `SET entity, group_id,
//! user_id` plus the caller's own counter change — so a member's row appears
//! on its first write and no writer ever needs to read it first. The `ADD`
//! builder ([`add_update`]) is shared by the reaction transaction
//! (`engagement.rs`), the comment transaction (`engagement.rs`), and the
//! promotion transaction (`questions.rs`); the streak reset builder
//! ([`reset_current_streak_update`]) is used by the leave transaction
//! (`groups.rs`). [`settle_streak`] and [`bump_editions_answered`] are
//! sent by this module itself, for `lambda-cycle-tick`'s close-time sweep
//! (`docs/design/lifecycle.md` §5.3): the former with a check that the
//! member still belongs to the group, the latter without one, since the
//! count belongs to whoever answered even if they've since left.

use crate::error::{self, RepoError};
use crate::keys::{
    attr, member_stats_pk, member_stats_sk, membership_sk, user_pk, MEMBER_STATS_SK_PREFIX,
};
use crate::query::query_all;
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, ConditionCheck, TransactWriteItem, Update};
use domain::{CycleId, GroupId, MemberStats, UserId};
use serde_dynamo::from_item;
use shared::config::MEMBER_STATS_TX_ATTEMPTS;

/// AP23 — every member's stats row in a group, reading every page. The whole
/// set is safe to hold in memory: one row per member, bounded by `memberSoftCap`.
pub async fn list_stats_for_group(
    repo: &Repo,
    group_id: &GroupId,
) -> Result<Vec<MemberStats>, RepoError> {
    let request = repo
        .client
        .query()
        .table_name(&repo.table)
        .key_condition_expression("#pk = :pk AND begins_with(#sk, :prefix)")
        .expression_attribute_names("#pk", attr::PK)
        .expression_attribute_names("#sk", attr::SK)
        .expression_attribute_values(":pk", AttributeValue::S(member_stats_pk(group_id)))
        .expression_attribute_values(":prefix", AttributeValue::S(MEMBER_STATS_SK_PREFIX.into()));
    query_all(request)
        .await?
        .into_iter()
        .map(|i| from_item::<_, MemberStats>(i).map_err(RepoError::from))
        .collect()
}

/// Builds (does not send) an unconditioned `ADD`-only upsert of one counter on
/// a member's stats row. `field` must be one of `MemberStats`'s own counter
/// attribute names (`reactions_received`, `reactions_given`,
/// `comments_written`, `questions_asked`) — every call site in this crate
/// passes a literal, so there's no untrusted input to validate.
pub(crate) fn add_update(
    repo: &Repo,
    group_id: &GroupId,
    user_id: &UserId,
    field: &str,
    delta: i64,
) -> Result<Update, RepoError> {
    Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(member_stats_pk(group_id)))
        .key(attr::SK, AttributeValue::S(member_stats_sk(user_id)))
        .update_expression(
            "SET entity = :entity, group_id = :group_id, user_id = :user_id ADD #field :delta",
        )
        .expression_attribute_names("#field", field)
        .expression_attribute_values(":entity", AttributeValue::S("MemberStats".into()))
        .expression_attribute_values(":group_id", AttributeValue::S(group_id.to_string()))
        .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":delta", AttributeValue::N(delta.to_string()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// Bump one member's `editions_answered` by 1 and stamp `last_answered_cycle
/// = cycle_id`, conditioned on that marker not already being this cycle, so a
/// retried tick or several non-empty answers in the same cycle count it once
/// (`docs/design/lifecycle.md` §5.3, `docs/design/data-model.md` §4 #4a). A
/// standalone conditional `UpdateItem`, not part of a transaction: the
/// cycle-tick sweep calls it once per answerer after the draft sweep. A
/// `RepoError::ConditionalCheckFailed` means the cycle is already counted —
/// callers treat that as success via `RepoError::is_lost_race`.
pub async fn bump_editions_answered(
    repo: &Repo,
    group_id: &GroupId,
    user_id: &UserId,
    cycle_id: &CycleId,
) -> Result<(), RepoError> {
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(member_stats_pk(group_id)))
        .key(attr::SK, AttributeValue::S(member_stats_sk(user_id)))
        .update_expression(
            "SET entity = :entity, group_id = :group_id, user_id = :user_id, \
             last_answered_cycle = :cycle ADD editions_answered :one",
        )
        .condition_expression(
            "attribute_not_exists(last_answered_cycle) OR last_answered_cycle <> :cycle",
        )
        .expression_attribute_values(":entity", AttributeValue::S("MemberStats".into()))
        .expression_attribute_values(":group_id", AttributeValue::S(group_id.to_string()))
        .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":cycle", AttributeValue::S(cycle_id.to_string()))
        .expression_attribute_values(":one", AttributeValue::N("1".into()))
        .send()
        .await
        .map_err(error::from_update_item_error)?;
    Ok(())
}

/// Builds (does not send) an unconditioned upsert that zeroes a member's
/// `current_streak` — the leave transaction's step (§4 #6). `longest_streak`
/// and every other counter are left untouched.
pub(crate) fn reset_current_streak_update(
    repo: &Repo,
    group_id: &GroupId,
    user_id: &UserId,
) -> Result<Update, RepoError> {
    Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(member_stats_pk(group_id)))
        .key(attr::SK, AttributeValue::S(member_stats_sk(user_id)))
        .update_expression(
            "SET entity = :entity, group_id = :group_id, user_id = :user_id, current_streak = :zero",
        )
        .expression_attribute_values(":entity", AttributeValue::S("MemberStats".into()))
        .expression_attribute_values(":group_id", AttributeValue::S(group_id.to_string()))
        .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":zero", AttributeValue::N("0".into()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))
}

/// `lambda-cycle-tick`'s per-member streak settlement (`docs/design/lifecycle.md`
/// §5.3): sets `current_streak`, `longest_streak` and `streak_cycle` to their
/// freshly-computed values, conditioned on this cycle not already being
/// settled and on `user_id` still being a member of the group.
///
/// The values are computed from a stats snapshot read earlier, so without the
/// membership check a leave (§4 #6) committing in between would have its
/// `current_streak = 0` overwritten, and a later rejoin would pick up the
/// stale streak. The leave transaction deletes the membership row in the same
/// write that zeroes the streak, so either this settlement lands first and
/// the leave zeroes it, or the leave lands first and this one is refused.
///
/// A `RepoError::TransactionCancelled` means an earlier, crashed pass already
/// settled this member for `cycle_id`, or they left — the caller skips it
/// silently (`RepoError::is_lost_race`). A plain `TransactionConflict` is
/// retried, up to `MEMBER_STATS_TX_ATTEMPTS` sends in all.
pub async fn settle_streak(
    repo: &Repo,
    group_id: &GroupId,
    user_id: &UserId,
    current_streak: i64,
    longest_streak: i64,
    cycle_id: &CycleId,
) -> Result<(), RepoError> {
    let settle = Update::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(member_stats_pk(group_id)))
        .key(attr::SK, AttributeValue::S(member_stats_sk(user_id)))
        .update_expression(
            "SET entity = :entity, group_id = :group_id, user_id = :user_id, \
             current_streak = :current, longest_streak = :longest, streak_cycle = :cycle",
        )
        .condition_expression("attribute_not_exists(streak_cycle) OR streak_cycle <> :cycle")
        .expression_attribute_values(":entity", AttributeValue::S("MemberStats".into()))
        .expression_attribute_values(":group_id", AttributeValue::S(group_id.to_string()))
        .expression_attribute_values(":user_id", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":current", AttributeValue::N(current_streak.to_string()))
        .expression_attribute_values(":longest", AttributeValue::N(longest_streak.to_string()))
        .expression_attribute_values(":cycle", AttributeValue::S(cycle_id.to_string()))
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    let still_member = ConditionCheck::builder()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(user_pk(user_id)))
        .key(attr::SK, AttributeValue::S(membership_sk(group_id)))
        .condition_expression("attribute_exists(#sk)")
        .expression_attribute_names("#sk", attr::SK)
        .build()
        .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;

    error::transact_write_retrying_conflicts(
        repo,
        vec![
            TransactWriteItem::builder().update(settle).build(),
            TransactWriteItem::builder()
                .condition_check(still_member)
                .build(),
        ],
        MEMBER_STATS_TX_ATTEMPTS,
        "streak settlement",
    )
    .await
}
