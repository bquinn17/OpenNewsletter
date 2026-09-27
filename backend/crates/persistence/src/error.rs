use aws_sdk_dynamodb::error::SdkError;
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::operation::update_item::UpdateItemError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("dynamodb error: {0}")]
    Dynamo(String),

    #[error("serialization error: {0}")]
    Serde(String),

    #[error("conditional check failed")]
    ConditionalCheckFailed,

    #[error("transaction cancelled: {0}")]
    TransactionCancelled(String),

    #[error("not found")]
    NotFound,
}

impl RepoError {
    /// True for a condition failure — an in-flight race that a concurrent
    /// writer won, not a real error. Callers such as `lambda-cycle-tick` log
    /// these at INFO and move on rather than treating them as failures
    /// (`plans/06-newsletter-lifecycle.md` §8).
    pub fn is_lost_race(&self) -> bool {
        matches!(
            self,
            RepoError::ConditionalCheckFailed | RepoError::TransactionCancelled(_)
        )
    }
}

impl<E, R> From<aws_sdk_dynamodb::error::SdkError<E, R>> for RepoError
where
    E: std::fmt::Debug,
    R: std::fmt::Debug,
{
    fn from(err: aws_sdk_dynamodb::error::SdkError<E, R>) -> Self {
        RepoError::Dynamo(format!("{err:?}"))
    }
}

impl From<serde_dynamo::Error> for RepoError {
    fn from(err: serde_dynamo::Error) -> Self {
        RepoError::Serde(err.to_string())
    }
}

/// Classify a `PutItem` failure, distinguishing a lost `attribute_not_exists`
/// race from every other error. The blanket `From<SdkError<..>>` above can't
/// do this — it has no operation-specific error type to inspect.
pub fn from_put_item_error<R: std::fmt::Debug>(err: SdkError<PutItemError, R>) -> RepoError {
    match err.as_service_error() {
        Some(e) if e.is_conditional_check_failed_exception() => RepoError::ConditionalCheckFailed,
        _ => RepoError::Dynamo(format!("{err:?}")),
    }
}

/// Classify an `UpdateItem` failure the same way as [`from_put_item_error`].
pub fn from_update_item_error<R: std::fmt::Debug>(err: SdkError<UpdateItemError, R>) -> RepoError {
    match err.as_service_error() {
        Some(e) if e.is_conditional_check_failed_exception() => RepoError::ConditionalCheckFailed,
        _ => RepoError::Dynamo(format!("{err:?}")),
    }
}

/// Classify a `TransactWriteItems` failure. DynamoDB reports a lost
/// conditional race as `TransactionCanceledException` regardless of which
/// item's condition failed; every write in this crate's transactions uses
/// exactly one status-guard condition, so any cancellation here means that
/// guard lost the race.
pub fn from_transact_write_error<R: std::fmt::Debug>(
    err: SdkError<TransactWriteItemsError, R>,
) -> RepoError {
    match err.as_service_error() {
        Some(e) if e.is_transaction_canceled_exception() => {
            RepoError::TransactionCancelled(format!("{e:?}"))
        }
        _ => RepoError::Dynamo(format!("{err:?}")),
    }
}
