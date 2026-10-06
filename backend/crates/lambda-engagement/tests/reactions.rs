//! `GET .../reactions`, `PUT`/`DELETE .../reactions/{emoji}`
//! (`plans/03-api-contract.md` §8.5-§8.7).
//!
//! Each test uses its own group id (never a shared one): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL, and `cargo test` runs many of these tests concurrently in the
//! same process.

mod common;

use domain::{ApiErrorCode, CycleId, GroupId, QuestionId, ResponseId, Role};
use engagement::handlers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn put_is_idempotent_and_toggles_on() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn1", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    common::put_newsletter(&repo, &common::published_newsletter("grxn1", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn1", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn1", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn1");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let u2 = domain::UserId::new("u2");

    let first = handlers::put_reaction(
        &state,
        &u2,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .expect("first put");
    assert_eq!(first.reaction_groups.len(), 1);
    assert_eq!(first.reaction_groups[0].emoji, "🔥");
    assert_eq!(first.reaction_groups[0].count, 1);
    assert!(first.reaction_groups[0].reacted_by_me);
    assert_eq!(first.my_reactions, vec!["🔥".to_string()]);

    // Idempotent re-put: still exactly one reaction.
    let second = handlers::put_reaction(
        &state,
        &u2,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .expect("idempotent re-put");
    assert_eq!(second.reaction_groups.len(), 1);
    assert_eq!(second.reaction_groups[0].count, 1);
}

#[tokio::test]
async fn delete_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn2", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_newsletter(&repo, &common::published_newsletter("grxn2", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn2", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn2", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn2");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let u1 = domain::UserId::new("u1");

    handlers::put_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .expect("put");

    let after_first_delete = handlers::delete_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .expect("delete");
    assert_eq!(after_first_delete.reaction_groups.len(), 0);
    assert_eq!(after_first_delete.my_reactions.len(), 0);

    // Deleting again (nothing to remove) still succeeds and returns the same shape.
    let after_second_delete = handlers::delete_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .expect("idempotent delete");
    assert_eq!(after_second_delete.reaction_groups.len(), 0);
}

#[tokio::test]
async fn a_zwj_family_emoji_round_trips_through_put_and_get() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn3", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_newsletter(&repo, &common::published_newsletter("grxn3", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn3", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn3", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn3");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let u1 = domain::UserId::new("u1");

    let family = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
    handlers::put_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        family,
    )
    .await
    .expect("ZWJ sequence stored");

    let fetched = handlers::get_reactions(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
    )
    .await
    .expect("reactions fetched");
    assert_eq!(fetched.reaction_groups.len(), 1);
    assert_eq!(fetched.reaction_groups[0].emoji, family);
    assert_eq!(fetched.my_reactions, vec![family.to_string()]);
}

#[tokio::test]
async fn a_percent_encoded_emoji_path_param_is_decoded() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn4", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_newsletter(&repo, &common::published_newsletter("grxn4", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn4", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn4", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn4");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let u1 = domain::UserId::new("u1");

    // %F0%9F%94%A5 is the percent-encoded UTF-8 bytes of 🔥.
    let result = handlers::put_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "%F0%9F%94%A5",
    )
    .await
    .expect("percent-encoded emoji accepted");
    assert_eq!(result.reaction_groups[0].emoji, "🔥");
}

#[tokio::test]
async fn an_invalid_emoji_is_rejected() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn5", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    common::put_newsletter(&repo, &common::published_newsletter("grxn5", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn5", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn5", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn5");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");
    let u1 = domain::UserId::new("u1");

    let err = handlers::put_reaction(
        &state,
        &u1,
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "ab",
    )
    .await
    .expect_err("not an emoji");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "emoji");
}

#[tokio::test]
async fn reaction_groups_are_ordered_by_count_then_earliest_then_emoji() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("grxn6", "u1");
    for u in ["u1", "u2", "u3"] {
        common::seed_group_and_membership(&repo, &group, u, Role::Member).await;
    }
    common::put_newsletter(&repo, &common::published_newsletter("grxn6", "202606")).await;
    common::put_locked_question(
        &repo,
        &common::locked_question("grxn6", "202606", "q1", "u1"),
    )
    .await;
    common::put_published_text_answer(&repo, "grxn6", "202606", "q1", "u1", "r1", "Lake 22.").await;

    let state = common::test_state(repo);
    let group_id = GroupId::new("grxn6");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let response_id = ResponseId::new("r1");

    // 🤣 gets one early reaction; 🔥 gets two later reactions. 🔥 should sort
    // first (higher count) even though 🤣 came first chronologically.
    handlers::put_reaction(
        &state,
        &domain::UserId::new("u1"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🤣",
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    handlers::put_reaction(
        &state,
        &domain::UserId::new("u2"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .unwrap();
    handlers::put_reaction(
        &state,
        &domain::UserId::new("u3"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
        "🔥",
    )
    .await
    .unwrap();

    let result = handlers::get_reactions(
        &state,
        &domain::UserId::new("u1"),
        &group_id,
        &cycle_id,
        &question_id,
        &response_id,
    )
    .await
    .expect("reactions fetched");
    assert_eq!(
        result
            .reaction_groups
            .iter()
            .map(|g| g.emoji.as_str())
            .collect::<Vec<_>>(),
        vec!["🔥", "🤣"]
    );
}
