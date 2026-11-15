mod common;

use chrono::{TimeZone, Utc};
use domain::{CycleId, GroupId, ImageId, QuestionId, UserId};
use persistence::engagement::{self, CommentLocation};
use pretty_assertions::assert_eq;

fn stats_for<'a>(
    stats: &'a [domain::MemberStats],
    user_id: &str,
) -> Option<&'a domain::MemberStats> {
    stats.iter().find(|s| s.user_id == UserId::new(user_id))
}

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
        &c.author_user_id,
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
    engagement::update_comment(
        &repo,
        &loc,
        Some("edited"),
        None,
        edited_at,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    engagement::soft_delete_comment(&repo, &loc, &c.author_user_id, edited_at, None)
        .await
        .unwrap();
    let err = engagement::update_comment(
        &repo,
        &loc,
        Some("revived"),
        None,
        edited_at,
        None,
        None,
        None,
    )
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
async fn soft_delete_refuses_a_stale_image_and_leaves_the_comment_live() {
    // A delete that read image A must not land after an edit swapped the
    // comment to image B: it would release A and strand B's claim on a
    // `[deleted]` row.
    let (_c, repo) = common::make_repo().await;
    let mut c = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-stale-del");
    c.image_media_id = Some(ImageId::new("img-b"));
    engagement::put_comment(&repo, &c).await.unwrap();
    let loc = CommentLocation {
        group_id: &GroupId::new("g1"),
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &c.comment_id,
        created_at: c.created_at,
    };
    let at = Utc.with_ymd_and_hms(2026, 6, 7, 10, 0, 0).unwrap();

    for stale in [Some(ImageId::new("img-a")), None] {
        let err =
            engagement::soft_delete_comment(&repo, &loc, &c.author_user_id, at, stale.as_ref())
                .await
                .unwrap_err();
        assert!(
            err.is_lost_race(),
            "expected a cancelled transaction, got {err:?}"
        );
    }
    let comments = engagement::list_comments(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert!(comments[0].deleted_at.is_none());
    assert_eq!(comments[0].image_media_id, Some(ImageId::new("img-b")));

    // With the image it actually holds, the delete goes through.
    engagement::soft_delete_comment(
        &repo,
        &loc,
        &c.author_user_id,
        at,
        Some(&ImageId::new("img-b")),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn update_refuses_a_stale_image() {
    let (_c, repo) = common::make_repo().await;
    let mut c = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-stale-edit");
    c.image_media_id = Some(ImageId::new("img-b"));
    engagement::put_comment(&repo, &c).await.unwrap();
    let loc = CommentLocation {
        group_id: &GroupId::new("g1"),
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &c.comment_id,
        created_at: c.created_at,
    };
    let at = Utc.with_ymd_and_hms(2026, 6, 7, 10, 0, 0).unwrap();
    let stale = ImageId::new("img-a");

    let err = engagement::update_comment(
        &repo,
        &loc,
        None,
        Some(None),
        at,
        Some(&stale),
        Some(&stale),
        None,
    )
    .await
    .unwrap_err();
    assert!(
        err.is_lost_race(),
        "expected a cancelled transaction, got {err:?}"
    );

    let current = ImageId::new("img-b");
    engagement::update_comment(
        &repo,
        &loc,
        Some("still here"),
        None,
        at,
        Some(&current),
        None,
        None,
    )
    .await
    .unwrap();
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

#[tokio::test]
async fn list_engagement_reads_comments_and_reactions_in_one_query() {
    let (_c, repo) = common::make_repo().await;
    let c1 = common::comment("g1", "202606", "q1", "u1", "u2", "cmt-01");
    let c2 = common::comment("g1", "202606", "q1", "u1", "u3", "cmt-02");
    let r1 = common::reaction("g1", "202606", "q1", "u1", "u2");
    let other_answer = common::comment("g1", "202606", "q1", "u9", "u2", "cmt-03");
    for c in [&c2, &c1, &other_answer] {
        engagement::put_comment(&repo, c).await.unwrap();
    }
    engagement::put_reaction(&repo, &r1).await.unwrap();

    let (comments, reactions) = engagement::list_engagement(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();

    // Same order as `list_comments`, and nothing from another answer.
    let ids: Vec<&str> = comments.iter().map(|c| c.comment_id.as_str()).collect();
    assert_eq!(ids, vec!["cmt-01", "cmt-02"]);
    assert_eq!(reactions, vec![r1]);
}

// ---------- §4 #8 — counted reactions ----------

fn published_answer(
    group_id: &str,
    cycle_id: &str,
    question_id: &str,
    user_id: &str,
) -> domain::Response {
    let mut r = common::draft_response(group_id, cycle_id, question_id, user_id);
    r.status = domain::ResponseStatus::Published;
    r.published_at = Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap());
    r
}

#[tokio::test]
async fn put_reaction_counted_bumps_the_response_and_both_stats_rows() {
    let (_c, repo) = common::make_repo().await;
    let answer = published_answer("g-react-1", "202606", "q1", "u1");
    common::put_response(&repo, &answer).await;

    let reaction = common::reaction("g-react-1", "202606", "q1", "u1", "u2");
    engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap();

    let updated = persistence::responses::get_my_response(
        &repo,
        &GroupId::new("g-react-1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.reaction_count, 1);

    let stats = persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-1"))
        .await
        .unwrap();
    assert_eq!(stats_for(&stats, "u1").unwrap().reactions_received, 1);
    assert_eq!(stats_for(&stats, "u2").unwrap().reactions_given, 1);
}

#[tokio::test]
async fn put_reaction_counted_is_idempotent_on_a_duplicate() {
    let (_c, repo) = common::make_repo().await;
    let answer = published_answer("g-react-2", "202606", "q1", "u1");
    common::put_response(&repo, &answer).await;

    let reaction = common::reaction("g-react-2", "202606", "q1", "u1", "u2");
    engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap();
    let err = engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 1)
        .await
        .unwrap_err();
    assert!(
        matches!(err, engagement::ReactionTxError::Duplicate),
        "expected Duplicate, got {err:?}"
    );

    let updated = persistence::responses::get_my_response(
        &repo,
        &GroupId::new("g-react-2"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.reaction_count, 1, "duplicate must not double count");

    let stats = persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-2"))
        .await
        .unwrap();
    assert_eq!(stats_for(&stats, "u1").unwrap().reactions_received, 1);
    assert_eq!(stats_for(&stats, "u2").unwrap().reactions_given, 1);
}

#[tokio::test]
async fn put_reaction_counted_refuses_a_missing_answer_and_writes_nothing() {
    let (_c, repo) = common::make_repo().await;
    // Never stored: stands in for an answer deleted after the handler read it.
    let answer = published_answer("g-react-gone", "202606", "q1", "u1");

    let reaction = common::reaction("g-react-gone", "202606", "q1", "u1", "u2");
    let err = engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap_err();
    assert!(
        matches!(err, engagement::ReactionTxError::CountRace),
        "expected CountRace, got {err:?}"
    );

    // No keys-only Response row came back in its place, so the most-reacted
    // ranking still reads cleanly.
    let response = persistence::responses::get_my_response(
        &repo,
        &GroupId::new("g-react-gone"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert!(response.is_none());
    let top = persistence::responses::list_top_reacted_answers(
        &repo,
        &GroupId::new("g-react-gone"),
        |_| true,
    )
    .await
    .unwrap();
    assert!(top.is_empty());
    let stats =
        persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-gone"))
            .await
            .unwrap();
    assert!(stats.is_empty(), "the stats bumps must roll back too");
}

#[tokio::test]
async fn delete_reaction_counted_reverses_the_bump() {
    let (_c, repo) = common::make_repo().await;
    let answer = published_answer("g-react-3", "202606", "q1", "u1");
    common::put_response(&repo, &answer).await;

    let reaction = common::reaction("g-react-3", "202606", "q1", "u1", "u2");
    engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap();
    engagement::delete_reaction_counted(&repo, &reaction, &answer.response_id, 1)
        .await
        .unwrap();

    let updated = persistence::responses::get_my_response(
        &repo,
        &GroupId::new("g-react-3"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.reaction_count, 0);

    let stats = persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-3"))
        .await
        .unwrap();
    assert_eq!(stats_for(&stats, "u1").unwrap().reactions_received, 0);
    assert_eq!(stats_for(&stats, "u2").unwrap().reactions_given, 0);
}

#[tokio::test]
async fn delete_reaction_counted_is_a_idempotent_no_op_when_repeated() {
    let (_c, repo) = common::make_repo().await;
    let answer = published_answer("g-react-4", "202606", "q1", "u1");
    common::put_response(&repo, &answer).await;

    let reaction = common::reaction("g-react-4", "202606", "q1", "u1", "u2");
    engagement::put_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap();
    engagement::delete_reaction_counted(&repo, &reaction, &answer.response_id, 1)
        .await
        .unwrap();
    let err = engagement::delete_reaction_counted(&repo, &reaction, &answer.response_id, 0)
        .await
        .unwrap_err();
    assert!(
        matches!(err, engagement::ReactionTxError::Duplicate),
        "expected Duplicate (nothing to remove), got {err:?}"
    );

    let stats = persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-4"))
        .await
        .unwrap();
    assert_eq!(
        stats_for(&stats, "u1").unwrap().reactions_received,
        0,
        "the repeated delete must not decrement again"
    );
}

/// Self-reactions count nowhere: the plain `put_reaction`/`delete_reaction`
/// path never touches the Response row's `reaction_count` or any stats row.
#[tokio::test]
async fn self_reactions_use_the_plain_path_and_count_nowhere() {
    let (_c, repo) = common::make_repo().await;
    let answer = published_answer("g-react-5", "202606", "q1", "u1");
    common::put_response(&repo, &answer).await;

    let reaction = common::reaction("g-react-5", "202606", "q1", "u1", "u1");
    engagement::put_reaction(&repo, &reaction).await.unwrap();

    let updated = persistence::responses::get_my_response(
        &repo,
        &GroupId::new("g-react-5"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.reaction_count, 0);

    let stats = persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-react-5"))
        .await
        .unwrap();
    assert!(
        stats.is_empty(),
        "a self-reaction must create no stats row at all"
    );
}

// ---------- §4 #9 — comment counters ----------

#[tokio::test]
async fn create_comment_bumps_the_authors_comments_written() {
    let (_c, repo) = common::make_repo().await;
    let new = engagement::NewComment {
        group_id: &GroupId::new("g-comment-1"),
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &domain::CommentId::new("cmt-01"),
        author_user_id: &UserId::new("u2"),
        body: "Nice answer!",
        image_media_id: None,
        created_at: Utc.with_ymd_and_hms(2026, 6, 6, 10, 0, 0).unwrap(),
    };
    engagement::create_comment(&repo, &new).await.unwrap();

    let stats =
        persistence::member_stats::list_stats_for_group(&repo, &GroupId::new("g-comment-1"))
            .await
            .unwrap();
    assert_eq!(stats_for(&stats, "u2").unwrap().comments_written, 1);
}

#[tokio::test]
async fn soft_delete_comment_reverses_the_bump_and_is_idempotent() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g-comment-2");
    let c = common::comment("g-comment-2", "202606", "q1", "u1", "u2", "cmt-02");
    engagement::put_comment(&repo, &c).await.unwrap();
    // Seed the counter the way `create_comment` would have.
    persistence::member_stats::list_stats_for_group(&repo, &group_id)
        .await
        .unwrap();

    let loc = CommentLocation {
        group_id: &group_id,
        cycle_id: &CycleId::new("202606"),
        question_id: &QuestionId::new("q1"),
        answer_user_id: &UserId::new("u1"),
        comment_id: &c.comment_id,
        created_at: c.created_at,
    };
    let at = Utc.with_ymd_and_hms(2026, 6, 7, 10, 0, 0).unwrap();
    engagement::soft_delete_comment(&repo, &loc, &c.author_user_id, at, None)
        .await
        .unwrap();

    let stats = persistence::member_stats::list_stats_for_group(&repo, &group_id)
        .await
        .unwrap();
    assert_eq!(stats_for(&stats, "u2").unwrap().comments_written, -1);

    // Repeating the delete must not decrement a second time.
    let err = engagement::soft_delete_comment(&repo, &loc, &c.author_user_id, at, None)
        .await
        .unwrap_err();
    assert!(
        err.is_lost_race(),
        "expected a cancelled transaction, got {err:?}"
    );
    let stats = persistence::member_stats::list_stats_for_group(&repo, &group_id)
        .await
        .unwrap();
    assert_eq!(stats_for(&stats, "u2").unwrap().comments_written, -1);
}
