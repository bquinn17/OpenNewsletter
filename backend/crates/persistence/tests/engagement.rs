mod common;

use chrono::{TimeZone, Utc};
use domain::{CycleId, GroupId, QuestionId, UserId};
use persistence::engagement::{self, CommentLocation};
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

    let loc = CommentLocation {
        group_id: &GroupId::new("g1"),
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &c.comment_id,
        created_at: c.created_at,
    };
    engagement::soft_delete_comment(
        &repo,
        &loc,
        Utc.with_ymd_and_hms(2026, 6, 7, 10, 0, 0).unwrap(),
        None,
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
async fn it_refuses_to_edit_a_soft_deleted_comment() {
    // An edit racing a delete must not write a body back into the `[deleted]` row.
    let (_c, repo) = common::make_repo().await;
    let c = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-race");
    engagement::put_comment(&repo, &c).await.unwrap();
    let loc = CommentLocation {
        group_id: &GroupId::new("g1"),
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &c.comment_id,
        created_at: c.created_at,
    };
    let edited_at = Utc.with_ymd_and_hms(2026, 6, 7, 10, 0, 0).unwrap();

    // A live comment edits fine.
    engagement::update_comment(&repo, &loc, Some("edited"), None, edited_at, None, None)
        .await
        .unwrap();

    engagement::soft_delete_comment(&repo, &loc, edited_at, None)
        .await
        .unwrap();
    let err = engagement::update_comment(&repo, &loc, Some("revived"), None, edited_at, None, None)
        .await
        .unwrap_err();
    assert!(
        err.is_lost_race(),
        "expected a cancelled transaction, got {err:?}"
    );

    let comments = engagement::list_comments(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert_eq!(comments[0].body, "");
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
