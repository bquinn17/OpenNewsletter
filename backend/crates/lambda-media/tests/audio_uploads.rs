//! `POST /audio-uploads`, `GET`/`PATCH`/`DELETE /audio-uploads/{audioId}`
//! (`docs/design/media.md` §12, `docs/design/api.md` §9 — voice
//! clips). Mirrors `uploads.rs`'s coverage of the image routes.
//!
//! Each test uses its own group id (never a shared one): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL, and `cargo test` runs many of these tests concurrently in the
//! same process.

mod common;

use ::media::handlers;
use domain::api::{CreateAudioUploadRequest, PatchAudioUploadRequest};
use domain::{ApiErrorCode, CycleId, GroupId, ImagePurpose, MediaStatus, QuestionId, Role, UserId};
use pretty_assertions::assert_eq;

fn audio_request(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    mime_type: &str,
    byte_size: u64,
    purpose: Option<ImagePurpose>,
) -> CreateAudioUploadRequest {
    CreateAudioUploadRequest {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        purpose,
        mime_type: mime_type.into(),
        byte_size,
        sha256: None,
    }
}

async fn seed_open_cycle(repo: &persistence::Repo, group_id: &str, cycle_id: &str, user_id: &str) {
    let group = common::group(group_id, user_id);
    common::seed_group_and_membership(repo, &group, user_id, Role::Member).await;
    persistence::newsletters::write_status_transition(
        repo,
        &common::open_newsletter(group_id, cycle_id),
    )
    .await
    .unwrap();
    common::put_locked_question(
        repo,
        &common::text_locked_question(group_id, cycle_id, "q1", user_id),
    )
    .await;
}

#[tokio::test]
async fn create_audio_upload_returns_a_presigned_put_with_the_right_headers() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gapresign", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = audio_request("gapresign", "202606", "q1", "audio/webm", 1_048_576, None);
    let response = handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect("presign succeeds");

    assert!(response.upload_url.contains("test-media-originals"));
    assert!(response.upload_url.contains("audio-uploads/"));
    assert_eq!(
        response.headers.get("content-type").map(String::as_str),
        Some("audio/webm")
    );
    assert_eq!(response.expires_in_seconds, 600);
}

#[tokio::test]
async fn codec_parameters_are_stripped_from_the_mime_type() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gacodec", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = audio_request(
        "gacodec",
        "202606",
        "q1",
        "audio/webm;codecs=opus",
        1_048_576,
        None,
    );
    handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect("codec parameter is stripped before matching the allowlist");
}

#[tokio::test]
async fn unsupported_audio_type_is_rejected_as_bad_type() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gabad", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = audio_request("gabad", "202606", "q1", "audio/flac", 1_048_576, None);
    let err = handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("flac is not allowed");
    assert_eq!(err.code, ApiErrorCode::AudioBadType);
}

#[tokio::test]
async fn oversized_audio_upload_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gaover", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = audio_request(
        "gaover",
        "202606",
        "q1",
        "audio/webm",
        shared::config::MAX_AUDIO_BYTES + 1,
        None,
    );
    let err = handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("over the byte cap");
    assert_eq!(err.code, ApiErrorCode::AudioTooLarge);
}

#[tokio::test]
async fn fourth_response_clip_hits_the_clip_cap() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gacap", "202606", "u1").await;
    for i in 0..shared::config::MAX_AUDIO_CLIPS_PER_RESPONSE {
        common::seed_audio(
            &repo,
            "gacap",
            "202606",
            "q1",
            &format!("aud-{i}"),
            "u1",
            MediaStatus::Ready,
            ImagePurpose::Response,
        )
        .await;
    }
    let state = common::test_state(repo);

    let request = audio_request("gacap", "202606", "q1", "audio/webm", 1_048_576, None);
    let err = handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("at the cap already");
    assert_eq!(err.code, ApiErrorCode::AudioLimitExceeded);
}

#[tokio::test]
async fn comment_purpose_clip_requires_a_published_cycle() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gacomment", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = audio_request(
        "gacomment",
        "202606",
        "q1",
        "audio/webm",
        1_048_576,
        Some(ImagePurpose::Comment),
    );
    let err = handlers::create_audio_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("cycle is open, not published");
    assert_eq!(err.code, ApiErrorCode::CycleNotPublished);
}

#[tokio::test]
async fn get_audio_upload_by_non_owner_is_forbidden() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gaget", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    let audio_id = common::seed_audio(
        &repo,
        "gaget",
        "202606",
        "q1",
        "aud-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);

    let err = handlers::get_audio_upload(
        &state,
        &UserId::new("u2"),
        &GroupId::new("gaget"),
        &CycleId::new("202606"),
        &audio_id,
    )
    .await
    .expect_err("another member doesn't own this clip");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn patch_audio_upload_sets_and_clears_the_caption() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gapatch", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let audio_id = common::seed_audio(
        &repo,
        "gapatch",
        "202606",
        "q1",
        "aud-1",
        "u1",
        MediaStatus::Pending,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let group_id = GroupId::new("gapatch");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    let updated = handlers::patch_audio_upload(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &audio_id,
        PatchAudioUploadRequest {
            caption: Some("The time the tent blew away".into()),
        },
    )
    .await
    .expect("caption set");
    assert_eq!(
        updated.caption,
        Some("The time the tent blew away".to_owned())
    );

    let cleared = handlers::patch_audio_upload(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &audio_id,
        PatchAudioUploadRequest { caption: None },
    )
    .await
    .expect("caption cleared");
    assert_eq!(cleared.caption, None);
}

#[tokio::test]
async fn delete_removes_the_objects_and_the_row() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gadelok", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let audio = common::seed_ready_audio(&repo, "gadelok", "202606", "q1", "aud-1", "u1").await;
    let (state, fakes) = common::test_state_with_fakes(repo);
    let group_id = GroupId::new("gadelok");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    handlers::delete_audio_upload(&state, &caller, &group_id, &cycle_id, &audio.audio_id)
        .await
        .expect("unattached clip can be deleted");

    assert_eq!(
        fakes.store.deleted().await,
        vec![
            (
                common::MEDIA_ORIGINALS.to_owned(),
                audio.original_key.clone()
            ),
            (
                common::MEDIA_PROCESSED.to_owned(),
                audio.clip_key.clone().unwrap(),
            ),
        ]
    );
    let err = handlers::get_audio_upload(&state, &caller, &group_id, &cycle_id, &audio.audio_id)
        .await
        .expect_err("the row is gone");
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn deleting_an_already_deleted_clip_succeeds_and_touches_nothing() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gadelagain", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let audio = common::seed_ready_audio(&repo, "gadelagain", "202606", "q1", "aud-1", "u1").await;
    let (state, fakes) = common::test_state_with_fakes(repo);
    let group_id = GroupId::new("gadelagain");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");
    handlers::delete_audio_upload(&state, &caller, &group_id, &cycle_id, &audio.audio_id)
        .await
        .unwrap();

    handlers::delete_audio_upload(&state, &caller, &group_id, &cycle_id, &audio.audio_id)
        .await
        .expect("a repeat delete is a no-op success");

    assert_eq!(fakes.store.deleted().await.len(), 2);
}

#[tokio::test]
async fn delete_of_a_clip_in_a_published_answer_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gadelused", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let audio_id = common::seed_audio(
        &repo,
        "gadelused",
        "202606",
        "q1",
        "aud-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    common::put_published_response_with_audio(&repo, "gadelused", "202606", "q1", "u1", &["aud-1"])
        .await;
    let state = common::test_state(repo);

    let err = handlers::delete_audio_upload(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gadelused"),
        &CycleId::new("202606"),
        &audio_id,
    )
    .await
    .expect_err("clip is referenced by a published answer");
    assert_eq!(err.code, ApiErrorCode::AudioInUse);
}

#[tokio::test]
async fn delete_of_a_clip_claimed_by_a_comment_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gadelcmt", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let audio_id = common::seed_audio(
        &repo,
        "gadelcmt",
        "202606",
        "q1",
        "aud-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Comment,
    )
    .await;
    common::attach_audio_to_comment(&repo, "gadelcmt", "202606", &audio_id, "c1").await;
    let state = common::test_state(repo);

    let err = handlers::delete_audio_upload(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gadelcmt"),
        &CycleId::new("202606"),
        &audio_id,
    )
    .await
    .expect_err("clip is attached to a comment");
    assert_eq!(err.code, ApiErrorCode::AudioInUse);
}
