//! `delete_image_media` against DynamoDB Local and the recording fakes. The
//! ready-image, failure and avatar paths are covered through the
//! `lambda-media` handlers; this covers what they don't reach.

use chrono::Utc;
use domain::{
    CycleId, GroupId, ImageId, ImageMedia, ImageMimeType, ImagePurpose, MediaStatus, UserId,
};
use media_delete::fakes::fake_deleter;
use persistence::{media, test_factories};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn an_unprocessed_image_deletes_its_original_and_row_without_an_invalidation() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let image = ImageMedia {
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
    };
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
