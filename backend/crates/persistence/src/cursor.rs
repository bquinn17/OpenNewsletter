//! Opaque pagination cursor: `base64url(JSON(LastEvaluatedKey))`
//! (`plans/03-api-contract.md` §1 — "Cursor is base64url(JSON) encoding
//! DynamoDB's LastEvaluatedKey").
//!
//! Every paginated query in this crate pages the base table by `pk`/`sk`
//! alone, and both attributes are always `AttributeValue::S`
//! (`plans/02-data-model-dynamodb.md` §1), so this only needs to round-trip
//! string-valued keys — not the general DynamoDB attribute-value tree.

use crate::error::RepoError;
use aws_sdk_dynamodb::types::AttributeValue;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use std::collections::HashMap;

/// Encodes a `LastEvaluatedKey` as an opaque cursor string.
pub fn encode(key: &HashMap<String, AttributeValue>) -> Result<String, RepoError> {
    let mut plain: HashMap<&str, &str> = HashMap::with_capacity(key.len());
    for (k, v) in key {
        let AttributeValue::S(s) = v else {
            return Err(RepoError::Serde(format!(
                "cursor key `{k}` is not a string attribute"
            )));
        };
        plain.insert(k.as_str(), s.as_str());
    }
    let json = serde_json::to_vec(&plain).map_err(|e| RepoError::Serde(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

/// Decodes a cursor string back into a `LastEvaluatedKey`. Any malformed
/// input (bad base64, bad JSON, non-string values) is `RepoError::Serde` —
/// callers map that to 422 `VALIDATION_FAILED` on the `cursor` field.
pub fn decode(cursor: &str) -> Result<HashMap<String, AttributeValue>, RepoError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|e| RepoError::Serde(format!("invalid cursor: {e}")))?;
    let plain: HashMap<String, String> = serde_json::from_slice(&bytes)
        .map_err(|e| RepoError::Serde(format!("invalid cursor: {e}")))?;
    Ok(plain
        .into_iter()
        .map(|(k, v)| (k, AttributeValue::S(v)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_string_only_key() {
        let mut key = HashMap::new();
        key.insert("pk".to_owned(), AttributeValue::S("GROUP#g1".into()));
        key.insert(
            "sk".to_owned(),
            AttributeValue::S("C#2026-06-04T00:00:00Z#c1".into()),
        );

        let cursor = encode(&key).unwrap();
        let decoded = decode(&cursor).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn rejects_garbage_input() {
        let err = decode("not-valid-base64url!!!").expect_err("garbage cursor");
        assert!(matches!(err, RepoError::Serde(_)));
    }

    #[test]
    fn rejects_a_non_string_attribute() {
        let mut key = HashMap::new();
        key.insert("n".to_owned(), AttributeValue::N("5".into()));
        let err = encode(&key).expect_err("non-string attribute");
        assert!(matches!(err, RepoError::Serde(_)));
    }
}
