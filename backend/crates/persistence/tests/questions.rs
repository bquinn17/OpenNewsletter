mod common;

use chrono::Utc;
use domain::{CycleId, GroupId, NewsletterStatus};
use persistence::questions;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_puts_and_lists_candidates() {
    let (_c, repo) = common::make_repo().await;
    let q1 = common::candidate("g1", "202606", "q1", "u1", 0);
    let q2 = common::candidate("g1", "202606", "q2", "u1", 0);
    questions::put_candidate(&repo, &q1).await.unwrap();
    questions::put_candidate(&repo, &q2).await.unwrap();

    let mut list = questions::list_candidates(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
        .await
        .unwrap();
    list.sort_by(|a, b| a.question_id.as_str().cmp(b.question_id.as_str()));
    assert_eq!(list.len(), 2);
    assert_eq!(list[0], q1);
    assert_eq!(list[1], q2);
}

#[tokio::test]
async fn it_lists_top_candidates_by_votes_descending() {
    let (_c, repo) = common::make_repo().await;
    let low = common::candidate("g1", "202606", "q-low", "u1", 1);
    let mid = common::candidate("g1", "202606", "q-mid", "u1", 5);
    let top = common::candidate("g1", "202606", "q-top", "u1", 10);
    questions::put_candidate(&repo, &low).await.unwrap();
    questions::put_candidate(&repo, &mid).await.unwrap();
    questions::put_candidate(&repo, &top).await.unwrap();

    let top2 = questions::list_top_candidates_by_votes(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        2,
    )
    .await
    .unwrap();
    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].question_id, top.question_id);
    assert_eq!(top2[1].question_id, mid.question_id);
}

#[tokio::test]
async fn it_lists_my_votes_for_a_cycle() {
    let (_c, repo) = common::make_repo().await;
    let q1 = common::candidate("g1", "202606", "q1", "u1", 0);
    let q2 = common::candidate("g1", "202606", "q2", "u1", 0);
    questions::put_candidate(&repo, &q1).await.unwrap();
    questions::put_candidate(&repo, &q2).await.unwrap();

    let v1 = common::vote("u2", "g1", "202606", "q1");
    questions::cast_vote_tx(&repo, &v1, 1).await.unwrap();

    let votes = questions::list_my_votes(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &domain::UserId::new("u2"),
    )
    .await
    .unwrap();
    assert_eq!(votes.len(), 1);
    assert_eq!(votes[0].question_id, v1.question_id);
}

#[tokio::test]
async fn it_lists_locked_questions_for_a_cycle() {
    let (_c, repo) = common::make_repo().await;
    let lq1 = common::locked_question("g1", "202606", "q1", "u1");
    let lq2 = common::locked_question("g1", "202606", "q2", "u1");
    // Use promote_candidates_tx to write them with the newsletter.
    let mut nl = common::voting_newsletter("g1", "202606");
    nl.locked_question_ids = vec![lq1.question_id.clone(), lq2.question_id.clone()];
    nl.next_transition_at = Some(Utc::now());
    persistence::newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    questions::promote_candidates_tx(&repo, &[lq1.clone(), lq2.clone()], &nl)
        .await
        .unwrap();

    let mut locked =
        questions::list_locked_questions(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
            .await
            .unwrap();
    locked.sort_by(|a, b| a.question_id.as_str().cmp(b.question_id.as_str()));
    assert_eq!(locked.len(), 2);
    assert_eq!(locked[0].question_id, lq1.question_id);
    assert_eq!(locked[1].question_id, lq2.question_id);
}

#[tokio::test]
async fn cast_vote_tx_increments_vote_count_and_records_vote() {
    let (_c, repo) = common::make_repo().await;
    let q = common::candidate("g1", "202606", "q1", "u1", 0);
    questions::put_candidate(&repo, &q).await.unwrap();

    let v = common::vote("u2", "g1", "202606", "q1");
    questions::cast_vote_tx(&repo, &v, 1).await.unwrap();

    let updated = questions::list_candidates(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
        .await
        .unwrap();
    assert_eq!(updated.len(), 1);
    assert_eq!(updated[0].vote_count, 1);

    let votes = questions::list_my_votes(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &domain::UserId::new("u2"),
    )
    .await
    .unwrap();
    assert_eq!(votes.len(), 1);
}

#[tokio::test]
async fn cast_vote_tx_fails_on_duplicate_vote() {
    let (_c, repo) = common::make_repo().await;
    let q = common::candidate("g1", "202606", "q1", "u1", 0);
    questions::put_candidate(&repo, &q).await.unwrap();

    let v = common::vote("u2", "g1", "202606", "q1");
    questions::cast_vote_tx(&repo, &v, 1).await.unwrap();

    // Casting the same vote again must fail (put_vote condition: attribute_not_exists).
    let result = questions::cast_vote_tx(&repo, &v, 2).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn withdraw_vote_tx_decrements_count_and_removes_vote() {
    let (_c, repo) = common::make_repo().await;
    let q = common::candidate("g1", "202606", "q1", "u1", 0);
    questions::put_candidate(&repo, &q).await.unwrap();

    let v = common::vote("u2", "g1", "202606", "q1");
    questions::cast_vote_tx(&repo, &v, 1).await.unwrap();
    questions::withdraw_vote_tx(&repo, &v, 0).await.unwrap();

    let updated = questions::list_candidates(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
        .await
        .unwrap();
    assert_eq!(updated[0].vote_count, 0);

    let votes = questions::list_my_votes(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &domain::UserId::new("u2"),
    )
    .await
    .unwrap();
    assert_eq!(votes.len(), 0);
}

#[tokio::test]
async fn promote_candidates_tx_creates_locked_questions_and_flips_status_to_open() {
    let (_c, repo) = common::make_repo().await;
    let mut nl = common::voting_newsletter("g1", "202606");
    persistence::newsletters::write_status_transition(&repo, &nl)
        .await
        .unwrap();

    let lq = common::locked_question("g1", "202606", "q1", "u1");
    nl.status = NewsletterStatus::Open;
    nl.locked_question_ids = vec![lq.question_id.clone()];
    nl.next_transition_at = Some(Utc::now());
    questions::promote_candidates_tx(&repo, std::slice::from_ref(&lq), &nl)
        .await
        .unwrap();

    let locked =
        questions::list_locked_questions(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
            .await
            .unwrap();
    assert_eq!(locked.len(), 1);

    let updated_nl = persistence::newsletters::get_newsletter(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated_nl.status, NewsletterStatus::Open);
}
