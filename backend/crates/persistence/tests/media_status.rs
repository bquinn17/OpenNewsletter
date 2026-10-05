mod common;

use chrono::{TimeZone, Utc};
use domain::{AvatarId, CycleId, GroupId, ImageId, MediaStatus, UserId};
use persistence::media::{self, get_avatar, get_image};
use persistence::media_status::{
    mark_avatar_failed, mark_avatar_ready, mark_image_failed, mark_image_ready, ImageReady,
    UpdateOutcome,
};
use pretty_assertions::assert_eq;

fn processed_at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()
}

#[tokio::test]
async fn it_marks_a_pending_image_ready_with_dimensions_and_keys() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("g1", "202606", "img-01", "u1");
    media::put_image(&repo, &img).await.unwrap();

    let outcome = mark_image_ready(
        &repo,
        &ImageReady {
            group_id: &GroupId::new("g1"),
            cycle_id: &CycleId::new("202606"),
            image_id: &ImageId::new("img-01"),
            width: 1200,
            height: 800,
            display_key: "img/g1/202606/q/u1/img-01/display.webp",
            thumb_key: "img/g1/202606/q/u1/img-01/thumb.webp",
            processed_at: processed_at(),
        },
    )
    .await
    .unwrap();
    assert_eq!(outcome, UpdateOutcome::Updated);

    let got = get_image(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
    assert_eq!(got.width, Some(1200));
    assert_eq!(got.height, Some(800));
    assert_eq!(
        got.display_key,
        Some("img/g1/202606/q/u1/img-01/display.webp".to_string())
    );
    assert_eq!(
        got.thumb_key,
        Some("img/g1/202606/q/u1/img-01/thumb.webp".to_string())
    );
    assert_eq!(got.processed_at, Some(processed_at()));
}

#[tokio::test]
async fn it_reports_already_final_when_marking_an_already_ready_image_ready() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("g1", "202606", "img-02", "u1");
    media::put_image(&repo, &img).await.unwrap();

    mark_image_ready(
        &repo,
        &ImageReady {
            group_id: &GroupId::new("g1"),
            cycle_id: &CycleId::new("202606"),
            image_id: &ImageId::new("img-02"),
            width: 1200,
            height: 800,
            display_key: "display",
            thumb_key: "thumb",
            processed_at: processed_at(),
        },
    )
    .await
    .unwrap();

    let second = mark_image_ready(
        &repo,
        &ImageReady {
            group_id: &GroupId::new("g1"),
            cycle_id: &CycleId::new("202606"),
            image_id: &ImageId::new("img-02"),
            width: 1,
            height: 1,
            display_key: "other-display",
            thumb_key: "other-thumb",
            processed_at: processed_at(),
        },
    )
    .await
    .unwrap();
    assert_eq!(second, UpdateOutcome::AlreadyFinal);

    // the second call must not have clobbered the first write
    let got = get_image(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-02"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.width, Some(1200));
}

#[tokio::test]
async fn it_marks_a_pending_image_failed_with_an_error_message() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("g1", "202606", "img-03", "u1");
    media::put_image(&repo, &img).await.unwrap();

    let outcome = mark_image_failed(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-03"),
        "IMAGE_TOO_LARGE",
        processed_at(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, UpdateOutcome::Updated);

    let got = get_image(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-03"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("IMAGE_TOO_LARGE".to_string()));
}

#[tokio::test]
async fn it_reports_already_final_when_marking_an_already_failed_image_failed() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("g1", "202606", "img-04", "u1");
    media::put_image(&repo, &img).await.unwrap();

    mark_image_failed(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-04"),
        "IMAGE_DECODE_FAILED",
        processed_at(),
    )
    .await
    .unwrap();

    let second = mark_image_failed(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-04"),
        "IMAGE_TOO_LARGE",
        processed_at(),
    )
    .await
    .unwrap();
    assert_eq!(second, UpdateOutcome::AlreadyFinal);
}

#[tokio::test]
async fn it_marks_a_pending_avatar_ready_with_its_display_key() {
    let (_c, repo) = common::make_repo().await;
    let avatar = common::avatar_media("u1", "av-01");
    media::put_avatar(&repo, &avatar).await.unwrap();

    let outcome = mark_avatar_ready(
        &repo,
        &UserId::new("u1"),
        &AvatarId::new("av-01"),
        "avatar/av-01/display.webp",
        processed_at(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, UpdateOutcome::Updated);

    let got = get_avatar(&repo, &UserId::new("u1"), &AvatarId::new("av-01"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
    assert_eq!(
        got.display_key,
        Some("avatar/av-01/display.webp".to_string())
    );
}

#[tokio::test]
async fn it_marks_a_pending_avatar_failed_with_an_error_message() {
    let (_c, repo) = common::make_repo().await;
    let avatar = common::avatar_media("u1", "av-02");
    media::put_avatar(&repo, &avatar).await.unwrap();

    let outcome = mark_avatar_failed(
        &repo,
        &UserId::new("u1"),
        &AvatarId::new("av-02"),
        "IMAGE_DECODE_FAILED",
        processed_at(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, UpdateOutcome::Updated);

    let got = get_avatar(&repo, &UserId::new("u1"), &AvatarId::new("av-02"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("IMAGE_DECODE_FAILED".to_string()));
}

#[tokio::test]
async fn it_reports_already_final_when_marking_an_already_ready_avatar_failed() {
    let (_c, repo) = common::make_repo().await;
    let avatar = common::avatar_media("u1", "av-03");
    media::put_avatar(&repo, &avatar).await.unwrap();

    mark_avatar_ready(
        &repo,
        &UserId::new("u1"),
        &AvatarId::new("av-03"),
        "avatar/av-03/display.webp",
        processed_at(),
    )
    .await
    .unwrap();

    let second = mark_avatar_failed(
        &repo,
        &UserId::new("u1"),
        &AvatarId::new("av-03"),
        "IMAGE_TOO_LARGE",
        processed_at(),
    )
    .await
    .unwrap();
    assert_eq!(second, UpdateOutcome::AlreadyFinal);
}
