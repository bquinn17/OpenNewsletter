mod common;

use domain::{CycleId, GroupId, QuestionId, UserId};
use persistence::engagement;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_puts_and_lists_comments() {
    let (_c, repo) = common::make_repo().await;
    let c1 = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-01");
    let c2 = common::comment("g1", "202606", "q1", "u1", "u3", "cmt-02");
    engagement::put_comment(&repo, &c1).await.unwrap();
    engagement::put_comment(&repo, &c2).await.unwrap();

    let comments = engagement::list_comments(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert_eq!(comments.len(), 2);
}

#[tokio::test]
async fn it_soft_deletes_a_comment() {
    let (_c, repo) = common::make_repo().await;
    let c = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-01");
    engagement::put_comment(&repo, &c).await.unwrap();

    let created_at_iso = c.created_at.to_rfc3339();
    engagement::soft_delete_comment(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
        &created_at_iso,
        &c.comment_id,
        "2026-06-07T10:00:00+00:00",
    )
    .await
    .unwrap();

    let comments = engagement::list_comments(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    // Comment row still exists (soft-deleted).
    assert_eq!(comments.len(), 1);
    assert!(comments[0].deleted_at.is_some());
    assert_eq!(comments[0].body, ""); // body cleared
}

#[tokio::test]
async fn it_puts_and_lists_reactions() {
    let (_c, repo) = common::make_repo().await;
    let r1 = common::reaction("g1", "202606", "q1", "u1", "u2");
    let mut r2 = common::reaction("g1", "202606", "q1", "u1", "u3");
    r2.emoji = "👍".into();
    engagement::put_reaction(&repo, &r1).await.unwrap();
    engagement::put_reaction(&repo, &r2).await.unwrap();

    let reactions = engagement::list_reactions(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert_eq!(reactions.len(), 2);
}

#[tokio::test]
async fn it_toggles_reaction_off_by_deleting_it() {
    let (_c, repo) = common::make_repo().await;
    let r = common::reaction("g1", "202606", "q1", "u1", "u2");
    engagement::put_reaction(&repo, &r).await.unwrap();

    engagement::delete_reaction(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
        &UserId::new("u2"),
        &r.emoji,
    )
    .await
    .unwrap();

    let reactions = engagement::list_reactions(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert_eq!(reactions.len(), 0);
}
