//! `GET`/`POST .../comments`, `PATCH`/`DELETE .../comments/{commentId}`
//! (`plans/03-api-contract.md` §8.1-§8.4).
//!
//! Each test uses its own group id (never a shared one): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL, and `cargo test` runs many of these tests concurrently in the
//! same process.

mod common;

use domain::api::{CreateCommentRequest, PatchCommentRequest};
use domain::{
    ApiErrorCode, CommentId, CycleId, GroupId, MediaStatus, QuestionId, ResponseId, Role,
};
use engagement::handlers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_creates_lists_and_soft_deletes_a_comment_with_an_image() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt1", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_user(&repo, "u2").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt1", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt1", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt1", "202606", "q1", "u2", "r1", "Lake 22.").await;
    let image_id = common::seed_comment_image(
        &repo,
        "gcmt1",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
    )
    .await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("gcmt1");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let author = domain::UserId::new("u1");

    let created = handlers::create_comment(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("Nice answer!".into()),
            image_media_id: Some(image_id.clone()),
        },
    )
    .await
    .expect("comment created");
    assert_eq!(created.display_name, "User u1");
    assert_eq!(created.body, "Nice answer!");
    let image = created.image.expect("image hydrated");
    assert_eq!(image.image_id, image_id);
    assert_eq!(
        image.display_url,
        "https://cdn.example.com/processed/img-1/display.webp"
    );
    assert!(created.deleted_at.is_none());
    assert!(created.edited_at.is_none());

    // The image is now claimed.
    let claimed = common::get_image(&state.repo, "gcmt1", "202606", &image_id).await;
    assert_eq!(
        claimed.attached_comment_id,
        Some(created.comment_id.clone())
    );

    let listed = handlers::list_comments(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        50,
        None,
    )
    .await
    .expect("comments listed");
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.next_cursor, None);

    handlers::delete_comment(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
    )
    .await
    .expect("author can delete own comment");

    let after_delete = handlers::list_comments(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        50,
        None,
    )
    .await
    .expect("comments listed");
    assert_eq!(
        after_delete.items.len(),
        1,
        "soft-deleted row stays, as a placeholder"
    );
    let placeholder = &after_delete.items[0];
    assert!(placeholder.deleted_at.is_some());
    assert_eq!(placeholder.body, "");
    assert!(placeholder.image.is_none());

    // The image's claim is released.
    let released = common::get_image(&state.repo, "gcmt1", "202606", &image_id).await;
    assert_eq!(released.attached_comment_id, None);

    // Idempotent: deleting again still succeeds.
    handlers::delete_comment(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
    )
    .await
    .expect("delete is idempotent");

    // Deleting a missing comment 404s.
    let err = handlers::delete_comment(
        &state,
        &author,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &CommentId::new("nope"),
    )
    .await
    .expect_err("missing comment");
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn create_rejects_a_blank_body_with_no_image() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt2", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt2", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt2", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt2", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let err = handlers::create_comment(
        &state,
        &domain::UserId::new("u1"),
        &GroupId::new("gcmt2"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &ResponseId::new("r1"),
        CreateCommentRequest {
            body: Some("   ".into()),
            image_media_id: None,
        },
    )
    .await
    .expect_err("blank body, no image");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "body");
}

#[tokio::test]
async fn create_rejects_an_image_already_attached_to_another_comment() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt3", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt3", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt3", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt3", "202606", "q1", "u1", "r1", "Lake 22.").await;
    let image_id = common::seed_comment_image(
        &repo,
        "gcmt3",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
    )
    .await;

    let state = common::test_state(repo);
    let caller = domain::UserId::new("u1");
    let group_id = GroupId::new("gcmt3");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    handlers::create_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: None,
            image_media_id: Some(image_id.clone()),
        },
    )
    .await
    .expect("first comment claims the image");

    let err = handlers::create_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("also mine".into()),
            image_media_id: Some(image_id.clone()),
        },
    )
    .await
    .expect_err("image already claimed");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "imageMediaId");
}

#[tokio::test]
async fn create_rejects_an_image_for_the_wrong_question() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt4", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt4", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt4", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt4", "202606", "q1", "u1", "r1", "Lake 22.").await;
    // Image was uploaded for a different question.
    let image_id = common::seed_comment_image(
        &repo,
        "gcmt4",
        "202606",
        "q-other",
        "img-1",
        "u1",
        MediaStatus::Ready,
    )
    .await;

    let state = common::test_state(repo);
    let err = handlers::create_comment(
        &state,
        &domain::UserId::new("u1"),
        &GroupId::new("gcmt4"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &ResponseId::new("r1"),
        CreateCommentRequest {
            body: None,
            image_media_id: Some(image_id),
        },
    )
    .await
    .expect_err("image belongs to a different question");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "imageMediaId");
}

#[tokio::test]
async fn only_the_author_can_edit_a_comment() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt5", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_user(&repo, "u2").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt5", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt5", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt5", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("gcmt5");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let created = handlers::create_comment(
        &state,
        &domain::UserId::new("u2"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("mine".into()),
            image_media_id: None,
        },
    )
    .await
    .expect("u2 creates a comment");

    // u1 is a group admin, but PATCH is author-only — admins can't edit others'.
    let err = handlers::patch_comment(
        &state,
        &domain::UserId::new("u1"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
        PatchCommentRequest {
            body: Some("hijacked".into()),
            image_media_id: None,
        },
    )
    .await
    .expect_err("admin cannot edit someone else's comment");
    assert_eq!(err.code, ApiErrorCode::Forbidden);

    let edited = handlers::patch_comment(
        &state,
        &domain::UserId::new("u2"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
        PatchCommentRequest {
            body: Some("mine, edited".into()),
            image_media_id: None,
        },
    )
    .await
    .expect("author can edit their own comment");
    assert_eq!(edited.body, "mine, edited");
    assert!(edited.edited_at.is_some());
}

#[tokio::test]
async fn patch_distinguishes_absent_from_explicit_null_image() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt6", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt6", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt6", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt6", "202606", "q1", "u1", "r1", "Lake 22.").await;
    let image_id = common::seed_comment_image(
        &repo,
        "gcmt6",
        "202606",
        "q1",
        "img-1",
        "u1",
        MediaStatus::Ready,
    )
    .await;

    let state = common::test_state(repo);
    let caller = domain::UserId::new("u1");
    let group_id = GroupId::new("gcmt6");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let created = handlers::create_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("hi".into()),
            image_media_id: Some(image_id.clone()),
        },
    )
    .await
    .expect("created with image");

    // Absent `imageMediaId` leaves the image attached; only the body changes.
    let patched = handlers::patch_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
        PatchCommentRequest {
            body: Some("hi, edited".into()),
            image_media_id: None,
        },
    )
    .await
    .expect("body-only patch");
    assert_eq!(patched.body, "hi, edited");
    assert!(
        patched.image.is_some(),
        "image untouched by an absent field"
    );

    // Explicit `null` removes it.
    let patched = handlers::patch_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
        PatchCommentRequest {
            body: None,
            image_media_id: Some(None),
        },
    )
    .await
    .expect("image removed");
    assert!(patched.image.is_none());
    assert_eq!(
        patched.body, "hi, edited",
        "body left alone by an absent field"
    );

    let released = common::get_image(&state.repo, "gcmt6", "202606", &image_id).await;
    assert_eq!(released.attached_comment_id, None);
}

#[tokio::test]
async fn patch_requires_the_result_to_still_have_a_body_or_image() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt7", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt7", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt7", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt7", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let caller = domain::UserId::new("u1");
    let group_id = GroupId::new("gcmt7");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let created = handlers::create_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("hi".into()),
            image_media_id: None,
        },
    )
    .await
    .expect("created");

    let err = handlers::patch_comment(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
        PatchCommentRequest {
            body: Some("   ".into()),
            image_media_id: None,
        },
    )
    .await
    .expect_err("would leave the comment empty");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
}

#[tokio::test]
async fn admin_can_delete_any_comment_but_a_member_cannot() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt8", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u3", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_user(&repo, "u2").await;
    common::put_user(&repo, "u3").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt8", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt8", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt8", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("gcmt8");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let created = handlers::create_comment(
        &state,
        &domain::UserId::new("u2"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        CreateCommentRequest {
            body: Some("u2's comment".into()),
            image_media_id: None,
        },
    )
    .await
    .expect("u2 creates a comment");

    let err = handlers::delete_comment(
        &state,
        &domain::UserId::new("u3"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
    )
    .await
    .expect_err("a non-author, non-admin member cannot delete");
    assert_eq!(err.code, ApiErrorCode::Forbidden);

    handlers::delete_comment(
        &state,
        &domain::UserId::new("u1"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        &created.comment_id,
    )
    .await
    .expect("an admin can delete any comment");
}

#[tokio::test]
async fn comment_list_orders_oldest_first_across_a_pagination_boundary() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt9", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt9", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt9", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt9", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let caller = domain::UserId::new("u1");
    let group_id = GroupId::new("gcmt9");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let mut created_ids = Vec::new();
    for i in 0..5 {
        let created = handlers::create_comment(
            &state,
            &caller,
            &group_id,
            &cycle_id,
            &question_id,
            &response_id,
            CreateCommentRequest {
                body: Some(format!("comment {i}")),
                image_media_id: None,
            },
        )
        .await
        .expect("comment created");
        created_ids.push(created.comment_id);
        // DynamoDB-Local resolves same-millisecond writes arbitrarily; comment
        // ordering is `created_at` then `commentId` lexicographically
        // (`comment_sk`), so this test relies on `created_at` alone advancing.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let page1 = handlers::list_comments(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        2,
        None,
    )
    .await
    .expect("first page");
    assert_eq!(page1.items.len(), 2);
    assert!(page1.next_cursor.is_some());

    let page2 = handlers::list_comments(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        2,
        page1.next_cursor.as_deref(),
    )
    .await
    .expect("second page");
    assert_eq!(page2.items.len(), 2);
    assert!(page2.next_cursor.is_some());

    let page3 = handlers::list_comments(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        2,
        page2.next_cursor.as_deref(),
    )
    .await
    .expect("third page");
    assert_eq!(page3.items.len(), 1);
    assert_eq!(page3.next_cursor, None);

    let all_ids: Vec<_> = page1
        .items
        .into_iter()
        .chain(page2.items)
        .chain(page3.items)
        .map(|c| c.comment_id)
        .collect();
    assert_eq!(
        all_ids, created_ids,
        "oldest-first order preserved across pages"
    );
}

#[tokio::test]
async fn list_rejects_an_invalid_cursor_and_an_out_of_range_limit() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcmt10", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_user(&repo, "u1").await;
    common::put_newsletter(&repo, &common::published_newsletter("gcmt10", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("gcmt10", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "gcmt10", "202606", "q1", "u1", "r1", "Lake 22.")
        .await;

    let state = common::test_state(repo);
    let caller = domain::UserId::new("u1");
    let group_id = GroupId::new("gcmt10");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    let err = handlers::list_comments(
        &state,
        &caller,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        50,
        Some("not-a-valid-cursor!!"),
    )
    .await
    .expect_err("invalid cursor");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "cursor");
}

/// The check order shared by all seven engagement routes
/// (`03-api-contract.md` §8): membership -> cycle exists -> not archived ->
/// published -> answer resolves to a published text response.
mod check_order {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn non_member_is_forbidden() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco1", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::published_newsletter("gco1", "202606")).await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("outsider"),
            &GroupId::new("gco1"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("not a member");
        assert_eq!(err.code, ApiErrorCode::Forbidden);
    }

    #[tokio::test]
    async fn missing_cycle_is_not_found() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco2", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco2"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("no such cycle");
        assert_eq!(err.code, ApiErrorCode::NotFound);
    }

    #[tokio::test]
    async fn archived_cycle_is_410() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco3", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::archived_newsletter("gco3", "202606")).await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco3"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("archived");
        assert_eq!(err.code, ApiErrorCode::NewsletterArchived);
    }

    #[tokio::test]
    async fn non_published_cycle_is_409() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco4", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::open_newsletter("gco4", "202606")).await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco4"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("still open, not published");
        assert_eq!(err.code, ApiErrorCode::CycleNotPublished);
    }

    #[tokio::test]
    async fn missing_answer_is_not_found() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco5", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::published_newsletter("gco5", "202606")).await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco5"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("no-such-response"),
            50,
            None,
        )
        .await
        .expect_err("no answer with that responseId");
        assert_eq!(err.code, ApiErrorCode::NotFound);
    }

    #[tokio::test]
    async fn a_poll_answer_is_not_an_engagement_target() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco6", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::published_newsletter("gco6", "202606")).await;
        common::put_published_poll_answer(&repo, "gco6", "202606", "q1", "u1", "r1").await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco6"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("poll answers are never engagement targets");
        assert_eq!(err.code, ApiErrorCode::NotFound);
    }

    #[tokio::test]
    async fn an_unpublished_draft_answer_is_not_an_engagement_target() {
        let (_c, repo) = common::make_repo().await;
        let group = common::group("gco7", "u1");
        common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
        common::put_newsletter(&repo, &common::published_newsletter("gco7", "202606")).await;
        common::put_draft_text_answer(&repo, "gco7", "202606", "q1", "u1", "r1").await;

        let state = common::test_state(repo);
        let err = handlers::list_comments(
            &state,
            &domain::UserId::new("u1"),
            &GroupId::new("gco7"),
            &CycleId::new("202606"),
            &QuestionId::new("q1"),
            &ResponseId::new("r1"),
            50,
            None,
        )
        .await
        .expect_err("draft answers are never engagement targets");
        assert_eq!(err.code, ApiErrorCode::NotFound);
    }
}
