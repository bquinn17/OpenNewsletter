//! Handler-level tests for `lambda-responses` (`plans/03-api-contract.md` §7).
//!
//! Each test uses its own group id (never a shared "g1"): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL (`plans/02-data-model-dynamodb.md` §6), and `cargo test` runs many
//! of these tests concurrently in the same process. Reusing a group id across
//! tests let one test's cached role leak into another's assertions.

mod common;

use domain::api::SaveResponseRequest;
use domain::{
    ApiErrorCode, CycleId, GroupId, ImageId, ImagePurpose, MediaStatus, PollOptionId, QuestionId,
    ResponseStatus, Role, UserId,
};
use persistence::newsletters;
use pretty_assertions::assert_eq;
// Leading `::` resolves to the extern crate `responses` (this Lambda's own
// library, per `lambda-responses/Cargo.toml`'s `[lib] name = "responses"`)
// rather than the `persistence::responses` module used inside `handlers`.
use ::responses::handlers;
use ::responses::state::AppState;

fn state(repo: persistence::Repo) -> AppState {
    AppState { repo }
}

fn text_request(body: &str, image_media_ids: Vec<ImageId>, publish: bool) -> SaveResponseRequest {
    SaveResponseRequest::Text {
        body: body.to_owned(),
        image_media_ids,
        publish,
    }
}

fn poll_request(poll_option_id: &str, publish: bool) -> SaveResponseRequest {
    SaveResponseRequest::Poll {
        poll_option_id: PollOptionId::new(poll_option_id),
        publish,
    }
}

#[tokio::test]
async fn non_member_is_forbidden_on_every_route() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnm", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gnm", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gnm", "202606", "q1", "u1")).await;

    let state = state(repo);
    let outsider = UserId::new("intruder");
    let group_id = GroupId::new("gnm");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");

    let list_err = handlers::list_my_responses(&state, &outsider, &group_id, &cycle_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(list_err.code, ApiErrorCode::Forbidden);

    let get_err = handlers::get_my_response(&state, &outsider, &group_id, &cycle_id, &question_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(get_err.code, ApiErrorCode::Forbidden);

    let put_err = handlers::save_response(
        &state,
        &outsider,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("hi", vec![], false),
    )
    .await
    .expect_err("non-member must be refused");
    assert_eq!(put_err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn get_my_response_404s_when_no_draft_exists() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("g404", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("g404", "202606"))
        .await
        .unwrap();

    let state = state(repo);
    let err = handlers::get_my_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("g404"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
    )
    .await
    .expect_err("no draft exists yet");
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn save_draft_then_get_returns_the_draft() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gsd", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gsd", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gsd", "202606", "q1", "u1")).await;

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gsd");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");

    let saved = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("my draft answer", vec![], false),
    )
    .await
    .expect("member can save a draft");
    assert_eq!(saved.status, ResponseStatus::Draft);
    assert_eq!(saved.body.as_deref(), Some("my draft answer"));
    assert!(saved.published_at.is_none());

    let fetched = handlers::get_my_response(&state, &caller, &group_id, &cycle_id, &question_id)
        .await
        .expect("draft exists");
    assert_eq!(fetched, saved);
}

#[tokio::test]
async fn interleaved_saves_last_write_wins_with_stable_response_id() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gis", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gis", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gis", "202606", "q1", "u1")).await;

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gis");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");

    let first = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("first save", vec![], false),
    )
    .await
    .unwrap();

    let second = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("second save", vec![], false),
    )
    .await
    .unwrap();

    assert_eq!(second.body.as_deref(), Some("second save"));
    assert_eq!(
        second.response_id, first.response_id,
        "responseId must stay stable across saves"
    );
}

#[tokio::test]
async fn publish_sets_published_at_and_bumps_editions_answered_once() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpb", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gpb", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gpb", "202606", "q1", "u1")).await;
    common::put_locked_question(&repo, &common::locked_question("gpb", "202606", "q2", "u1")).await;

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gpb");
    let cycle_id = CycleId::new("202606");

    let published_first = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &QuestionId::new("q1"),
        text_request("answer one", vec![], true),
    )
    .await
    .unwrap();
    assert_eq!(published_first.status, ResponseStatus::Published);
    assert!(published_first.published_at.is_some());

    let membership_after_first =
        persistence::groups::get_membership(&state.repo, &caller, &group_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(membership_after_first.editions_answered, 1);

    handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &QuestionId::new("q2"),
        text_request("answer two", vec![], true),
    )
    .await
    .unwrap();

    let membership_after_second =
        persistence::groups::get_membership(&state.repo, &caller, &group_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        membership_after_second.editions_answered, 1,
        "a second publish in the same cycle must not double-count"
    );
}

#[tokio::test]
async fn first_publish_in_a_second_group_same_month_still_bumps_editions_answered() {
    // Cycle ids are calendar months shared by every group, and GSI1's
    // `USER#{u}#NL#{cycleId}` key has no group: group A's published answer
    // must not count as "already published" for group B (`02` AP15 note).
    let (_c, repo) = common::make_repo().await;
    for g in ["gx-a", "gx-b"] {
        let group = common::group(g, "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        newsletters::write_status_transition(&repo, &common::open_newsletter(g, "202606"))
            .await
            .unwrap();
    }
    common::put_locked_question(
        &repo,
        &common::locked_question("gx-a", "202606", "qa", "u1"),
    )
    .await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gx-b", "202606", "qb", "u1"),
    )
    .await;

    let state = state(repo);
    let caller = UserId::new("u1");
    let cycle_id = CycleId::new("202606");
    for (g, q) in [("gx-a", "qa"), ("gx-b", "qb")] {
        handlers::save_response(
            &state,
            &caller,
            &GroupId::new(g),
            &cycle_id,
            &QuestionId::new(q),
            text_request("an answer", vec![], true),
        )
        .await
        .unwrap();
    }

    for g in ["gx-a", "gx-b"] {
        let membership =
            persistence::groups::get_membership(&state.repo, &caller, &GroupId::new(g))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(membership.editions_answered, 1, "group {g}");
    }

    let listed = handlers::list_my_responses(&state, &caller, &GroupId::new("gx-b"), &cycle_id)
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1);
}

#[tokio::test]
async fn publish_false_after_publish_stays_published() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gsp", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gsp", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gsp", "202606", "q1", "u1")).await;

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gsp");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");

    let published = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("final answer", vec![], true),
    )
    .await
    .unwrap();
    assert_eq!(published.status, ResponseStatus::Published);

    let edited = handlers::save_response(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        text_request("a typo fix after publishing", vec![], false),
    )
    .await
    .expect("autosave after publish must still succeed");

    assert_eq!(edited.status, ResponseStatus::Published);
    assert_eq!(edited.published_at, published.published_at);
    assert_eq!(edited.body.as_deref(), Some("a typo fix after publishing"));
}

#[tokio::test]
async fn put_past_the_deadline_is_refused_even_though_status_is_still_open() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpd", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(
        &repo,
        &common::open_newsletter_past_deadline("gpd", "202606"),
    )
    .await
    .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gpd", "202606", "q1", "u1")).await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpd"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("too late", vec![], false),
    )
    .await
    .expect_err("responseCloseAt has passed");
    assert_eq!(err.code, ApiErrorCode::CycleNotOpen);
}

#[tokio::test]
async fn put_on_a_published_cycle_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpc", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::published_newsletter("gpc", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gpc", "202606", "q1", "u1")).await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpc"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("too late", vec![], false),
    )
    .await
    .expect_err("cycle already published");
    assert_eq!(err.code, ApiErrorCode::CycleNotOpen);
}

#[tokio::test]
async fn put_on_a_missing_cycle_is_not_found() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gmc", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gmc"),
        &CycleId::new("202699"),
        &QuestionId::new("q1"),
        text_request("hi", vec![], false),
    )
    .await
    .expect_err("no such cycle");
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn put_on_an_archived_cycle_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gar", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::archived_newsletter("gar", "202606"))
        .await
        .unwrap();

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gar"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("hi", vec![], false),
    )
    .await
    .expect_err("cycle is archived");
    assert_eq!(err.code, ApiErrorCode::NewsletterArchived);
}

#[tokio::test]
async fn body_too_long_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gbl", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gbl", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gbl", "202606", "q1", "u1")).await;

    let state = state(repo);
    let overlong = "a".repeat(shared::config::MAX_RESPONSE_BODY_CHARS + 1);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gbl"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request(&overlong, vec![], false),
    )
    .await
    .expect_err("body exceeds the limit");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn more_than_ten_distinct_images_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gti", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gti", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gti", "202606", "q1", "u1")).await;

    let state = state(repo);
    let ids: Vec<ImageId> = (0..shared::config::MAX_IMAGES_PER_RESPONSE + 1)
        .map(|i| ImageId::new(format!("img-{i}")))
        .collect();
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gti"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photos", ids, false),
    )
    .await
    .expect_err("too many images");
    assert_eq!(err.code, ApiErrorCode::ImageLimitExceeded);
}

#[tokio::test]
async fn poll_option_not_in_the_question_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpo", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gpo", "202606"))
        .await
        .unwrap();
    common::put_locked_question(
        &repo,
        &common::poll_locked_question("gpo", "202606", "q1", "u1", &["opt-a", "opt-b"]),
    )
    .await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpo"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        poll_request("opt-nonexistent", false),
    )
    .await
    .expect_err("option isn't on this question");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn poll_kind_saves_correctly_when_option_is_valid() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpv", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gpv", "202606"))
        .await
        .unwrap();
    common::put_locked_question(
        &repo,
        &common::poll_locked_question("gpv", "202606", "q1", "u1", &["opt-a", "opt-b"]),
    )
    .await;

    let state = state(repo);
    let saved = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpv"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        poll_request("opt-a", true),
    )
    .await
    .expect("valid poll option");
    assert_eq!(saved.poll_option_id, Some(PollOptionId::new("opt-a")));
    assert_eq!(saved.body, None);
    assert_eq!(saved.status, ResponseStatus::Published);
}

#[tokio::test]
async fn text_request_on_a_poll_question_is_a_kind_mismatch() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gkm1", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gkm1", "202606"))
        .await
        .unwrap();
    common::put_locked_question(
        &repo,
        &common::poll_locked_question("gkm1", "202606", "q1", "u1", &["opt-a", "opt-b"]),
    )
    .await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gkm1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("wrong kind", vec![], false),
    )
    .await
    .expect_err("text request on a poll question");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn poll_request_on_a_text_question_is_a_kind_mismatch() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gkm2", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gkm2", "202606"))
        .await
        .unwrap();
    common::put_locked_question(
        &repo,
        &common::locked_question("gkm2", "202606", "q1", "u1"),
    )
    .await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gkm2"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        poll_request("opt-a", false),
    )
    .await
    .expect_err("poll request on a text question");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn image_that_does_not_exist_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gie", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gie", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gie", "202606", "q1", "u1")).await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gie"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photo", vec![ImageId::new("does-not-exist")], false),
    )
    .await
    .expect_err("image does not exist");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn image_owned_by_someone_else_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gio", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gio", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gio", "202606", "q1", "u1")).await;
    let image_id = common::seed_ready_response_image(&repo, "gio", "202606", "img1", "u2").await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gio"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photo", vec![image_id], false),
    )
    .await
    .expect_err("image is owned by another user");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn image_not_yet_ready_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnr", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gnr", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gnr", "202606", "q1", "u1")).await;
    let image_id = common::seed_image(
        &repo,
        "gnr",
        "202606",
        "img1",
        "u1",
        MediaStatus::Pending,
        ImagePurpose::Response,
    )
    .await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gnr"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photo", vec![image_id], false),
    )
    .await
    .expect_err("image isn't ready yet");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn image_with_comment_purpose_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcp", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gcp", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gcp", "202606", "q1", "u1")).await;
    let image_id = common::seed_image(
        &repo,
        "gcp",
        "202606",
        "img1",
        "u1",
        MediaStatus::Ready,
        ImagePurpose::Comment,
    )
    .await;

    let state = state(repo);
    let err = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gcp"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photo", vec![image_id], false),
    )
    .await
    .expect_err("image purpose is comment, not response");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn valid_ready_response_image_is_accepted() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gva", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gva", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gva", "202606", "q1", "u1")).await;
    let image_id = common::seed_ready_response_image(&repo, "gva", "202606", "img1", "u1").await;

    let state = state(repo);
    let saved = handlers::save_response(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gva"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        text_request("photo", vec![image_id.clone()], false),
    )
    .await
    .expect("a ready response image owned by the caller is valid");
    assert_eq!(saved.image_media_ids, vec![image_id]);
}

#[tokio::test]
async fn my_responses_lists_only_the_callers_rows() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gml", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    newsletters::write_status_transition(&repo, &common::open_newsletter("gml", "202606"))
        .await
        .unwrap();
    common::put_locked_question(&repo, &common::locked_question("gml", "202606", "q1", "u1")).await;
    common::put_locked_question(&repo, &common::locked_question("gml", "202606", "q2", "u1")).await;

    let state = state(repo);
    let group_id = GroupId::new("gml");
    let cycle_id = CycleId::new("202606");

    handlers::save_response(
        &state,
        &UserId::new("u1"),
        &group_id,
        &cycle_id,
        &QuestionId::new("q1"),
        text_request("u1's answer to q1", vec![], false),
    )
    .await
    .unwrap();
    handlers::save_response(
        &state,
        &UserId::new("u1"),
        &group_id,
        &cycle_id,
        &QuestionId::new("q2"),
        text_request("u1's answer to q2", vec![], false),
    )
    .await
    .unwrap();
    handlers::save_response(
        &state,
        &UserId::new("u2"),
        &group_id,
        &cycle_id,
        &QuestionId::new("q1"),
        text_request("u2's answer to q1", vec![], false),
    )
    .await
    .unwrap();

    let mine = handlers::list_my_responses(&state, &UserId::new("u1"), &group_id, &cycle_id)
        .await
        .unwrap();
    assert_eq!(mine.items.len(), 2);
    assert!(mine.items.iter().all(|r| r.user_id == UserId::new("u1")));
}
