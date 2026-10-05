//! `GET /media-cookie` (`plans/03-api-contract.md` §9.5,
//! `plans/08-media-uploads.md` §4).

mod common;

use ::media::handlers;
use domain::{ApiErrorCode, GroupId, Role, UserId};
use pretty_assertions::assert_eq;
use rsa::{Pkcs1v15Sign, RsaPublicKey};
use sha1::{Digest, Sha1};

/// Reverses [`cloudfront::modified_base64_encode`].
fn decode_modified_base64(s: &str) -> Vec<u8> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    let standard = s.replace('-', "+").replace('_', "=").replace('~', "/");
    STANDARD.decode(standard).expect("valid modified base64")
}

/// A minimal CloudFront policy "Resource" matcher: only the trailing `*`
/// wildcard this app ever emits.
fn resource_matches(pattern: &str, url: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => url.starts_with(prefix),
        None => pattern == url,
    }
}

fn resource_of(policy_json: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(policy_json).expect("policy is valid JSON");
    value["Statement"][0]["Resource"]
        .as_str()
        .expect("Resource is a string")
        .to_owned()
}

#[tokio::test]
async fn non_member_is_forbidden() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcookiefb", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let state = common::test_state(repo);

    let err =
        handlers::get_media_cookie(&state, &UserId::new("intruder"), &GroupId::new("gcookiefb"))
            .await
            .expect_err("non-member must be refused");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn policy_resource_is_scoped_to_the_requested_group() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("groupA", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let state = common::test_state(repo);

    let cookie = handlers::get_media_cookie(&state, &UserId::new("u1"), &GroupId::new("groupA"))
        .await
        .expect("member can get a cookie");

    let policy_json = String::from_utf8(decode_modified_base64(&cookie.body.policy)).unwrap();
    let resource = resource_of(&policy_json);
    assert_eq!(resource, "https://cdn.example.com/img/groupA/*");
}

#[tokio::test]
async fn signature_verifies_against_the_matching_public_key() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gverify", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let state = common::test_state(repo);

    let cookie = handlers::get_media_cookie(&state, &UserId::new("u1"), &GroupId::new("gverify"))
        .await
        .unwrap();

    let policy_bytes = decode_modified_base64(&cookie.body.policy);
    let signature_bytes = decode_modified_base64(&cookie.body.signature);
    let public_key = RsaPublicKey::from(&common::test_signing_key());
    let digest = Sha1::digest(&policy_bytes);

    public_key
        .verify(Pkcs1v15Sign::new::<Sha1>(), &digest, &signature_bytes)
        .expect("signature verifies against the matching public key");
}

#[tokio::test]
async fn a_group_a_cookie_does_not_scope_to_a_group_b_url() {
    let (_c, repo) = common::make_repo().await;
    let group_a = common::group("groupA2", "u1");
    common::seed_group_and_membership(&repo, &group_a, "u1", Role::Member).await;
    let state = common::test_state(repo);

    let cookie = handlers::get_media_cookie(&state, &UserId::new("u1"), &GroupId::new("groupA2"))
        .await
        .unwrap();
    let policy_json = String::from_utf8(decode_modified_base64(&cookie.body.policy)).unwrap();
    let resource = resource_of(&policy_json);

    let group_a_url = "https://cdn.example.com/img/groupA2/2026/q1/u1/img1/display.webp";
    let group_b_url = "https://cdn.example.com/img/groupB2/2026/q1/u1/img1/display.webp";

    assert!(resource_matches(&resource, group_a_url));
    assert!(!resource_matches(&resource, group_b_url));
}

#[tokio::test]
async fn set_cookie_headers_are_emitted_when_a_cookie_domain_is_configured() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gwithcookie", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let state = common::test_state(repo);

    let cookie =
        handlers::get_media_cookie(&state, &UserId::new("u1"), &GroupId::new("gwithcookie"))
            .await
            .unwrap();

    assert_eq!(cookie.set_cookie_headers.len(), 3);
    assert!(cookie.set_cookie_headers[0].starts_with("CloudFront-Policy="));
    assert!(cookie.set_cookie_headers[1].starts_with("CloudFront-Signature="));
    assert!(cookie.set_cookie_headers[2].starts_with("CloudFront-Key-Pair-Id=KTESTKEYPAIR;"));
    for header in &cookie.set_cookie_headers {
        assert!(header.contains("Domain=.example.com"));
        assert!(header.contains("Path=/img/gwithcookie/"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("Secure"));
        assert!(header.contains("SameSite=None"));
    }
}

#[tokio::test]
async fn no_set_cookie_headers_when_the_cookie_domain_is_empty() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnocookie", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let state = common::test_state_without_cookie_domain(repo);

    let cookie = handlers::get_media_cookie(&state, &UserId::new("u1"), &GroupId::new("gnocookie"))
        .await
        .unwrap();

    assert!(cookie.set_cookie_headers.is_empty());
}
