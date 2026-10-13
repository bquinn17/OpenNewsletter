//! `Query` pagination: follow `LastEvaluatedKey` to exhaustion.
//!
//! DynamoDB caps a single `Query` response at 1 MB, so a caller that reads
//! only `resp.items` and ignores `last_evaluated_key` silently drops every
//! item past that page (e.g. 50+ answers at the response body's 20,000-char
//! cap already exceed it). Every "list everything" query goes through
//! [`query_all`] so that can't happen.

use crate::error::RepoError;
use aws_sdk_dynamodb::operation::query::builders::QueryFluentBuilder;
use aws_sdk_dynamodb::types::AttributeValue;
use std::collections::HashMap;

type Item = HashMap<String, AttributeValue>;

/// Runs `request` to exhaustion, accumulating every page's items in query
/// order. `request` may already carry its own `Limit` (callers that want a
/// capped *page size* rather than a capped *result count* can set one); this
/// only stops once DynamoDB reports no `LastEvaluatedKey`.
pub(crate) async fn query_all(request: QueryFluentBuilder) -> Result<Vec<Item>, RepoError> {
    let mut items = Vec::new();
    let mut next = request;
    loop {
        let resp = next.clone().send().await?;
        items.extend(resp.items.unwrap_or_default());
        match resp.last_evaluated_key {
            Some(key) if !key.is_empty() => next = next.set_exclusive_start_key(Some(key)),
            _ => break,
        }
    }
    Ok(items)
}
