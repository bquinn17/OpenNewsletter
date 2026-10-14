//! Request validation for the push routes (`docs/design/api.md` §10,
//! M11 decision D8).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use domain::ApiError;
use shared::config::PUSH_ENDPOINT_HOST_SUFFIXES;

const MAX_ENDPOINT_CHARS: usize = 2048;
const MAX_USER_AGENT_CHARS: usize = 512;
const P256DH_DECODED_BYTES: usize = 65;
const AUTH_DECODED_BYTES: usize = 16;

/// `endpoint` must be an `https://` URL of at most 2048 characters, whose
/// host is a known Web Push service (`shared::config::PUSH_ENDPOINT_HOST_SUFFIXES`).
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
    if !has_allowed_host(raw) {
        return Err(unknown_service());
    }
    Ok(raw.to_owned())
}

/// Parses `raw` and checks its host against the allowlist by exact match or
/// `.`-prefixed suffix match on the parsed host — never a substring check
/// against the raw string, so a userinfo trick like
/// `https://fcm.googleapis.com@evil.com/` is judged on `evil.com`, the host
/// the browser and any HTTP client would actually connect to.
fn has_allowed_host(raw: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    // Push services listen on 443; any other port would turn the allowlist
    // into a port scanner against those hosts. `port()` is `None` for 443.
    if url.port().is_some() {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    PUSH_ENDPOINT_HOST_SUFFIXES.iter().any(|allowed| {
        if allowed.starts_with('.') {
            // The leading dot is part of the match, so a suffix like
            // ".push.services.mozilla.com" cannot match
            // "evilpush.services.mozilla.com" — only a real subdomain.
            host.ends_with(allowed)
        } else {
            host == *allowed
        }
    })
}

fn unknown_service() -> ApiError {
    ApiError::invalid_field("endpoint", "endpoint must be a known Web Push service")
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
        let long = format!(
            "https://fcm.googleapis.com/{}",
            "a".repeat(MAX_ENDPOINT_CHARS)
        );
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
    fn endpoint_accepts_every_allowlisted_push_service() {
        for url in [
            "https://fcm.googleapis.com/fcm/send/abc",
            "https://updates.push.services.mozilla.com/wpush/v2/abc",
            "https://foo.push.services.mozilla.com/wpush/v2/abc",
            "https://web.push.apple.com/abc",
            "https://sg2p.notify.windows.com/abc",
            "https://some-id.push.apple.com/3/device/abc",
        ] {
            endpoint(url).unwrap_or_else(|e| panic!("{url} should be allowed: {e:?}"));
        }
    }

    #[test]
    fn endpoint_rejects_a_host_outside_the_allowlist() {
        let err = endpoint("https://evil.com/push").expect_err("unknown service");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        assert_eq!(err.field_errors[0].field, "endpoint");
        assert_eq!(
            err.field_errors[0].message,
            "endpoint must be a known Web Push service"
        );
    }

    #[test]
    fn endpoint_rejects_an_allowlisted_name_used_as_a_subdomain_of_an_attacker_host() {
        let err = endpoint("https://fcm.googleapis.com.evil.com/push")
            .expect_err("suffix trick must not match");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn endpoint_rejects_a_userinfo_trick() {
        // The browser/HTTP client connects to `evil.com`; `fcm.googleapis.com`
        // here is just a (discarded) username, not the host.
        let err = endpoint("https://fcm.googleapis.com@evil.com/push")
            .expect_err("host is evil.com, not fcm.googleapis.com");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    }

    #[test]
    fn endpoint_rejects_a_non_default_port() {
        let err = endpoint("https://fcm.googleapis.com:8443/fcm/send/abc")
            .expect_err("push services only listen on 443");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
        endpoint("https://fcm.googleapis.com:443/fcm/send/abc")
            .expect("an explicit default port is the same URL");
    }

    #[test]
    fn endpoint_rejects_a_bare_subdomain_without_the_dot_boundary() {
        // Must not match the ".push.apple.com" suffix via a bare substring —
        // there is no dot before "push" here.
        let err = endpoint("https://evilpush.apple.com/push").expect_err("not a real subdomain");
        assert_eq!(err.code, ApiErrorCode::ValidationFailed);
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
