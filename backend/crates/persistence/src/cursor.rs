//! Opaque pagination cursor: `base64url(JSON(LastEvaluatedKey))`
//! (`plans/03-api-contract.md` §1 — "Cursor is base64url(JSON) encoding
//! DynamoDB's LastEvaluatedKey").
//!
//! Every paginated query in this crate pages the base table by `pk`/`sk`
//! alone, and both attributes are always `AttributeValue::S`
//! (`plans/02-data-model-dynamodb.md` §1), so this only needs to round-trip
//! string-valued keys — not the general DynamoDB attribute-value tree.

use crate::error::RepoError;
use crate::keys::attr;
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

/// [`decode`], plus a check that the key addresses a row inside the query
/// it's resuming: exactly `pk`/`sk`, `pk` equal to `pk`, `sk` under
/// `sk_prefix`. A well-formed cursor from another partition (or a
/// hand-crafted one) would otherwise reach DynamoDB, which rejects it with a
/// `ValidationException` the caller can't tell apart from a real failure.
pub fn decode_scoped(
    cursor: &str,
    pk: &str,
    sk_prefix: &str,
) -> Result<HashMap<String, AttributeValue>, RepoError> {
    let key = decode(cursor)?;
    let in_scope = key.len() == 2
        && matches!(key.get(attr::PK), Some(AttributeValue::S(p)) if p == pk)
        && matches!(key.get(attr::SK), Some(AttributeValue::S(s)) if s.starts_with(sk_prefix));
    if !in_scope {
        return Err(RepoError::Serde(
            "invalid cursor: not from this listing".into(),
        ));
    }
    Ok(key)
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

    fn cursor_for(pairs: &[(&str, &str)]) -> String {
        let key = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), AttributeValue::S((*v).to_owned())))
            .collect();
        encode(&key).unwrap()
    }

    #[test]
    fn decode_scoped_accepts_a_key_inside_the_partition() {
        let cursor = cursor_for(&[("pk", "ENG#a"), ("sk", "C#2026#c1")]);
        assert!(decode_scoped(&cursor, "ENG#a", "C#").is_ok());
    }

    #[test]
    fn decode_scoped_rejects_keys_outside_the_partition() {
        let cases = [
            cursor_for(&[("pk", "ENG#b"), ("sk", "C#2026#c1")]),
            cursor_for(&[("pk", "ENG#a"), ("sk", "R#u1#x")]),
            cursor_for(&[("pk", "ENG#a")]),
            cursor_for(&[("pk", "ENG#a"), ("sk", "C#2026#c1"), ("extra", "x")]),
            cursor_for(&[]),
        ];
        for cursor in cases {
            let err = decode_scoped(&cursor, "ENG#a", "C#").expect_err("out-of-scope cursor");
            assert!(matches!(err, RepoError::Serde(_)));
        }
    }

    #[test]
    fn rejects_a_non_string_attribute() {
        let mut key = HashMap::new();
        key.insert("n".to_owned(), AttributeValue::N("5".into()));
        let err = encode(&key).expect_err("non-string attribute");
        assert!(matches!(err, RepoError::Serde(_)));
    }
}
