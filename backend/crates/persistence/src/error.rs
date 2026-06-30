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
