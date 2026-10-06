//! Pure-Rust Web Push crypto: RFC 8291 message encryption
//! (`Content-Encoding: aes128gcm`) and RFC 8292 VAPID JWTs. No network I/O —
//! every function here is cheap to unit test directly (M11 decision D1).

use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::Aes128Gcm;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use rand::RngCore;
use sha2::Sha256;
use thiserror::Error;

/// Single-record `aes128gcm` size (RFC 8188 §2) — comfortably above the
/// largest payload this app ever sends (`shared::config::MAX_PUSH_PAYLOAD_BYTES`).
const RECORD_SIZE: u32 = 4096;
/// RFC 8188 "last record" padding delimiter — every push here is one record.
const LAST_RECORD_DELIMITER: u8 = 0x02;

#[derive(Debug, Error)]
pub enum WebPushError {
    #[error("invalid key: {0}")]
    InvalidKey(String),
    #[error("encryption failed: {0}")]
    Encrypt(String),
}

/// The application server's VAPID keypair, decoded once at cold start and
/// held for the life of the warm container.
#[derive(Debug)]
pub struct VapidKeys {
    public_key: [u8; 65],
    private_key: [u8; 32],
    subject: String,
}

impl VapidKeys {
    /// Parses the raw base64url (unpadded) key material from the
    /// `opennewsletter/vapid/{env}` secret and verifies the private key
    /// actually derives the given public key — a mismatched pair would
    /// otherwise fail silently at send time with a push-service-side
    /// signature error (`07-notifications.md` §2).
    pub fn parse(
        public_key_b64: &str,
        private_key_b64: &str,
        subject: String,
    ) -> Result<Self, WebPushError> {
        let public_key = decode_fixed::<65>(public_key_b64)?;
        let private_key = decode_fixed::<32>(private_key_b64)?;
        let keys = Self {
            public_key,
            private_key,
            subject,
        };
        keys.verify_keypair()?;
        Ok(keys)
    }

    fn verify_keypair(&self) -> Result<(), WebPushError> {
        let secret = SecretKey::from_bytes(&self.private_key.into())
            .map_err(|e| WebPushError::InvalidKey(format!("invalid VAPID private key: {e}")))?;
        let derived = secret.public_key().to_encoded_point(false);
        if derived.as_bytes() != self.public_key {
            return Err(WebPushError::InvalidKey(
                "VAPID private key does not derive the configured public key".into(),
            ));
        }
        Ok(())
    }

    pub fn public_key_b64(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.public_key)
    }
}

fn decode_fixed<const N: usize>(b64: &str) -> Result<[u8; N], WebPushError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(b64)
        .map_err(|e| WebPushError::InvalidKey(format!("invalid base64url: {e}")))?;
    let len = bytes.len();
    bytes
        .try_into()
        .map_err(|_| WebPushError::InvalidKey(format!("expected {N} bytes, got {len}")))
}

/// RFC 8292 VAPID `Authorization` header value: `vapid t={jwt}, k={publicKey}`.
/// `aud` is the scheme+host of `endpoint` (not the full URL — push services
/// reject a JWT audience that includes the path).
pub fn vapid_authorization_header(
    keys: &VapidKeys,
    endpoint: &str,
    now: DateTime<Utc>,
) -> Result<String, WebPushError> {
    let aud = endpoint_origin(endpoint)?;
    let header = URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = serde_json::json!({
        "aud": aud,
        "exp": (now + Duration::hours(12)).timestamp(),
        "sub": keys.subject,
    })
    .to_string();
    let signing_input = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims));

    let signing_key = SigningKey::from_bytes((&keys.private_key).into())
        .map_err(|e| WebPushError::InvalidKey(format!("invalid VAPID signing key: {e}")))?;
    let signature: Signature = signing_key.sign(signing_input.as_bytes());
    let jwt = format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    );

    Ok(format!("vapid t={jwt}, k={}", keys.public_key_b64()))
}

/// `scheme://host[:port]` of `endpoint`, with no path — the JWS `aud` claim
/// RFC 8292 requires.
fn endpoint_origin(endpoint: &str) -> Result<String, WebPushError> {
    let (scheme, rest) = endpoint
        .split_once("://")
        .ok_or_else(|| WebPushError::InvalidKey(format!("endpoint has no scheme: {endpoint}")))?;
    let host = rest.split('/').next().filter(|h| !h.is_empty());
    match host {
        Some(host) => Ok(format!("{scheme}://{host}")),
        None => Err(WebPushError::InvalidKey(format!(
            "endpoint has no host: {endpoint}"
        ))),
    }
}

/// RFC 8291 message encryption. `p256dh_b64`/`auth_b64` are the subscriber's
/// base64url-encoded public key (65-byte uncompressed point) and auth secret
/// (16 bytes) exactly as stored on the `PushSubscription` row. Returns the
/// full `aes128gcm` wire body: `salt(16) || recordSize(4) || keyIdLen(1) ||
/// keyId(65) || ciphertext+tag`.
pub fn encrypt(
    plaintext: &[u8],
    p256dh_b64: &str,
    auth_b64: &str,
) -> Result<Vec<u8>, WebPushError> {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let as_secret = SecretKey::random(&mut rand::thread_rng());
    encrypt_with(plaintext, p256dh_b64, auth_b64, salt, &as_secret)
}

/// Core of [`encrypt`] with the per-message randomness (`salt`, the
/// application server's ephemeral keypair) injected rather than generated —
/// the RFC 8291 Appendix A test vector fixes both, so this is the hook the
/// unit test below uses to reproduce it deterministically.
fn encrypt_with(
    plaintext: &[u8],
    p256dh_b64: &str,
    auth_b64: &str,
    salt: [u8; 16],
    as_secret: &SecretKey,
) -> Result<Vec<u8>, WebPushError> {
    let ua_public_bytes = URL_SAFE_NO_PAD
        .decode(p256dh_b64)
        .map_err(|e| WebPushError::InvalidKey(format!("invalid p256dh: {e}")))?;
    let ua_public = PublicKey::from_sec1_bytes(&ua_public_bytes)
        .map_err(|e| WebPushError::InvalidKey(format!("invalid p256dh point: {e}")))?;
    let auth_secret = URL_SAFE_NO_PAD
        .decode(auth_b64)
        .map_err(|e| WebPushError::InvalidKey(format!("invalid auth secret: {e}")))?;

    let as_public_point = as_secret.public_key().to_encoded_point(false);
    let as_public_bytes = as_public_point.as_bytes();

    let shared = p256::ecdh::diffie_hellman(as_secret.to_nonzero_scalar(), ua_public.as_affine());
    let ecdh_secret = shared.raw_secret_bytes();

    let mut key_info = Vec::with_capacity(14 + ua_public_bytes.len() + as_public_bytes.len());
    key_info.extend_from_slice(b"WebPush: info");
    key_info.push(0);
    key_info.extend_from_slice(&ua_public_bytes);
    key_info.extend_from_slice(as_public_bytes);

    let ikm_hkdf = Hkdf::<Sha256>::new(Some(&auth_secret), ecdh_secret.as_slice());
    let mut ikm = [0u8; 32];
    ikm_hkdf
        .expand(&key_info, &mut ikm)
        .map_err(|e| WebPushError::Encrypt(format!("ikm expand: {e}")))?;

    let record_hkdf = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let mut cek = [0u8; 16];
    record_hkdf
        .expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|e| WebPushError::Encrypt(format!("cek expand: {e}")))?;
    let mut nonce_bytes = [0u8; 12];
    record_hkdf
        .expand(b"Content-Encoding: nonce\0", &mut nonce_bytes)
        .map_err(|e| WebPushError::Encrypt(format!("nonce expand: {e}")))?;

    let mut padded = Vec::with_capacity(plaintext.len() + 1);
    padded.extend_from_slice(plaintext);
    padded.push(LAST_RECORD_DELIMITER);

    let cipher =
        Aes128Gcm::new_from_slice(&cek).map_err(|e| WebPushError::Encrypt(format!("key: {e}")))?;
    let nonce = GenericArray::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, padded.as_ref())
        .map_err(|e| WebPushError::Encrypt(format!("aes-gcm: {e}")))?;

    let mut body = Vec::with_capacity(16 + 4 + 1 + as_public_bytes.len() + ciphertext.len());
    body.extend_from_slice(&salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(as_public_bytes.len() as u8);
    body.extend_from_slice(as_public_bytes);
    body.extend_from_slice(&ciphertext);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 8291 §5 / Appendix A test vector. Keys and salt are fixed inputs
    /// (an application server would normally generate these per message),
    /// injected via [`encrypt_with`] so the output is exactly reproducible.
    #[test]
    fn rfc8291_appendix_a_test_vector() {
        let ua_public_b64 =
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
        let auth_secret_b64 = "BTBZMqHH6r4Tts7J_aSIgg";
        let as_private_b64 = "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw";
        let salt_b64 = "DGv6ra1nlYgDCS1FRnbzlw";
        let plaintext = b"When I grow up, I want to be a watermelon";

        let salt: [u8; 16] = decode_fixed(salt_b64).expect("valid salt");
        let as_private: [u8; 32] = decode_fixed(as_private_b64).expect("valid as private key");
        let as_secret = SecretKey::from_bytes(&as_private.into()).expect("valid scalar");

        let body = encrypt_with(plaintext, ua_public_b64, auth_secret_b64, salt, &as_secret)
            .expect("encryption succeeds");

        // The exact `aes128gcm` body RFC 8291 §5 publishes for these inputs —
        // catches a spec-level mistake (wrong info string, key order, record
        // size) that the self-consistent round trip below would not.
        let expected = URL_SAFE_NO_PAD
            .decode(concat!(
                "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27ml",
                "mlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPT",
                "pK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN",
            ))
            .expect("valid base64url");
        assert_eq!(body, expected);

        // Decrypt from the receiver's side (its own private key + the
        // sender's ephemeral public key embedded in the header) to confirm
        // the derivation chain is correct end-to-end, independent of this
        // test's own encoder.
        let ua_private_b64 = "q1dXpw3UpT5VOmu_cf_v6ih07Aems3njxI-JWgLcM94";
        let ua_private: [u8; 32] = decode_fixed(ua_private_b64).expect("valid ua private key");
        let ua_secret = SecretKey::from_bytes(&ua_private.into()).expect("valid scalar");

        let decrypted = decrypt_for_test(&body, &ua_secret, auth_secret_b64);
        assert_eq!(decrypted, plaintext);
    }

    /// Mirrors [`encrypt_with`] from the receiving side, for the round-trip
    /// assertion above. Not part of the production crate surface — a real
    /// push service does this, not this application.
    fn decrypt_for_test(body: &[u8], ua_secret: &SecretKey, auth_secret_b64: &str) -> Vec<u8> {
        let salt = &body[0..16];
        let key_id_len = body[20] as usize;
        let as_public_bytes = &body[21..21 + key_id_len];
        let ciphertext = &body[21 + key_id_len..];

        let as_public = PublicKey::from_sec1_bytes(as_public_bytes).expect("valid as public key");
        let ua_public_bytes = ua_secret.public_key().to_encoded_point(false);

        let auth_secret = URL_SAFE_NO_PAD.decode(auth_secret_b64).expect("valid auth");

        let shared =
            p256::ecdh::diffie_hellman(ua_secret.to_nonzero_scalar(), as_public.as_affine());
        let ecdh_secret = shared.raw_secret_bytes();

        let mut key_info = Vec::new();
        key_info.extend_from_slice(b"WebPush: info");
        key_info.push(0);
        key_info.extend_from_slice(ua_public_bytes.as_bytes());
        key_info.extend_from_slice(as_public_bytes);

        let ikm_hkdf = Hkdf::<Sha256>::new(Some(&auth_secret), ecdh_secret.as_slice());
        let mut ikm = [0u8; 32];
        ikm_hkdf.expand(&key_info, &mut ikm).expect("ikm expand");

        let record_hkdf = Hkdf::<Sha256>::new(Some(salt), &ikm);
        let mut cek = [0u8; 16];
        record_hkdf
            .expand(b"Content-Encoding: aes128gcm\0", &mut cek)
            .expect("cek expand");
        let mut nonce_bytes = [0u8; 12];
        record_hkdf
            .expand(b"Content-Encoding: nonce\0", &mut nonce_bytes)
            .expect("nonce expand");

        let cipher = Aes128Gcm::new_from_slice(&cek).expect("valid key");
        let nonce = GenericArray::from_slice(&nonce_bytes);
        let mut padded = cipher.decrypt(nonce, ciphertext).expect("decrypts");
        assert_eq!(padded.pop(), Some(LAST_RECORD_DELIMITER));
        padded
    }

    #[test]
    fn vapid_jwt_verifies_with_the_public_key() {
        let private = SecretKey::random(&mut rand::thread_rng());
        let public_point = private.public_key().to_encoded_point(false);
        let keys = VapidKeys::parse(
            &URL_SAFE_NO_PAD.encode(public_point.as_bytes()),
            &URL_SAFE_NO_PAD.encode(private.to_bytes()),
            "mailto:admin@opennewsletter.example.com".to_owned(),
        )
        .expect("valid keypair");

        let now = Utc::now();
        let header =
            vapid_authorization_header(&keys, "https://fcm.googleapis.com/fcm/send/abc", now)
                .expect("builds header");

        let (t_part, k_part) = header
            .strip_prefix("vapid t=")
            .expect("starts with vapid t=")
            .split_once(", k=")
            .expect("has a k= component");
        assert_eq!(k_part, keys.public_key_b64());

        let mut parts = t_part.split('.');
        let header_b64 = parts.next().expect("header segment");
        let claims_b64 = parts.next().expect("claims segment");
        let sig_b64 = parts.next().expect("signature segment");
        assert!(parts.next().is_none(), "exactly three JWT segments");

        let header_json: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(header_b64).unwrap()).unwrap();
        assert_eq!(header_json["typ"], "JWT");
        assert_eq!(header_json["alg"], "ES256");

        let claims_json: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(claims_b64).unwrap()).unwrap();
        assert_eq!(claims_json["aud"], "https://fcm.googleapis.com");
        assert_eq!(
            claims_json["sub"],
            "mailto:admin@opennewsletter.example.com"
        );
        assert!(claims_json["exp"].as_i64().unwrap() > now.timestamp());

        let signature_bytes = URL_SAFE_NO_PAD.decode(sig_b64).unwrap();
        let signature = Signature::from_slice(&signature_bytes).expect("64-byte r||s signature");
        let verifying_key = p256::ecdsa::VerifyingKey::from(
            p256::PublicKey::from_sec1_bytes(public_point.as_bytes()).unwrap(),
        );
        let signing_input = format!("{header_b64}.{claims_b64}");
        use p256::ecdsa::signature::Verifier;
        verifying_key
            .verify(signing_input.as_bytes(), &signature)
            .expect("signature verifies against the public key");
    }

    #[test]
    fn vapid_parse_rejects_a_mismatched_keypair() {
        let private_a = SecretKey::random(&mut rand::thread_rng());
        let public_b = SecretKey::random(&mut rand::thread_rng())
            .public_key()
            .to_encoded_point(false);

        let err = VapidKeys::parse(
            &URL_SAFE_NO_PAD.encode(public_b.as_bytes()),
            &URL_SAFE_NO_PAD.encode(private_a.to_bytes()),
            "mailto:admin@opennewsletter.example.com".to_owned(),
        )
        .expect_err("mismatched keypair");
        assert!(matches!(err, WebPushError::InvalidKey(_)));
    }

    #[test]
    fn endpoint_origin_strips_the_path() {
        assert_eq!(
            endpoint_origin("https://fcm.googleapis.com/fcm/send/abc123").unwrap(),
            "https://fcm.googleapis.com"
        );
        assert_eq!(
            endpoint_origin("https://updates.push.services.mozilla.com:443/wpush/v2/abc").unwrap(),
            "https://updates.push.services.mozilla.com:443"
        );
    }
}
