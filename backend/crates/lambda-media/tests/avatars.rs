//! `POST /avatars`, `GET /avatars/{avatarId}`, `DELETE /avatars/{avatarId}`
//! (`plans/03-api-contract.md` §9.7).

mod common;

use ::media::handlers;
use domain::api::CreateAvatarRequest;
use domain::{ApiErrorCode, AvatarId, MediaStatus, UserId};
use persistence::users;
use pretty_assertions::assert_eq;

fn avatar_request(mime_type: &str, byte_size: u64) -> CreateAvatarRequest {
    CreateAvatarRequest {
        mime_type: mime_type.into(),
        byte_size,
        sha256: None,
    }
}

#[tokio::test]
async fn create_avatar_returns_a_presigned_put() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo);

    let response = handlers::create_avatar(
        &state,
        &UserId::new("u1"),
        avatar_request("image/png", 204_800),
    )
    .await
    .expect("avatar presign succeeds");

    assert!(response.upload_url.contains("test-avatars-originals"));
    assert!(response.upload_url.contains("X-Amz-Signature"));
    assert_eq!(
        response.headers.get("content-type").map(String::as_str),
        Some("image/png")
    );
    assert_eq!(response.expires_in_seconds, 600);
}

#[tokio::test]
async fn gif_avatar_is_rejected_as_bad_type() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo);

    let err = handlers::create_avatar(
        &state,
        &UserId::new("u1"),
        avatar_request("image/gif", 10_000),
    )
    .await
    .expect_err("gif is not an allowed avatar type");
    assert_eq!(err.code, ApiErrorCode::ImageBadType);
}

#[tokio::test]
async fn oversized_avatar_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let state = common::test_state(repo);

    let six_mb = 6 * 1024 * 1024;
    let err = handlers::create_avatar(
        &state,
        &UserId::new("u1"),
        avatar_request("image/jpeg", six_mb),
    )
    .await
    .expect_err("6 MB is over the 5 MB avatar cap");
    assert_eq!(err.code, ApiErrorCode::ImageTooLarge);
}

#[tokio::test]
async fn get_avatar_returns_the_caller_s_own_avatar() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-01", MediaStatus::Ready).await;
    let state = common::test_state(repo);

    let response = handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("owner can read their avatar");
    assert_eq!(response.avatar_id, avatar_id);
    assert_eq!(
        response.avatar_url,
        Some("https://cdn.example.com/avatar/av-01/display.webp".into())
    );
}

#[tokio::test]
async fn pending_avatar_has_no_url_yet() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-pending", MediaStatus::Pending).await;
    let state = common::test_state(repo);

    let response = handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .unwrap();
    assert_eq!(response.avatar_url, None);
}

#[tokio::test]
async fn another_user_s_avatar_is_not_found() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-02", MediaStatus::Ready).await;
    let state = common::test_state(repo);

    let err = handlers::get_avatar(&state, &UserId::new("u2"), &avatar_id)
        .await
        .expect_err("another user's avatar must 404");
    assert_eq!(err.code, ApiErrorCode::NotFound);

    let delete_err = handlers::delete_avatar(&state, &UserId::new("u2"), &avatar_id)
        .await
        .expect_err("another user can't delete it either");
    assert_eq!(delete_err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn delete_avatar_clears_the_user_s_avatar_media_id() {
    let (_c, repo) = common::make_repo().await;
    let mut u = common::user("u1");
    let avatar_id = AvatarId::new("av-03");
    u.avatar_media_id = Some(avatar_id.clone());
    users::put_user(&repo, &u).await.unwrap();
    common::seed_avatar(&repo, "u1", "av-03", MediaStatus::Ready).await;
    let state = common::test_state(repo);

    handlers::delete_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("owner can delete their avatar");

    let got = users::get_user(&state.repo, &UserId::new("u1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.avatar_media_id, None);

    let after = handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("avatar row is still readable, just marked failed");
    assert_eq!(after.status, MediaStatus::Failed);
    assert_eq!(after.error_message, Some("DELETED".into()));

    // Idempotent.
    handlers::delete_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("delete is idempotent");
}

#[tokio::test]
async fn delete_avatar_does_not_clear_a_different_current_avatar() {
    let (_c, repo) = common::make_repo().await;
    let mut u = common::user("u1");
    let current = AvatarId::new("av-current");
    u.avatar_media_id = Some(current.clone());
    users::put_user(&repo, &u).await.unwrap();
    let stale_id = common::seed_avatar(&repo, "u1", "av-stale", MediaStatus::Ready).await;
    let state = common::test_state(repo);

    handlers::delete_avatar(&state, &UserId::new("u1"), &stale_id)
        .await
        .expect("deleting a non-current avatar still succeeds");

    let got = users::get_user(&state.repo, &UserId::new("u1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.avatar_media_id, Some(current));
}
