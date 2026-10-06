//! `POST /uploads`, `GET`/`PATCH`/`DELETE /uploads/{imageId}`, and
//! `POST /uploads/{imageId}/complete` (`plans/03-api-contract.md` §9.1-§9.4,
//! §9.6).
//!
//! Each test uses its own group id (never a shared one): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL, and `cargo test` runs many of these tests concurrently in the
//! same process.

mod common;

use ::media::handlers;
use domain::api::{CreateUploadRequest, PatchUploadRequest};
use domain::{
    ApiErrorCode, CycleId, GroupId, ImageId, ImagePurpose, MediaStatus, QuestionId, Role, UserId,
};
use pretty_assertions::assert_eq;

fn upload_request(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    mime_type: &str,
    byte_size: u64,
    purpose: Option<ImagePurpose>,
    sha256: Option<String>,
) -> CreateUploadRequest {
    CreateUploadRequest {
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        question_id: QuestionId::new(question_id),
        purpose,
        mime_type: mime_type.into(),
        byte_size,
        sha256,
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
async fn create_upload_returns_a_presigned_put_with_the_right_headers() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gpresign", "202606", "u1").await;
    let state = common::test_state(repo);

    let sha256 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode([7u8; 32])
    };
    let request = upload_request(
        "gpresign",
        "202606",
        "q1",
        "image/jpeg",
        4_823_920,
        None,
        Some(sha256),
    );

    let response = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect("presign succeeds");

    assert!(response.upload_url.contains("test-media-originals"));
    assert!(response.upload_url.contains("X-Amz-Signature"));
    assert_eq!(
        response.headers.get("content-type").map(String::as_str),
        Some("image/jpeg")
    );
    assert!(response.headers.contains_key("x-amz-checksum-sha256"));
    assert!(!response.headers.contains_key("x-amz-checksum-crc32"));
    assert_eq!(response.expires_in_seconds, 600);
}

#[tokio::test]
async fn create_upload_without_sha256_signs_no_checksum_header() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gnosha", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = upload_request("gnosha", "202606", "q1", "image/png", 1_000, None, None);
    let response = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect("presign succeeds");

    assert!(!response.headers.contains_key("x-amz-checksum-sha256"));
    assert!(!response.headers.contains_key("x-amz-checksum-crc32"));
}

#[tokio::test]
async fn svg_mime_type_is_rejected_as_bad_type() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gsvg", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = upload_request("gsvg", "202606", "q1", "image/svg+xml", 1_000, None, None);
    let err = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("svg must be rejected");
    assert_eq!(err.code, ApiErrorCode::ImageBadType);
}

#[tokio::test]
async fn oversized_upload_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gbig", "202606", "u1").await;
    let state = common::test_state(repo);

    let sixteen_mb = 16 * 1024 * 1024;
    let request = upload_request("gbig", "202606", "q1", "image/jpeg", sixteen_mb, None, None);
    let err = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("16 MB is over the cap");
    assert_eq!(err.code, ApiErrorCode::ImageTooLarge);
}

#[tokio::test]
async fn eleventh_response_upload_hits_the_image_cap() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gcap", "202606", "u1").await;

    for i in 0..10 {
        common::seed_image(
            &repo,
            "gcap",
            "202606",
            "q1",
            &format!("img-{i}"),
            "u1",
            MediaStatus::Ready,
            ImagePurpose::Response,
        )
        .await;
    }
    // A failed row must not count toward the cap.
    common::seed_image(
        &repo,
        "gcap",
        "202606",
        "q1",
        "img-failed",
        "u1",
        MediaStatus::Failed,
        ImagePurpose::Response,
    )
    .await;

    let state = common::test_state(repo);
    let request = upload_request("gcap", "202606", "q1", "image/jpeg", 1_000, None, None);
    let err = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("the 11th response image must hit the cap");
    assert_eq!(err.code, ApiErrorCode::ImageLimitExceeded);
}

#[tokio::test]
async fn comment_purpose_upload_requires_a_published_cycle() {
    let (_c, repo) = common::make_repo().await;
    // Open (not published) — a comment-purpose upload must be refused.
    seed_open_cycle(&repo, "gcomment", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = upload_request(
        "gcomment",
        "202606",
        "q1",
        "image/jpeg",
        1_000,
        Some(ImagePurpose::Comment),
        None,
    );
    let err = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("comment uploads need a published cycle");
    assert_eq!(err.code, ApiErrorCode::CycleNotPublished);
}

#[tokio::test]
async fn comment_purpose_upload_succeeds_on_a_published_cycle() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcommentok", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    persistence::newsletters::write_status_transition(
        &repo,
        &common::published_newsletter("gcommentok", "202606"),
    )
    .await
    .unwrap();
    common::put_locked_question(
        &repo,
        &common::text_locked_question("gcommentok", "202606", "q1", "u1"),
    )
    .await;
    let state = common::test_state(repo);

    let request = upload_request(
        "gcommentok",
        "202606",
        "q1",
        "image/jpeg",
        1_000,
        Some(ImagePurpose::Comment),
        None,
    );
    handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect("comment upload on a published cycle succeeds");
}

#[tokio::test]
async fn unknown_question_id_is_a_validation_error() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gnoq", "202606", "u1").await;
    let state = common::test_state(repo);

    let request = upload_request(
        "gnoq",
        "202606",
        "no-such-question",
        "image/jpeg",
        1_000,
        None,
        None,
    );
    let err = handlers::create_upload(&state, &UserId::new("u1"), request)
        .await
        .expect_err("question must be locked into the cycle");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn non_member_is_forbidden_on_every_upload_route() {
    let (_c, repo) = common::make_repo().await;
    seed_open_cycle(&repo, "gnm", "202606", "u1").await;
    let image_id = common::seed_image(
        &repo,
        "gnm",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let outsider = UserId::new("intruder");
    let group_id = GroupId::new("gnm");
    let cycle_id = CycleId::new("202606");

    let create_err = handlers::create_upload(
        &state,
        &outsider,
        upload_request("gnm", "202606", "q1", "image/jpeg", 1_000, None, None),
    )
    .await
    .expect_err("non-member must be refused");
    assert_eq!(create_err.code, ApiErrorCode::Forbidden);

    let get_err = handlers::get_upload(&state, &outsider, &group_id, &cycle_id, &image_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(get_err.code, ApiErrorCode::Forbidden);

    let patch_err = handlers::patch_upload(
        &state,
        &outsider,
        &group_id,
        &cycle_id,
        &image_id,
        PatchUploadRequest {
            caption: Some("hi".into()),
        },
    )
    .await
    .expect_err("non-member must be refused");
    assert_eq!(patch_err.code, ApiErrorCode::Forbidden);

    let delete_err = handlers::delete_upload(&state, &outsider, &group_id, &cycle_id, &image_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(delete_err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn get_upload_is_readable_by_any_member_not_just_the_owner() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gread", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gread",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);

    let response = handlers::get_upload(
        &state,
        &UserId::new("u2"),
        &GroupId::new("gread"),
        &CycleId::new("202606"),
        &image_id,
    )
    .await
    .expect("any member can read");
    assert_eq!(response.image_id, image_id);
    assert_eq!(response.user_id, UserId::new("u1"));
}

#[tokio::test]
async fn patch_upload_by_non_owner_is_forbidden() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpatchfb", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gpatchfb",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);

    let err = handlers::patch_upload(
        &state,
        &UserId::new("u2"),
        &GroupId::new("gpatchfb"),
        &CycleId::new("202606"),
        &image_id,
        PatchUploadRequest {
            caption: Some("not yours".into()),
        },
    )
    .await
    .expect_err("non-owner must be refused");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn patch_upload_sets_and_clears_the_caption() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpatchok", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gpatchok",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let group_id = GroupId::new("gpatchok");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    let updated = handlers::patch_upload(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &image_id,
        PatchUploadRequest {
            caption: Some("Cocoa thermos at mile two.".into()),
        },
    )
    .await
    .expect("owner can set a caption");
    assert_eq!(updated.caption, Some("Cocoa thermos at mile two.".into()));

    let cleared = handlers::patch_upload(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &image_id,
        PatchUploadRequest { caption: None },
    )
    .await
    .expect("owner can clear a caption");
    assert_eq!(cleared.caption, None);
}

#[tokio::test]
async fn patch_upload_rejects_an_overlong_caption() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpatchlong", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gpatchlong",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);

    let err = handlers::patch_upload(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpatchlong"),
        &CycleId::new("202606"),
        &image_id,
        PatchUploadRequest {
            caption: Some("a".repeat(141)),
        },
    )
    .await
    .expect_err("141 chars is over the cap");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn delete_of_a_draft_only_image_succeeds_and_marks_it_failed() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gdelok", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gdelok",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let group_id = GroupId::new("gdelok");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    handlers::delete_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .expect("draft-only image can be deleted");

    let after = handlers::get_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .expect("still readable after delete");
    assert_eq!(after.status, MediaStatus::Failed);
    assert_eq!(after.error_message, Some("DELETED".into()));

    // Idempotent: deleting it again still succeeds.
    handlers::delete_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .expect("delete is idempotent");
}

#[tokio::test]
async fn delete_of_an_image_in_a_published_answer_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gdelused", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gdelused",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    common::put_published_response(&repo, "gdelused", "202606", "q1", "u1", &["img-1"]).await;
    let state = common::test_state(repo);

    let err = handlers::delete_upload(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gdelused"),
        &CycleId::new("202606"),
        &image_id,
    )
    .await
    .expect_err("an in-use image can't be deleted");
    assert_eq!(err.code, ApiErrorCode::ImageInUse);
}

#[tokio::test]
async fn delete_of_an_image_claimed_by_a_comment_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gdelcmt", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gdelcmt",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Comment,
    )
    .await;
    common::attach_image_to_comment(&repo, "gdelcmt", "202606", &image_id, "comment-1").await;
    let state = common::test_state(repo);

    let err = handlers::delete_upload(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gdelcmt"),
        &CycleId::new("202606"),
        &image_id,
    )
    .await
    .expect_err("a comment-claimed image can't be deleted");
    assert_eq!(err.code, ApiErrorCode::ImageInUse);
}

#[tokio::test]
async fn complete_upload_mirrors_get_upload() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcomplete", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gcomplete",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let group_id = GroupId::new("gcomplete");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    let via_get = handlers::get_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    let via_complete = handlers::complete_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    assert_eq!(via_get, via_complete);
}

#[tokio::test]
async fn ready_image_exposes_display_and_thumb_urls() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gurls", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let image_id = common::seed_image(
        &repo,
        "gurls",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Pending,
        ImagePurpose::Response,
    )
    .await;
    let state = common::test_state(repo);
    let group_id = GroupId::new("gurls");
    let cycle_id = CycleId::new("202606");
    let caller = UserId::new("u1");

    let pending = handlers::get_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    assert_eq!(pending.display_url, None);
    assert_eq!(pending.thumb_url, None);

    persistence::media::set_image_caption(&state.repo, &group_id, &cycle_id, &image_id, None)
        .await
        .unwrap();
    // Flip the row to ready with display/thumb keys, as the image-process
    // Lambda would, and confirm the URLs resolve through `CDN_BASE_URL`.
    mark_ready_with_keys(&state.repo, &group_id, &cycle_id, &image_id).await;

    let ready = handlers::get_upload(&state, &caller, &group_id, &cycle_id, &image_id)
        .await
        .unwrap();
    assert_eq!(
        ready.display_url,
        Some("https://cdn.example.com/img/gurls/202606/q1/u1/img-1/display.webp".into())
    );
    assert_eq!(
        ready.thumb_url,
        Some("https://cdn.example.com/img/gurls/202606/q1/u1/img-1/thumb.webp".into())
    );
}

async fn mark_ready_with_keys(
    repo: &persistence::Repo,
    group_id: &GroupId,
    cycle_id: &CycleId,
    image_id: &ImageId,
) {
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, image_pk, image_sk};

    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(image_pk(group_id, cycle_id)))
        .key(attr::SK, AttributeValue::S(image_sk(image_id)))
        .update_expression("SET #status = :ready, display_key = :dk, thumb_key = :tk")
        .expression_attribute_names("#status", "status")
        .expression_attribute_values(":ready", AttributeValue::S("ready".into()))
        .expression_attribute_values(
            ":dk",
            AttributeValue::S("img/gurls/202606/q1/u1/img-1/display.webp".into()),
        )
        .expression_attribute_values(
            ":tk",
            AttributeValue::S("img/gurls/202606/q1/u1/img-1/thumb.webp".into()),
        )
        .send()
        .await
        .unwrap();
}
