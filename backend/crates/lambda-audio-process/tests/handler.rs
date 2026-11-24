mod common;

use audio_process::handler::handle_event;
use audio_process::state::{AppState, Buckets};
use audio_process::storage::fake::FakeStore;
use common::FakeTranscoder;
use domain::{AudioId, CycleId, GroupId, MediaStatus};
use persistence::media;
use pretty_assertions::assert_eq;
use std::sync::Arc;

const MEDIA_ORIGINALS: &str = "media-originals";
const PROCESSED: &str = "media-processed";

fn buckets() -> Buckets {
    Buckets {
        media_originals: MEDIA_ORIGINALS.into(),
        processed: PROCESSED.into(),
    }
}

fn state(repo: persistence::Repo, store: Arc<FakeStore>, transcoder: FakeTranscoder) -> AppState {
    AppState {
        repo,
        store,
        buckets: buckets(),
        transcoder: Arc::new(transcoder),
    }
}

#[tokio::test]
async fn a_pending_clip_becomes_ready_with_duration_and_peaks() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = state(repo, fake.clone(), FakeTranscoder::with_duration_ms(4_200));

    let row = common::pending_audio("g1", "202606", "aud-01", "q1", "u1");
    media::put_audio(&state.repo, &row).await.unwrap();
    let key = "audio-uploads/g1/202606/q1/u1/aud-01.webm";
    fake.seed(MEDIA_ORIGINALS, key, vec![0u8; 1024]).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_audio(
        &state.repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &AudioId::new("aud-01"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Ready);
    assert_eq!(got.duration_ms, Some(4_200));
    let peaks = got.peaks.expect("peaks set");
    assert_eq!(peaks.len(), shared::config::AUDIO_PEAK_COUNT);

    let clip_key = got.clip_key.expect("clip key set");
    assert_eq!(clip_key, "aud/g1/202606/q1/u1/aud-01/clip.webm");
    assert!(fake.contains(PROCESSED, &clip_key).await);
    let clip_object = fake.get_object(PROCESSED, &clip_key).await.unwrap();
    assert_eq!(clip_object.content_type, "audio/webm");
    assert_eq!(
        clip_object.cache_control,
        "private, max-age=5184000, immutable"
    );

    assert!(
        !fake.contains(MEDIA_ORIGINALS, key).await,
        "the original is deleted once the row is ready"
    );
}

#[tokio::test]
async fn an_oversized_object_is_marked_failed_and_the_original_is_deleted() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = state(repo, fake.clone(), FakeTranscoder::with_duration_ms(1_000));

    let row = common::pending_audio("g2", "202606", "aud-02", "q1", "u1");
    media::put_audio(&state.repo, &row).await.unwrap();
    let key = "audio-uploads/g2/202606/q1/u1/aud-02.webm";
    fake.seed(
        MEDIA_ORIGINALS,
        key,
        vec![0u8; (shared::config::MAX_AUDIO_BYTES + 1) as usize],
    )
    .await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_audio(
        &state.repo,
        &GroupId::new("g2"),
        &CycleId::new("202606"),
        &AudioId::new("aud-02"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("AUDIO_TOO_LARGE".to_owned()));
    assert!(!fake.contains(MEDIA_ORIGINALS, key).await);
}

#[tokio::test]
async fn a_decode_failure_is_marked_failed_with_decode_failed() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = state(repo, fake.clone(), FakeTranscoder::decode_fails());

    let row = common::pending_audio("g3", "202606", "aud-03", "q1", "u1");
    media::put_audio(&state.repo, &row).await.unwrap();
    let key = "audio-uploads/g3/202606/q1/u1/aud-03.webm";
    fake.seed(MEDIA_ORIGINALS, key, vec![0u8; 1024]).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_audio(
        &state.repo,
        &GroupId::new("g3"),
        &CycleId::new("202606"),
        &AudioId::new("aud-03"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("AUDIO_DECODE_FAILED".to_owned()));
}

#[tokio::test]
async fn a_clip_over_the_duration_cap_is_marked_failed_with_audio_too_long() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let over_cap_ms = u64::from(shared::config::MAX_AUDIO_SECONDS) * 1000
        + shared::config::AUDIO_DURATION_SLACK_MS
        + 1;
    let state = state(
        repo,
        fake.clone(),
        FakeTranscoder::with_duration_ms(over_cap_ms),
    );

    let row = common::pending_audio("g4", "202606", "aud-04", "q1", "u1");
    media::put_audio(&state.repo, &row).await.unwrap();
    let key = "audio-uploads/g4/202606/q1/u1/aud-04.webm";
    fake.seed(MEDIA_ORIGINALS, key, vec![0u8; 1024]).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    let got = media::get_audio(
        &state.repo,
        &GroupId::new("g4"),
        &CycleId::new("202606"),
        &AudioId::new("aud-04"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Failed);
    assert_eq!(got.error_message, Some("AUDIO_TOO_LONG".to_owned()));
}

#[tokio::test]
async fn an_already_gone_original_is_skipped_not_an_error() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = state(repo, fake.clone(), FakeTranscoder::with_duration_ms(1_000));

    let row = common::pending_audio("g5", "202606", "aud-05", "q1", "u1");
    media::put_audio(&state.repo, &row).await.unwrap();
    // No seed: the original is "already gone" (a duplicate S3 delivery after
    // the first invocation already finished and deleted it).
    let key = "audio-uploads/g5/202606/q1/u1/aud-05.webm";

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .expect("a missing original is skipped, not a transient error");

    let got = media::get_audio(
        &state.repo,
        &GroupId::new("g5"),
        &CycleId::new("202606"),
        &AudioId::new("aud-05"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got.status, MediaStatus::Pending, "left untouched");
}

#[tokio::test]
async fn a_row_that_is_already_ready_is_skipped() {
    let (_c, repo) = common::make_repo().await;
    let fake = Arc::new(FakeStore::new());
    let state = state(repo, fake.clone(), FakeTranscoder::with_duration_ms(1_000));

    let mut row = common::pending_audio("g6", "202606", "aud-06", "q1", "u1");
    row.status = MediaStatus::Ready;
    row.clip_key = Some("aud/g6/202606/q1/u1/aud-06/clip.webm".into());
    media::put_audio(&state.repo, &row).await.unwrap();
    let key = "audio-uploads/g6/202606/q1/u1/aud-06.webm";
    fake.seed(MEDIA_ORIGINALS, key, vec![0u8; 1024]).await;

    handle_event(&state, common::s3_event(MEDIA_ORIGINALS, key))
        .await
        .unwrap();

    // Original untouched by this no-op invocation, and no second clip
    // written under a different key.
    assert!(fake.contains(MEDIA_ORIGINALS, key).await);
}
