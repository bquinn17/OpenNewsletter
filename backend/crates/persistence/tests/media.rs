mod common;

use domain::{AvatarId, CycleId, GroupId, ImageId, UserId};
use persistence::media;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_puts_and_gets_image() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("g1", "202606", "img-01", "u1");
    media::put_image(&repo, &img).await.unwrap();
    let got = media::get_image(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got, img);
}

#[tokio::test]
async fn it_returns_none_for_missing_image() {
    let (_c, repo) = common::make_repo().await;
    let result = media::get_image(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("no-such"),
    )
    .await
    .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_lists_user_images_via_gsi1() {
    let (_c, repo) = common::make_repo().await;
    let img1 = common::image_media("g1", "202606", "img-01", "u1");
    let img2 = common::image_media("g1", "202606", "img-02", "u1");
    let other = common::image_media("g1", "202606", "img-03", "u2");
    media::put_image(&repo, &img1).await.unwrap();
    media::put_image(&repo, &img2).await.unwrap();
    media::put_image(&repo, &other).await.unwrap();

    let imgs = media::list_user_images(&repo, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(imgs.len(), 2);
    assert!(imgs.iter().all(|i| i.user_id.as_str() == "u1"));
}

#[tokio::test]
async fn it_puts_and_gets_avatar() {
    let (_c, repo) = common::make_repo().await;
    let av = common::avatar_media("u1", "av-01");
    media::put_avatar(&repo, &av).await.unwrap();
    let got = media::get_avatar(&repo, &UserId::new("u1"), &AvatarId::new("av-01"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, av);
}

#[tokio::test]
async fn it_returns_none_for_missing_avatar() {
    let (_c, repo) = common::make_repo().await;
    let result = media::get_avatar(&repo, &UserId::new("u1"), &AvatarId::new("no-such"))
        .await
        .unwrap();
    assert_eq!(result, None);
}
