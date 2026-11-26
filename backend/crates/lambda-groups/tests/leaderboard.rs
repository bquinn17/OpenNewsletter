//! `GET /groups/{groupId}/leaderboard` (`docs/design/api.md` §4.4).
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`, and tests run concurrently.

mod common;

use common::{put_locked_question, put_reacted_response, seed, state};
use domain::{
    ApiErrorCode, CycleId, GroupId, LockedQuestion, MemberStats, QuestionId, QuestionKind,
    Response, ResponseId, ResponseStatus, Role, Timestamp, UserId,
};
use groups::group_routes;
use persistence::test_factories;
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
        locked_at: "2026-06-01T00:00:00Z".parse::<Timestamp>().unwrap(),
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
        audio_media_ids: Vec::new(),
        updated_at: "2026-06-03T00:00:00Z".parse::<Timestamp>().unwrap(),
        published_at: Some("2026-06-03T00:00:00Z".parse::<Timestamp>().unwrap()),
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
    test_factories::put_member_stats(
        &state.repo,
        &MemberStats {
            group_id: GroupId::new("glb-departed"),
            user_id: UserId::new("u-gone"),
            editions_answered: 0,
            reactions_received: 0,
            reactions_given: 0,
            comments_written: 0,
            questions_asked: 0,
            current_streak: 3,
            longest_streak: 3,
            streak_cycle: Some(CycleId::new("202605")),
        },
    )
    .await;

    let board =
        group_routes::get_leaderboard(&state, &UserId::new("a1"), &GroupId::new("glb-departed"))
            .await
            .unwrap();

    assert_eq!(board.members.len(), 1);
    assert_eq!(board.members[0].user_id, UserId::new("a1"));
}

#[tokio::test]
async fn top_answers_are_ordered_limited_to_three_ranks_and_filtered() {
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
    // leaving three distinct counts, one per rank.
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

#[tokio::test]
async fn tied_top_answers_all_stay_and_a_fourth_rank_is_left_off() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let users = ["a1", "m1", "m2", "m3", "m4", "m5", "m6"];
    let roles: Vec<_> = users
        .iter()
        .map(|u| {
            (
                *u,
                if *u == "a1" {
                    Role::Admin
                } else {
                    Role::Member
                },
            )
        })
        .collect();
    seed(&repo, "glb-ties", &roles).await;
    let state = state(repo);
    put_locked_question(
        &state.repo,
        &locked_question("glb-ties", "202606", "q1", "Tied prompt"),
    )
    .await;

    // Counts 5, 5, 5, 5, 3, 2, 1: four answers tie for first, and the 1 is a
    // fourth distinct count, past the board's three ranks.
    for (i, (user, count)) in [
        ("a1", 5),
        ("m1", 5),
        ("m2", 5),
        ("m3", 5),
        ("m4", 3),
        ("m5", 2),
        ("m6", 1),
    ]
    .into_iter()
    .enumerate()
    {
        put_reacted_response(
            &state.repo,
            &reacted_answer("glb-ties", "202606", "q1", &format!("r{i}"), user, count),
        )
        .await;
    }

    let board =
        group_routes::get_leaderboard(&state, &UserId::new("a1"), &GroupId::new("glb-ties"))
            .await
            .unwrap();

    assert_eq!(
        board
            .top_answers
            .iter()
            .map(|a| a.reaction_count)
            .collect::<Vec<_>>(),
        vec![5, 5, 5, 5, 3, 2]
    );
}

/// The safety cap (`shared::config::LEADERBOARD_TOP_ANSWERS_MAX`) never
/// splits a tie group below the first rank: one that doesn't fit is dropped.
#[tokio::test]
async fn the_top_answers_cap_drops_a_tie_group_that_would_overflow_it() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let max = shared::config::LEADERBOARD_TOP_ANSWERS_MAX;
    put_reacted_response(
        &repo,
        &reacted_answer("glb-cap", "202606", "q1", "r-top", "u-top", 4),
    )
    .await;
    for i in 0..max {
        put_reacted_response(
            &repo,
            &reacted_answer(
                "glb-cap",
                "202606",
                "q1",
                &format!("r{i:03}"),
                &format!("u{i}"),
                1,
            ),
        )
        .await;
    }

    let top =
        persistence::responses::list_top_reacted_answers(&repo, &GroupId::new("glb-cap"), |_| true)
            .await
            .unwrap();

    assert_eq!(
        top.iter().map(|a| a.reaction_count).collect::<Vec<_>>(),
        vec![4],
        "the {max} answers tied at 1 don't fit beside the first rank, so none show"
    );
}

/// The first rank alone is the one group the cap cuts, so the board is never
/// empty just because too many answers tie for first.
#[tokio::test]
async fn the_top_answers_cap_cuts_an_oversized_first_rank() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let max = shared::config::LEADERBOARD_TOP_ANSWERS_MAX;
    for i in 0..=max {
        put_reacted_response(
            &repo,
            &reacted_answer(
                "glb-cap1",
                "202606",
                "q1",
                &format!("r{i:03}"),
                &format!("u{i}"),
                2,
            ),
        )
        .await;
    }

    let top =
        persistence::responses::list_top_reacted_answers(&repo, &GroupId::new("glb-cap1"), |_| {
            true
        })
        .await
        .unwrap();

    assert_eq!(top.len(), max);
}
