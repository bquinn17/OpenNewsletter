//! `POST /avatars`, `GET /avatars/{avatarId}`, `DELETE /avatars/{avatarId}`
//! (`docs/design/api.md` §9.7).

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
    let avatar_url = response.avatar_url.expect("ready avatar has a url");
    assert!(
        avatar_url.starts_with(&format!(
            "https://{}.s3.us-east-1.amazonaws.com/avatar/av-01/display.webp?",
            common::MEDIA_PROCESSED
        )),
        "{avatar_url}"
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
}

#[tokio::test]
async fn deleting_another_user_s_avatar_touches_nothing_of_theirs() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-02b", MediaStatus::Ready).await;
    let (state, fakes) = common::test_state_with_fakes(repo);

    // The lookup is under the caller's own partition, so this is the same
    // no-op as deleting an already-deleted avatar of their own.
    handlers::delete_avatar(&state, &UserId::new("u2"), &avatar_id)
        .await
        .expect("a missing avatar is a no-op success");

    assert!(fakes.store.deleted().await.is_empty());
    handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("the owner's avatar is untouched");
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
}

#[tokio::test]
async fn delete_avatar_removes_the_objects_and_deletes_the_row() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-04", MediaStatus::Ready).await;
    let (state, fakes) = common::test_state_with_fakes(repo);

    handlers::delete_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("owner can delete their avatar");

    assert_eq!(
        fakes.store.deleted().await,
        vec![
            (
                common::AVATARS_ORIGINALS.to_owned(),
                "uploads/u1/av-04.jpg".to_owned()
            ),
            (
                common::MEDIA_PROCESSED.to_owned(),
                "avatar/av-04/display.webp".to_owned()
            ),
        ]
    );
    let err = handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect_err("the row is gone");
    assert_eq!(err.code, ApiErrorCode::NotFound);

    handlers::delete_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("a repeat delete is a no-op success");
    assert_eq!(fakes.store.deleted().await.len(), 2);
}

#[tokio::test]
async fn a_failed_avatar_delete_is_a_500_and_keeps_the_row() {
    let (_c, repo) = common::make_repo().await;
    let avatar_id = common::seed_avatar(&repo, "u1", "av-05", MediaStatus::Ready).await;
    let (state, fakes) = common::test_state_with_fakes(repo);
    fakes.store.set_failing(true);

    let err = handlers::delete_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect_err("an S3 failure fails the delete");

    assert_eq!(err.code, ApiErrorCode::Internal);
    handlers::get_avatar(&state, &UserId::new("u1"), &avatar_id)
        .await
        .expect("the row is kept for a retry");
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
