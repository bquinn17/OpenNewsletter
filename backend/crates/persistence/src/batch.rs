//! `BatchGetItem` with chunking and `UnprocessedKeys` retry.
//!
//! DynamoDB may return part of a batch as `UnprocessedKeys` under throttling;
//! a caller that ignores them silently loses rows. Every batch read goes through
//! [`batch_get_items`] so that can't happen.

use crate::error::RepoError;
use crate::repo::Repo;
use aws_sdk_dynamodb::types::{AttributeValue, KeysAndAttributes};
use std::collections::HashMap;
use std::time::Duration;

type Item = HashMap<String, AttributeValue>;

/// `BatchGetItem`'s per-request key limit.
const MAX_KEYS_PER_REQUEST: usize = 100;

/// Rounds of `UnprocessedKeys` retry before giving up with an error.
const MAX_ATTEMPTS: u32 = 5;

/// Fetch every item for `keys`, in no particular order. Missing items are absent
/// from the result, not errors.
pub(crate) async fn batch_get_items(repo: &Repo, keys: Vec<Item>) -> Result<Vec<Item>, RepoError> {
    let mut items = Vec::with_capacity(keys.len());
    for chunk in keys.chunks(MAX_KEYS_PER_REQUEST) {
        let mut pending = chunk.to_vec();
        let mut attempt = 0;
        while !pending.is_empty() {
            if attempt == MAX_ATTEMPTS {
                return Err(RepoError::Dynamo(format!(
                    "BatchGetItem left {} keys unprocessed after {MAX_ATTEMPTS} attempts",
                    pending.len()
                )));
            }
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(50 * 2u64.pow(attempt))).await;
            }
            attempt += 1;

            let request = KeysAndAttributes::builder()
                .set_keys(Some(pending))
                .build()
                .map_err(|e| RepoError::Dynamo(format!("{e:?}")))?;
            let resp = repo
                .client
                .batch_get_item()
                .request_items(&repo.table, request)
                .send()
                .await?;

            if let Some(mut tables) = resp.responses {
                items.extend(tables.remove(&repo.table).unwrap_or_default());
            }
            pending = resp
                .unprocessed_keys
                .and_then(|mut tables| tables.remove(&repo.table))
                .map(|k| k.keys)
                .unwrap_or_default();
        }
    }
    Ok(items)
}
