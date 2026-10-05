mod common;

use domain::{AvatarId, CycleId, GroupId, ImageId, MediaStatus, UserId};
use image::{ImageBuffer, ImageFormat, Rgba, RgbaImage};
use image_process::handler::handle_event;
use image_process::state::{AppState, Buckets};
use image_process::storage::fake::FakeStore;
use persistence::media;
use pretty_assertions::assert_eq;
use std::io::Cursor;
use std::sync::Arc;

const MEDIA_ORIGINALS: &str = "media-originals";
const AVATARS_ORIGINALS: &str = "avatars-originals";
const PROCESSED: &str = "media-processed";
const AVATARS_PROCESSED: &str = "avatars-processed";

fn buckets() -> Buckets {
    Buckets {
        media_originals: MEDIA_ORIGINALS.into(),
        avatars_originals: AVATARS_ORIGINALS.into(),
        processed: PROCESSED.into(),
        avatars_processed: AVATARS_PROCESSED.into(),
    }
}

fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
    let buf: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([10, 20, 30, 255]));
    let img = image::DynamicImage::ImageRgba8(buf);
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Jpeg)
        .expect("fixture encodes");
    out.into_inner()
}

#[tokio::test]
async fn a_pending_image_becomes_ready_with_both_variants_uploaded() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let row = common::pending_image("g1", "202606", "img-01", "q1", "u1");
    media::put_image(&state.repo, &row).await.unwrap();
    let key = "uploads/g1/202606/q1/u1/img-01.jpg";
    fake.seed(MEDIA_ORIGINALS, key, jpeg_bytes(2000, 1000))
        .await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_image(
        &state.repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
    assert_eq!(got.width, Some(2000));
    assert_eq!(got.height, Some(1000));
    let display_key = got.display_key.expect("display key set");
    let thumb_key = got.thumb_key.expect("thumb key set");
    assert!(fake.contains(PROCESSED, &display_key).await);
    assert!(fake.contains(PROCESSED, &thumb_key).await);

    let display_object = fake.get_object(PROCESSED, &display_key).await.unwrap();
    assert_eq!(display_object.content_type, "image/webp");
    assert_eq!(
        display_object.cache_control,
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn an_oversized_object_is_marked_failed_and_the_original_is_deleted() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let row = common::pending_image("g1", "202606", "img-02", "q1", "u1");
    media::put_image(&state.repo, &row).await.unwrap();
    let key = "uploads/g1/202606/q1/u1/img-02.jpg";
    let oversized = vec![0u8; 16 * 1024 * 1024];
    fake.seed(MEDIA_ORIGINALS, key, oversized).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_image(
        &state.repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-02"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("IMAGE_TOO_LARGE".to_string()));
    assert!(!fake.contains(MEDIA_ORIGINALS, key).await);
}

#[tokio::test]
async fn garbage_bytes_are_marked_failed_with_decode_failed_and_the_original_is_kept() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let row = common::pending_image("g1", "202606", "img-03", "q1", "u1");
    media::put_image(&state.repo, &row).await.unwrap();
    let key = "uploads/g1/202606/q1/u1/img-03.jpg";
    fake.seed(MEDIA_ORIGINALS, key, b"not an image".to_vec())
        .await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_image(
        &state.repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img-03"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("IMAGE_DECODE_FAILED".to_string()));
    assert!(fake.contains(MEDIA_ORIGINALS, key).await);
}

#[tokio::test]
async fn an_already_ready_row_is_skipped_and_not_reuploaded() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let mut row = common::pending_image("g1", "202606", "img-04", "q1", "u1");
    row.status = MediaStatus::Ready;
    row.display_key = Some("img/g1/202606/q1/u1/img-04/display.webp".into());
    row.thumb_key = Some("img/g1/202606/q1/u1/img-04/thumb.webp".into());
    media::put_image(&state.repo, &row).await.unwrap();
    let key = "uploads/g1/202606/q1/u1/img-04.jpg";
    fake.seed(MEDIA_ORIGINALS, key, jpeg_bytes(100, 100)).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    assert!(
        !fake
            .contains(PROCESSED, "img/g1/202606/q1/u1/img-04/display.webp")
            .await
    );
}

#[tokio::test]
async fn a_record_from_an_unrecognized_bucket_is_skipped() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let key = "uploads/g1/202606/q1/u1/img-05.jpg";
    fake.seed("some-other-bucket", key, jpeg_bytes(100, 100))
        .await;

    // No row exists for this image at all; if the handler mistakenly tried
    // to process it, the DynamoDB lookup itself would error. A clean `Ok`
    // return demonstrates the unknown bucket was skipped before any lookup.
    handle_event(&state, common::s3_event("some-other-bucket", key))
        .await
        .unwrap();
}

#[tokio::test]
async fn an_avatar_happy_path_produces_a_256_square_webp() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    let row = common::pending_avatar("u1", "av-01");
    media::put_avatar(&state.repo, &row).await.unwrap();
    let key = "uploads/u1/av-01.jpg";
    fake.seed(AVATARS_ORIGINALS, key, jpeg_bytes(500, 300))
        .await;

    handle_event(&state, common::s3_event(AVATARS_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_avatar(&state.repo, &UserId::new("u1"), &AvatarId::new("av-01"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
    let display_key = got.display_key.expect("display key set");
    assert_eq!(display_key, "avatar/av-01/display.webp");
    let stored = fake
        .get_object(AVATARS_PROCESSED, &display_key)
        .await
        .expect("avatar display object present");
    let decoded = image::load_from_memory(&stored.bytes).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (256, 256));
}

#[tokio::test]
async fn a_url_encoded_event_key_is_decoded_before_parsing() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = AppState {
        repo,
        store: fake.clone(),
        buckets: buckets(),
    };

    // The S3 event's key field uses `+` for the space in the image id (how
    // S3 encodes it in the notification). The object as actually stored in
    // the bucket — and the row in the table — use the decoded key.
    let row = common::pending_image("g1", "202606", "img 01", "q1", "u1");
    media::put_image(&state.repo, &row).await.unwrap();
    let decoded_key = "uploads/g1/202606/q1/u1/img 01.jpg";
    let raw_event_key = "uploads/g1/202606/q1/u1/img+01.jpg";
    fake.seed(MEDIA_ORIGINALS, decoded_key, jpeg_bytes(300, 300))
        .await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, raw_event_key))
        .await
        .unwrap();

    let got = media::get_image(
        &state.repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &ImageId::new("img 01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
}
