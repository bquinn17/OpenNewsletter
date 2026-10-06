//! Request validation for the push routes (`plans/03-api-contract.md` §10,
//! M11 decision D8).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use domain::ApiError;

const MAX_ENDPOINT_CHARS: usize = 2048;
const MAX_USER_AGENT_CHARS: usize = 512;
const P256DH_DECODED_BYTES: usize = 65;
const AUTH_DECODED_BYTES: usize = 16;

/// `endpoint` must be an `https://` URL of at most 2048 characters.
pub fn endpoint(raw: &str) -> Result<String, ApiError> {
    if !raw.starts_with("https://") {
        return Err(ApiError::invalid_field(
            "endpoint",
            "endpoint must be an https:// URL",
        ));
    }
    if raw.chars().count() > MAX_ENDPOINT_CHARS {
        return Err(ApiError::invalid_field(
            "endpoint",
            format!("endpoint must be at most {MAX_ENDPOINT_CHARS} characters"),
        ));
    }
    Ok(raw.to_owned())
}

/// `keys.p256dh` must base64url-decode to a 65-byte uncompressed P-256 point.
/// Returned unpadded — the form `webpush::encrypt` decodes.
pub fn p256dh(raw: &str) -> Result<String, ApiError> {
    let unpadded = raw.trim_end_matches('=');
    decode_fixed_len(unpadded, P256DH_DECODED_BYTES, "keys.p256dh")?;
    Ok(unpadded.to_owned())
}

/// `keys.auth` must base64url-decode to 16 bytes. Returned unpadded.
pub fn auth(raw: &str) -> Result<String, ApiError> {
    let unpadded = raw.trim_end_matches('=');
    decode_fixed_len(unpadded, AUTH_DECODED_BYTES, "keys.auth")?;
    Ok(unpadded.to_owned())
}

fn decode_fixed_len(raw: &str, expected: usize, field: &str) -> Result<(), ApiError> {
    let decoded = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| ApiError::invalid_field(field, format!("{field} must be valid base64url")))?;
    if decoded.len() != expected {
        return Err(ApiError::invalid_field(
            field,
            format!(
                "{field} must decode to {expected} bytes, got {}",
                decoded.len()
            ),
        ));
    }
    Ok(())
}

/// A missing `userAgent` becomes `""`; present-but-overlong is a 422.
pub fn user_agent(raw: Option<&str>) -> Result<String, ApiError> {
    let ua = raw.unwrap_or_default();
    if ua.chars().count() > MAX_USER_AGENT_CHARS {
        return Err(ApiError::invalid_field(
            "userAgent",
            format!("userAgent must be at most {MAX_USER_AGENT_CHARS} characters"),
        ));
    }
    Ok(ua.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ApiErrorCode;

    #[test]
    fn endpoint_requires_https() {
        let err = endpoint("http://example.com/push").expect_err("not https");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        assert_eq!(err.field_errors[0].field, "endpoint");
    }

    #[test]
    fn endpoint_rejects_overlong_values() {
        let long = format!("https://example.com/{}", "a".repeat(MAX_ENDPOINT_CHARS));
        assert!(endpoint(&long).is_err());
    }

    #[test]
    fn endpoint_accepts_a_valid_https_url() {
        assert_eq!(
            endpoint("https://fcm.googleapis.com/fcm/send/abc").unwrap(),
            "https://fcm.googleapis.com/fcm/send/abc"
        );
    }

    #[test]
    fn p256dh_requires_sixty_five_decoded_bytes() {
        let too_short = URL_SAFE_NO_PAD.encode([0u8; 64]);
        assert!(p256dh(&too_short).is_err());
        let right = URL_SAFE_NO_PAD.encode([0u8; 65]);
        assert!(p256dh(&right).is_ok());
    }

    #[test]
    fn auth_requires_sixteen_decoded_bytes() {
        let too_short = URL_SAFE_NO_PAD.encode([0u8; 15]);
        assert!(auth(&too_short).is_err());
        let right = URL_SAFE_NO_PAD.encode([0u8; 16]);
        assert!(auth(&right).is_ok());
    }

    #[test]
    fn keys_accept_padded_base64url_and_store_it_unpadded() {
        let padded = format!("{}==", URL_SAFE_NO_PAD.encode([7u8; 16]));
        assert_eq!(auth(&padded).unwrap(), URL_SAFE_NO_PAD.encode([7u8; 16]));
        let padded = format!("{}=", URL_SAFE_NO_PAD.encode([4u8; 65]));
        assert_eq!(p256dh(&padded).unwrap(), URL_SAFE_NO_PAD.encode([4u8; 65]));
    }

    #[test]
    fn user_agent_defaults_to_empty_when_missing() {
        assert_eq!(user_agent(None).unwrap(), "");
    }

    #[test]
    fn user_agent_rejects_overlong_values() {
        let long = "a".repeat(MAX_USER_AGENT_CHARS + 1);
        assert!(user_agent(Some(&long)).is_err());
    }
}
