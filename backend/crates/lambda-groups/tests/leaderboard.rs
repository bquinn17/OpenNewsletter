//! `GET /groups/{groupId}/leaderboard` (`docs/design/api.md` §4.4).
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`, and tests run concurrently.

mod common;

use chrono::{TimeZone, Utc};
use common::{put_locked_question, put_reacted_response, seed, state};
use domain::{
    ApiErrorCode, CycleId, GroupId, LockedQuestion, QuestionId, QuestionKind, Response, ResponseId,
    ResponseStatus, Role, UserId,
};
use groups::group_routes;
use persistence::{member_stats, test_factories};
use pretty_assertions::assert_eq;

fn locked_question(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    prompt: &str,
) -> LockedQuestion {
    LockedQuestion {
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        prompt: prompt.to_owned(),
        poll_options: None,
        display_order: 0,
        submitted_by: UserId::new("u1"),
        is_anonymous: false,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    }
}

fn reacted_answer(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    response_id: &str,
    user_id: &str,
    reaction_count: u32,
) -> Response {
    Response {
        response_id: ResponseId::new(response_id),
        user_id: UserId::new(user_id),
        question_id: QuestionId::new(question_id),
        group_id: GroupId::new(group_id),
        cycle_id: CycleId::new(cycle_id),
        kind: QuestionKind::Text,
        status: ResponseStatus::Published,
        body: Some("An answer.".into()),
        poll_option_id: None,
        image_media_ids: Vec::new(),
        updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
        published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
        reaction_count,
    }
}

#[tokio::test]
async fn non_member_is_forbidden() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "glb-forbidden", &[("a1", Role::Admin)]).await;
    let state = state(repo);

    let err = group_routes::get_leaderboard(
        &state,
        &UserId::new("outsider"),
        &GroupId::new("glb-forbidden"),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn a_member_with_no_stats_row_gets_all_zeros() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(
        &repo,
        "glb-zeros",
        &[("a1", Role::Admin), ("m1", Role::Member)],
    )
    .await;
    let state = state(repo);

    let board =
        group_routes::get_leaderboard(&state, &UserId::new("m1"), &GroupId::new("glb-zeros"))
            .await
            .unwrap();

    assert_eq!(board.members.len(), 2);
    let m1 = board
        .members
        .iter()
        .find(|m| m.user_id == UserId::new("m1"))
        .unwrap();
    assert_eq!(m1.editions_answered, 0);
    assert_eq!(m1.reactions_received, 0);
    assert_eq!(m1.reactions_given, 0);
    assert_eq!(m1.comments_written, 0);
    assert_eq!(m1.questions_asked, 0);
    assert_eq!(m1.current_streak, 0);
    assert_eq!(m1.longest_streak, 0);
}

#[tokio::test]
async fn a_departed_members_stats_are_excluded() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "glb-departed", &[("a1", Role::Admin)]).await;
    let state = state(repo);
    // u-gone has a stats row but is not a current member.
    member_stats::settle_streak(
        &state.repo,
        &GroupId::new("glb-departed"),
        &UserId::new("u-gone"),
        3,
        3,
        &CycleId::new("202605"),
    )
    .await
    .unwrap();

    let board =
        group_routes::get_leaderboard(&state, &UserId::new("a1"), &GroupId::new("glb-departed"))
            .await
            .unwrap();

    assert_eq!(board.members.len(), 1);
    assert_eq!(board.members[0].user_id, UserId::new("a1"));
}

#[tokio::test]
async fn top_answers_are_ordered_capped_at_three_and_filtered() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(
        &repo,
        "glb-top",
        &[
            ("a1", Role::Admin),
            ("m1", Role::Member),
            ("m2", Role::Member),
            ("m3", Role::Member),
        ],
    )
    .await;
    let state = state(repo);

    put_locked_question(
        &state.repo,
        &locked_question(
            "glb-top",
            "202606",
            "q1",
            "What was the best thing you ate?",
        ),
    )
    .await;
    put_locked_question(
        &state.repo,
        &locked_question("glb-top", "202606", "q2", "Second prompt"),
    )
    .await;
    put_locked_question(
        &state.repo,
        &locked_question("glb-top", "202606", "q3", "Third prompt"),
    )
    .await;
    put_locked_question(
        &state.repo,
        &locked_question("glb-top", "202606", "q4", "Fourth prompt"),
    )
    .await;

    // Five candidates: four current-member answers (one with 0 reactions,
    // filtered out) and one departed member's answer (also filtered out),
    // so only 3 of the 4 remaining qualify for the cap.
    put_reacted_response(
        &state.repo,
        &reacted_answer("glb-top", "202606", "q1", "r1", "m1", 8),
    )
    .await;
    put_reacted_response(
        &state.repo,
        &reacted_answer("glb-top", "202606", "q2", "r2", "m2", 5),
    )
    .await;
    put_reacted_response(
        &state.repo,
        &reacted_answer("glb-top", "202606", "q3", "r3", "m3", 3),
    )
    .await;
    put_reacted_response(
        &state.repo,
        &reacted_answer("glb-top", "202606", "q4", "r4", "m1", 0),
    )
    .await;
    put_reacted_response(
        &state.repo,
        &reacted_answer("glb-top", "202606", "q1", "r5", "u-departed", 9),
    )
    .await;

    let board = group_routes::get_leaderboard(&state, &UserId::new("a1"), &GroupId::new("glb-top"))
        .await
        .unwrap();

    assert_eq!(board.top_answers.len(), 3);
    assert_eq!(
        board
            .top_answers
            .iter()
            .map(|a| a.reaction_count)
            .collect::<Vec<_>>(),
        vec![8, 5, 3]
    );
    assert_eq!(
        board.top_answers[0].question_prompt,
        "What was the best thing you ate?"
    );
    assert!(
        board
            .top_answers
            .iter()
            .all(|a| a.user_id != UserId::new("u-departed")),
        "a departed member's answer must never appear"
    );
    assert!(
        board.top_answers.iter().all(|a| a.reaction_count > 0),
        "zero-count rows must be filtered out"
    );
}
