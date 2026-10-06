//! Request validation for the engagement routes (`plans/03-api-contract.md` §8).

use domain::{ApiError, ImageId};
use shared::config::{DEFAULT_COMMENT_LIST_LIMIT, MAX_COMMENT_BODY_CHARS, MAX_LIST_LIMIT};

/// `body` 0-2000 chars, counted in chars the same way
/// `lambda-responses::validation::body` counts its own limit
/// (`09-engagement.md` §1.3).
pub fn body(raw: &str) -> Result<String, ApiError> {
    if raw.chars().count() > MAX_COMMENT_BODY_CHARS {
        return Err(ApiError::invalid_field(
            "body",
            format!("body must be at most {MAX_COMMENT_BODY_CHARS} characters"),
        ));
    }
    Ok(raw.to_owned())
}

/// A comment needs a non-whitespace body or an attached image
/// (`09-engagement.md` §1.3). Checked both at create and after a patch is
/// applied, since a patch can clear either side.
pub fn require_body_or_image(
    body: Option<&str>,
    image_media_id: Option<&ImageId>,
) -> Result<(), ApiError> {
    let has_body = body.is_some_and(|b| !b.trim().is_empty());
    if has_body || image_media_id.is_some() {
        Ok(())
    } else {
        Err(ApiError::invalid_field(
            "body",
            "a comment needs a non-empty body or an attached image",
        ))
    }
}

/// `?limit=` for `GET .../comments` — default 50, 1..100
/// (`09-engagement.md` §8 #9).
pub fn list_limit(raw: Option<&str>) -> Result<i32, ApiError> {
    let n = match raw {
        None => return Ok(DEFAULT_COMMENT_LIST_LIMIT as i32),
        Some(s) => s.parse::<u32>().map_err(|_| {
            ApiError::invalid_field(
                "limit",
                format!("limit must be a positive integer, got `{s}`"),
            )
        })?,
    };
    if (1..=MAX_LIST_LIMIT).contains(&n) {
        Ok(n as i32)
    } else {
        Err(ApiError::invalid_field(
            "limit",
            format!("limit must be between 1 and {MAX_LIST_LIMIT}, got `{n}`"),
        ))
    }
}

/// Percent-decodes (if the raw path segment still contains `%` — API Gateway
/// HTTP APIs don't reliably decode path params), NFC-normalizes, and
/// validates an emoji path param against the canonical predicate
/// (`09-engagement.md` §2.2). Returns the normalized, storage-ready form.
pub fn emoji(raw: &str) -> Result<String, ApiError> {
    let decoded = if raw.contains('%') {
        percent_decode(raw)?
    } else {
        raw.to_owned()
    };
    let normalized = domain::engagement::normalize_emoji(&decoded);
    if domain::engagement::is_valid_emoji(&normalized) {
        Ok(normalized)
    } else {
        Err(ApiError::invalid_field(
            "emoji",
            "emoji must be 1-12 codepoints including at least one pictographic or \
             regional-indicator codepoint",
        ))
    }
}

/// Minimal `%XX` percent-decoding — the only path-param shape this API ever
/// needs to undo (`shared::http::query_param` skips this for query strings
/// for the same reason: everything else this API defines is already
/// URL-safe). A malformed escape is left as literal bytes; the result then
/// fails UTF-8 or emoji validation and comes back as 422 either way.
fn percent_decode(raw: &str) -> Result<String, ApiError> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .map_err(|_| ApiError::invalid_field("emoji", "emoji is not valid percent-encoded UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ApiErrorCode;

    #[test]
    fn body_within_limit_is_accepted() {
        assert_eq!(body("hello").unwrap(), "hello");
        assert_eq!(body("").unwrap(), "");
    }

    #[test]
    fn overlong_body_is_rejected() {
        let err = body(&"a".repeat(MAX_COMMENT_BODY_CHARS + 1)).expect_err("too long");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn whitespace_only_body_with_no_image_is_rejected() {
        let err = require_body_or_image(Some("   "), None).expect_err("blank body, no image");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn missing_body_with_an_image_is_accepted() {
        let image_id = ImageId::new("img-1");
        require_body_or_image(None, Some(&image_id)).expect("image alone is enough");
    }

    #[test]
    fn non_blank_body_with_no_image_is_accepted() {
        require_body_or_image(Some("hi"), None).expect("body alone is enough");
    }

    #[test]
    fn default_limit_is_fifty() {
        assert_eq!(list_limit(None).unwrap(), 50);
    }

    #[test]
    fn limit_out_of_range_is_rejected() {
        assert_eq!(
            list_limit(Some("0")).unwrap_err().code,
            ApiErrorCode::ValidationFailed
        );
        assert_eq!(
            list_limit(Some("101")).unwrap_err().code,
            ApiErrorCode::ValidationFailed
        );
        assert_eq!(
            list_limit(Some("abc")).unwrap_err().code,
            ApiErrorCode::ValidationFailed
        );
    }

    #[test]
    fn limit_within_range_is_accepted() {
        assert_eq!(list_limit(Some("1")).unwrap(), 1);
        assert_eq!(list_limit(Some("100")).unwrap(), 100);
    }

    #[test]
    fn plain_emoji_path_param_is_accepted() {
        assert_eq!(emoji("🔥").unwrap(), "🔥");
    }

    #[test]
    fn percent_encoded_emoji_path_param_is_decoded() {
        // %F0%9F%94%A5 is the UTF-8 encoding of 🔥.
        assert_eq!(emoji("%F0%9F%94%A5").unwrap(), "🔥");
    }

    #[test]
    fn ascii_path_param_is_rejected() {
        let err = emoji("a").expect_err("not an emoji");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        assert_eq!(err.field_errors[0].field, "emoji");
    }
}
