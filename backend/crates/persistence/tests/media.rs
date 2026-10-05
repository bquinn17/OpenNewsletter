mod common;

use domain::{AvatarId, CycleId, GroupId, ImageId, ImagePurpose, MediaStatus, QuestionId, UserId};
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

fn response_image(
    group_id: &str,
    cycle_id: &str,
    image_id: &str,
    user_id: &str,
    question_id: &str,
    status: MediaStatus,
) -> domain::ImageMedia {
    let mut img = common::image_media(group_id, cycle_id, image_id, user_id);
    img.question_id = Some(QuestionId::new(question_id));
    img.purpose = ImagePurpose::Response;
    img.status = status;
    img
}

#[tokio::test]
async fn it_counts_active_response_images_for_one_question() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("gcount1");
    let cycle_id = CycleId::new("202606");
    let user_id = UserId::new("u1");
    let question_id = QuestionId::new("q1");

    for i in 0..3 {
        let img = response_image(
            "gcount1",
            "202606",
            &format!("img-{i}"),
            "u1",
            "q1",
            MediaStatus::Ready,
        );
        media::put_image(&repo, &img).await.unwrap();
    }
    // A failed row for the same question must not count.
    let failed = response_image(
        "gcount1",
        "202606",
        "img-failed",
        "u1",
        "q1",
        MediaStatus::Failed,
    );
    media::put_image(&repo, &failed).await.unwrap();
    // A different user's image for the same question must not count.
    let other_user = response_image(
        "gcount1",
        "202606",
        "img-other-user",
        "u2",
        "q1",
        MediaStatus::Ready,
    );
    media::put_image(&repo, &other_user).await.unwrap();
    // A different question for the same user must not count.
    let other_question = response_image(
        "gcount1",
        "202606",
        "img-other-q",
        "u1",
        "q2",
        MediaStatus::Ready,
    );
    media::put_image(&repo, &other_question).await.unwrap();
    // A `comment`-purpose image must not count toward the response cap.
    let mut comment_image = response_image(
        "gcount1",
        "202606",
        "img-comment",
        "u1",
        "q1",
        MediaStatus::Ready,
    );
    comment_image.purpose = ImagePurpose::Comment;
    media::put_image(&repo, &comment_image).await.unwrap();

    let count =
        media::count_active_response_images(&repo, &group_id, &cycle_id, &user_id, &question_id)
            .await
            .unwrap();
    assert_eq!(count, 3);
}

#[tokio::test]
async fn it_sets_and_clears_an_image_caption() {
    let (_c, repo) = common::make_repo().await;
    let img = common::image_media("gcap", "202606", "img-01", "u1");
    media::put_image(&repo, &img).await.unwrap();

    media::set_image_caption(
        &repo,
        &GroupId::new("gcap"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
        Some("Cocoa thermos at mile two."),
    )
    .await
    .unwrap();
    let got = media::get_image(
        &repo,
        &GroupId::new("gcap"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.caption, Some("Cocoa thermos at mile two.".to_owned()));

    media::set_image_caption(
        &repo,
        &GroupId::new("gcap"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
        None,
    )
    .await
    .unwrap();
    let cleared = media::get_image(
        &repo,
        &GroupId::new("gcap"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.caption, None);
}

#[tokio::test]
async fn it_marks_an_image_deleted_idempotently() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("gdel");
    let cycle_id = CycleId::new("202606");
    let image_id = ImageId::new("img-01");
    let img = common::image_media("gdel", "202606", "img-01", "u1");
    media::put_image(&repo, &img).await.unwrap();

    media::mark_image_deleted(&repo, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    let got = media::get_image(&repo, &group_id, &cycle_id, &image_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("DELETED".to_owned()));

    // Idempotent: deleting an already-failed row succeeds and leaves it failed.
    media::mark_image_deleted(&repo, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    let got_again = media::get_image(&repo, &group_id, &cycle_id, &image_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got_again.status, MediaStatus::Failed);
}

#[tokio::test]
async fn it_marks_an_avatar_deleted() {
    let (_c, repo) = common::make_repo().await;
    let user_id = UserId::new("u1");
    let avatar_id = AvatarId::new("av-01");
    let av = common::avatar_media("u1", "av-01");
    media::put_avatar(&repo, &av).await.unwrap();

    media::mark_avatar_deleted(&repo, &user_id, &avatar_id)
        .await
        .unwrap();
    let got = media::get_avatar(&repo, &user_id, &avatar_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("DELETED".to_owned()));
}
