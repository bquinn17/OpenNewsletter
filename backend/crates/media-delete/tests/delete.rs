//! `delete_image_media` against DynamoDB Local and the recording fakes. The
//! ready-image, failure and avatar paths are covered through the
//! `lambda-media` handlers; these cover what they don't reach.

use chrono::Utc;
use domain::{
    CycleId, GroupId, ImageId, ImageMedia, ImageMimeType, ImagePurpose, MediaStatus, UserId,
};
use media_delete::fakes::fake_deleter;
use persistence::{media, test_factories};
use pretty_assertions::assert_eq;

fn pending_image() -> ImageMedia {
    ImageMedia {
        image_id: ImageId::new("img-1"),
        user_id: UserId::new("u1"),
        group_id: GroupId::new("g1"),
        cycle_id: CycleId::new("202606"),
        question_id: None,
        purpose: ImagePurpose::Response,
        mime_type: ImageMimeType::Png,
        original_key: "uploads/g1/202606/q1/u1/img-1.png".into(),
        display_key: None,
        thumb_key: None,
        status: MediaStatus::Pending,
        bytes: 1_024,
        width: None,
        height: None,
        caption: None,
        uploaded_at: Utc::now(),
        processed_at: None,
        error_message: None,
        attached_comment_id: None,
    }
}

#[tokio::test]
async fn an_unprocessed_image_deletes_its_original_and_row_without_an_invalidation() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let image = pending_image();
    media::put_image(&repo, &image).await.unwrap();
    let (deleter, store, cdn) = fake_deleter("originals", "processed");

    media_delete::delete_image_media(&repo, &deleter, &image)
        .await
        .unwrap();

    assert_eq!(
        store.deleted().await,
        vec![("originals".to_owned(), image.original_key.clone())]
    );
    assert!(
        cdn.invalidations().await.is_empty(),
        "nothing was ever served"
    );
    let row = media::get_image(&repo, &image.group_id, &image.cycle_id, &image.image_id)
        .await
        .unwrap();
    assert_eq!(row, None);
}

#[tokio::test]
async fn an_image_that_finishes_processing_mid_delete_loses_its_processed_objects_too() {
    let (_c, repo) = test_factories::make_test_repo().await;
    // The caller read the row while it was pending; processing has since
    // written the variants and marked it ready.
    let stale = pending_image();
    let ready = ImageMedia {
        display_key: Some("img/g1/202606/q1/u1/img-1/display.webp".into()),
        thumb_key: Some("img/g1/202606/q1/u1/img-1/thumb.webp".into()),
        status: MediaStatus::Ready,
        ..stale.clone()
    };
    media::put_image(&repo, &ready).await.unwrap();
    let (deleter, store, cdn) = fake_deleter("originals", "processed");

    media_delete::delete_image_media(&repo, &deleter, &stale)
        .await
        .unwrap();

    let deleted = store.deleted().await;
    assert!(deleted.contains(&(
        "processed".to_owned(),
        "img/g1/202606/q1/u1/img-1/display.webp".to_owned()
    )));
    assert!(deleted.contains(&(
        "processed".to_owned(),
        "img/g1/202606/q1/u1/img-1/thumb.webp".to_owned()
    )));
    assert_eq!(
        cdn.invalidations().await,
        vec![vec![
            "/img/g1/202606/q1/u1/img-1/display.webp".to_owned(),
            "/img/g1/202606/q1/u1/img-1/thumb.webp".to_owned(),
        ]]
    );
    let row = media::get_image(&repo, &stale.group_id, &stale.cycle_id, &stale.image_id)
        .await
        .unwrap();
    assert_eq!(row, None);
}
