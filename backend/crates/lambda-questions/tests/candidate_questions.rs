//! Handler-level tests for `lambda-questions` (`plans/03-api-contract.md` §6).
//!
//! Each test uses its own group id (never a shared "g1"): `persistence::auth`
//! keeps a process-wide membership cache keyed by `(userId, groupId)` with a
//! 60s TTL (`plans/02-data-model-dynamodb.md` §6), and `cargo test` runs many
//! of these tests concurrently in the same process. Reusing a group id across
//! tests let one test's cached role leak into another's assertions.

mod common;

use domain::api::CreateCandidateRequest;
use domain::{ApiErrorCode, CycleId, GroupId, QuestionId, Role, UserId};
use persistence::{newsletters, questions};
use pretty_assertions::assert_eq;
// Leading `::` resolves to the extern crate `questions` (this Lambda's own
// library, per `lambda-questions/Cargo.toml`'s `[lib] name = "questions"`)
// rather than the `persistence::questions` module imported just above.
use ::questions::handlers::{self, Sort};
use ::questions::state::AppState;

fn state(repo: persistence::Repo) -> AppState {
    AppState {
        repo,
        cdn_base_url: String::new(),
    }
}

fn text_request(prompt: &str) -> CreateCandidateRequest {
    CreateCandidateRequest {
        kind: domain::QuestionKind::Text,
        prompt: prompt.to_owned(),
        poll_options: None,
        is_anonymous: false,
    }
}

#[tokio::test]
async fn non_member_is_forbidden_on_every_route() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnm", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    let voting = common::voting_newsletter("gnm", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    let state = state(repo);
    let outsider = UserId::new("intruder");
    let group_id = GroupId::new("gnm");

    let list_err = handlers::list_candidates(&state, &outsider, &group_id, Sort::Top, 20, None)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(list_err.code, ApiErrorCode::Forbidden);

    let create_err = handlers::create_candidate(&state, &outsider, &group_id, text_request("Hi?"))
        .await
        .expect_err("non-member must be refused");
    assert_eq!(create_err.code, ApiErrorCode::Forbidden);

    let question_id = QuestionId::new("q1");
    let vote_err = handlers::cast_vote(&state, &outsider, &group_id, &question_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(vote_err.code, ApiErrorCode::Forbidden);

    let withdraw_err = handlers::withdraw_vote(&state, &outsider, &group_id, &question_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(withdraw_err.code, ApiErrorCode::Forbidden);

    let delete_err = handlers::admin_delete_candidate(&state, &outsider, &group_id, &question_id)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(delete_err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn member_cannot_admin_delete() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gmc", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    let voting = common::voting_newsletter("gmc", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gmc", "202606", "q1", "u1", 0))
        .await
        .unwrap();

    let state = state(repo);
    let err = handlers::admin_delete_candidate(
        &state,
        &UserId::new("u2"),
        &GroupId::new("gmc"),
        &QuestionId::new("q1"),
    )
    .await
    .expect_err("member (non-admin) must be refused");
    assert_eq!(err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn creating_a_candidate_requires_an_eligible_voting_cycle() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnc", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    // No Newsletter row exists at all yet.

    let state = state(repo);
    let err = handlers::create_candidate(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gnc"),
        text_request("What's your favorite hike?"),
    )
    .await
    .expect_err("no voting cycle exists");
    assert_eq!(err.code, ApiErrorCode::CycleNotVoting);
}

#[tokio::test]
async fn voting_on_a_non_voting_cycle_is_refused() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnv", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    // Only an `open` cycle exists — no `voting` cycle to attach a vote to.
    let mut open = common::voting_newsletter("gnv", "202606");
    open.status = domain::NewsletterStatus::Open;
    newsletters::write_status_transition(&repo, &open)
        .await
        .unwrap();

    let state = state(repo);
    let err = handlers::cast_vote(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gnv"),
        &QuestionId::new("q1"),
    )
    .await
    .expect_err("cycle is not in voting");
    assert_eq!(err.code, ApiErrorCode::CycleNotVoting);
}

#[tokio::test]
async fn create_candidate_populates_the_submitters_own_attribution() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gat", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    let voting = common::voting_newsletter("gat", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    persistence::users::put_user(&repo, &common::user("u1"))
        .await
        .unwrap();

    let state = state(repo);
    let mut request = text_request("What's your favorite hike?");
    request.is_anonymous = true;
    let item =
        handlers::create_candidate(&state, &UserId::new("u1"), &GroupId::new("gat"), request)
            .await
            .expect("member can submit a candidate");

    assert!(item.is_anonymous);
    assert_eq!(
        item.asked_by
            .expect("submitter always sees own attribution")
            .user_id,
        UserId::new("u1")
    );
    assert_eq!(item.vote_count, 0);
    assert!(!item.voted_by_me);
}

#[tokio::test]
async fn vote_cap_is_enforced() {
    let (_c, repo) = common::make_repo().await;
    let mut group = common::group("gvc", "u1");
    group.cycle_settings.votes_per_user_per_cycle = 1;
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let voting = common::voting_newsletter("gvc", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gvc", "202606", "q1", "u2", 0))
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gvc", "202606", "q2", "u2", 0))
        .await
        .unwrap();

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gvc");

    let first = handlers::cast_vote(&state, &caller, &group_id, &QuestionId::new("q1"))
        .await
        .expect("first vote is within the cap");
    assert!(first.voted_by_me);
    assert_eq!(first.my_vote_count, 1);

    let second = handlers::cast_vote(&state, &caller, &group_id, &QuestionId::new("q2"))
        .await
        .expect_err("second vote exceeds votesPerUserPerCycle=1");
    assert_eq!(second.code, ApiErrorCode::VoteCapReached);
}

#[tokio::test]
async fn casting_the_same_vote_twice_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcv", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let voting = common::voting_newsletter("gcv", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gcv", "202606", "q1", "u2", 0))
        .await
        .unwrap();

    let state = state(repo);
    let caller = UserId::new("u1");
    let group_id = GroupId::new("gcv");
    let question_id = QuestionId::new("q1");

    let first = handlers::cast_vote(&state, &caller, &group_id, &question_id)
        .await
        .unwrap();
    assert_eq!(first.vote_count, 1);

    let second = handlers::cast_vote(&state, &caller, &group_id, &question_id)
        .await
        .expect("re-voting is idempotent, not an error");
    assert_eq!(second.vote_count, 1);
    assert_eq!(second.my_vote_count, 1);
}

#[tokio::test]
async fn withdrawing_a_vote_that_was_never_cast_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gwv", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Member).await;
    let voting = common::voting_newsletter("gwv", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gwv", "202606", "q1", "u2", 3))
        .await
        .unwrap();

    let state = state(repo);
    let response = handlers::withdraw_vote(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gwv"),
        &QuestionId::new("q1"),
    )
    .await
    .expect("withdrawing a nonexistent vote is a no-op, not an error");
    assert!(!response.voted_by_me);
    assert_eq!(response.vote_count, 3);
}

#[tokio::test]
async fn admin_delete_cascades_votes_from_every_voter() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gcd", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    common::seed_group_and_membership(&repo, &group, "u3", Role::Member).await;
    let voting = common::voting_newsletter("gcd", "202606");
    newsletters::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gcd", "202606", "q1", "u2", 0))
        .await
        .unwrap();

    let state = state(repo);
    let group_id = GroupId::new("gcd");
    let question_id = QuestionId::new("q1");

    handlers::cast_vote(&state, &UserId::new("u2"), &group_id, &question_id)
        .await
        .unwrap();
    handlers::cast_vote(&state, &UserId::new("u3"), &group_id, &question_id)
        .await
        .unwrap();

    handlers::admin_delete_candidate(&state, &UserId::new("u1"), &group_id, &question_id)
        .await
        .expect("admin can delete a still-voting candidate");

    let remaining = questions::list_candidates(&state.repo, &group_id, &CycleId::new("202606"))
        .await
        .unwrap();
    assert!(remaining.is_empty());

    for voter in ["u2", "u3"] {
        let votes = questions::list_my_votes(
            &state.repo,
            &group_id,
            &CycleId::new("202606"),
            &UserId::new(voter),
        )
        .await
        .unwrap();
        assert!(votes.is_empty(), "vote cascade must remove {voter}'s vote");
    }
}

#[tokio::test]
async fn admin_delete_refuses_a_candidate_that_has_already_been_promoted() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gap", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;

    // Simulate the tick having just promoted "q1" into the now-`open` cycle.
    let lq = common::locked_question("gap", "202606", "q1", "u2");
    let mut nl = common::voting_newsletter("gap", "202606");
    newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();
    nl.status = domain::NewsletterStatus::Open;
    nl.locked_question_ids = vec![lq.question_id.clone()];
    nl.next_transition_at = Some(nl.response_close_at);
    questions::promote_candidates_tx(&repo, std::slice::from_ref(&lq), &nl)
        .await
        .unwrap();

    let state = state(repo);
    let err = handlers::admin_delete_candidate(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gap"),
        &QuestionId::new("q1"),
    )
    .await
    .expect_err("already-locked candidates can't be deleted");
    assert_eq!(err.code, ApiErrorCode::CandidatePromoted);
}
