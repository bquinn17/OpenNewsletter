//! Pure CloudFront signed-cookie/URL helpers (`plans/08-media-uploads.md` §4,
//! `plans/03-api-contract.md` §9.5). No network I/O — the private key and the
//! policy string are both passed in, so every function here is cheap to unit
//! test directly without a Secrets Manager call or a live CloudFront
//! distribution.

use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::{Pkcs1v15Sign, RsaPrivateKey};
use sha1::{Digest, Sha1};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CloudFrontError {
    #[error("failed to parse RSA private key: {0}")]
    InvalidKey(String),
    #[error("failed to sign policy: {0}")]
    SignFailed(String),
}

/// Parses an RSA private key PEM, accepting either PKCS#1
/// (`-----BEGIN RSA PRIVATE KEY-----`) or PKCS#8
/// (`-----BEGIN PRIVATE KEY-----`) — CloudFront key-pair exports and
/// hand-generated keys show up in either form, and the Secrets Manager
/// secret just stores whatever PEM the operator pasted in.
pub fn parse_private_key(pem: &str) -> Result<RsaPrivateKey, CloudFrontError> {
    RsaPrivateKey::from_pkcs1_pem(pem)
        .or_else(|_| RsaPrivateKey::from_pkcs8_pem(pem))
        .map_err(|e| CloudFrontError::InvalidKey(e.to_string()))
}

/// The exact, whitespace-free CloudFront custom policy for a signed
/// cookie/URL scoped to one resource pattern (`08-media-uploads.md` §4.3).
/// CloudFront verifies the signature over these literal bytes, so field
/// order and the absence of whitespace are load-bearing, not stylistic.
pub fn policy_json(resource: &str, expires_at_epoch_seconds: i64) -> String {
    format!(
        r#"{{"Statement":[{{"Resource":"{resource}","Condition":{{"DateLessThan":{{"AWS:EpochTime":{expires_at_epoch_seconds}}}}}}}]}}"#
    )
}

/// RSA PKCS#1 v1.5 signature over the SHA-1 digest of `policy` — the scheme
/// CloudFront's signed cookies and signed URLs both require.
pub fn sign_policy(key: &RsaPrivateKey, policy: &[u8]) -> Result<Vec<u8>, CloudFrontError> {
    let digest = Sha1::digest(policy);
    key.sign(Pkcs1v15Sign::new::<Sha1>(), &digest)
        .map_err(|e| CloudFrontError::SignFailed(e.to_string()))
}

/// CloudFront's modified base64 alphabet: standard base64, then `+`→`-`,
/// `=`→`_`, `/`→`~` (`08-media-uploads.md` §4.3).
pub fn modified_base64_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD
        .encode(bytes)
        .replace('+', "-")
        .replace('=', "_")
        .replace('/', "~")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A throwaway 2048-bit test key, PKCS#1 form. Generated for this test
    // suite only — never used outside it.
    const TEST_KEY_PKCS1_PEM: &str = include_str!("../tests/fixtures/test_signing_key.pem");

    #[test]
    fn modified_base64_replaces_the_three_unsafe_characters() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        // A single zero byte standard-base64-encodes to "AA==" — exercises
        // the '=' padding replacement.
        let standard = STANDARD.encode([0u8]);
        assert_eq!(standard, "AA==");
        assert_eq!(modified_base64_encode(&[0]), "AA__");

        // Bytes chosen so standard base64 of them contains both '+' and '/'.
        let bytes = [0xfb, 0xff, 0xbf];
        let standard = STANDARD.encode(bytes);
        assert!(standard.contains('+'));
        assert!(standard.contains('/'));
        let expected = standard
            .replace('+', "-")
            .replace('=', "_")
            .replace('/', "~");

        let encoded = modified_base64_encode(&bytes);
        assert_eq!(encoded, expected);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn policy_json_has_no_whitespace_and_the_exact_field_order() {
        let policy = policy_json("https://cdn.example.com/img/g1/*", 1_800_000_000);
        assert_eq!(
            policy,
            r#"{"Statement":[{"Resource":"https://cdn.example.com/img/g1/*","Condition":{"DateLessThan":{"AWS:EpochTime":1800000000}}}]}"#
        );
        assert!(!policy.contains(' '));
        assert!(!policy.contains('\n'));
    }

    #[test]
    fn parses_a_pkcs1_pem_key() {
        parse_private_key(TEST_KEY_PKCS1_PEM).expect("valid PKCS#1 PEM");
    }

    #[test]
    fn parses_a_pkcs8_pem_key() {
        let key = parse_private_key(TEST_KEY_PKCS1_PEM).expect("valid PKCS#1 PEM");
        let pkcs8_pem = {
            use rsa::pkcs8::EncodePrivateKey;
            key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
                .expect("key re-encodes as PKCS#8")
        };
        parse_private_key(&pkcs8_pem).expect("valid PKCS#8 PEM");
    }

    #[test]
    fn rejects_garbage_input() {
        let err = parse_private_key("not a key").expect_err("garbage must fail to parse");
        assert!(matches!(err, CloudFrontError::InvalidKey(_)));
    }

    #[test]
    fn sign_then_verify_round_trips() {
        let key = parse_private_key(TEST_KEY_PKCS1_PEM).expect("valid PKCS#1 PEM");
        let public = rsa::RsaPublicKey::from(&key);
        let policy = policy_json("https://cdn.example.com/img/g1/*", 1_800_000_000);

        let signature = sign_policy(&key, policy.as_bytes()).expect("signs");

        let digest = Sha1::digest(policy.as_bytes());
        public
            .verify(Pkcs1v15Sign::new::<Sha1>(), &digest, &signature)
            .expect("signature verifies against the matching public key");
    }

    #[test]
    fn signature_does_not_verify_against_a_different_policy() {
        let key = parse_private_key(TEST_KEY_PKCS1_PEM).expect("valid PKCS#1 PEM");
        let public = rsa::RsaPublicKey::from(&key);
        let policy_a = policy_json("https://cdn.example.com/img/groupA/*", 1_800_000_000);
        let policy_b = policy_json("https://cdn.example.com/img/groupB/*", 1_800_000_000);

        let signature = sign_policy(&key, policy_a.as_bytes()).expect("signs");

        let digest_b = Sha1::digest(policy_b.as_bytes());
        public
            .verify(Pkcs1v15Sign::new::<Sha1>(), &digest_b, &signature)
            .expect_err("a signature for policy A must not verify against policy B");
    }
}
