mod common;

use common::utc;
use domain::{
    CycleId, GroupId, ImageId, PollOptionId, QuestionId, QuestionKind, ResponseStatus, UserId,
};
use persistence::newsletters;
use persistence::responses::{self, ResponseSave};
use pretty_assertions::assert_eq;

/// The member's `MemberStats.editions_answered`; 0 when they have no stats row.
async fn editions_answered(repo: &persistence::Repo, group_id: &GroupId, user_id: &UserId) -> i64 {
    persistence::member_stats::list_stats_for_group(repo, group_id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| &s.user_id == user_id)
        .map_or(0, |s| s.editions_answered)
}

fn open_newsletter(group_id: &str, cycle_id: &str) -> domain::Newsletter {
    let mut nl = common::voting_newsletter(group_id, cycle_id);
    nl.status = domain::NewsletterStatus::Open;
    nl.response_open_at = utc("2026-06-01T00:00:00Z");
    nl.response_close_at = utc("2026-06-05T00:00:00Z");
    nl.next_transition_at = Some(nl.response_close_at);
    nl
}

fn text_save<'a>(
    group_id: &'a GroupId,
    cycle_id: &'a CycleId,
    question_id: &'a QuestionId,
    user_id: &'a UserId,
    body: &'a str,
    images: &'a [ImageId],
    publish: bool,
) -> ResponseSave<'a> {
    ResponseSave {
        group_id,
        cycle_id,
        question_id,
        user_id,
        kind: QuestionKind::Text,
        body: Some(body),
        poll_option_id: None,
        image_media_ids: images,
        publish,
    }
}

#[tokio::test]
async fn it_saves_and_reads_a_draft_response() {
    let (_c, repo) = common::make_repo().await;
    let r = common::draft_response("g1", "202606", "q1", "u1");
    common::put_response(&repo, &r).await;

    let got = responses::get_my_response(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got, r);
}

#[tokio::test]
async fn it_returns_none_for_missing_response() {
    let (_c, repo) = common::make_repo().await;
    let result = responses::get_my_response(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_lists_all_answers_to_a_question() {
    let (_c, repo) = common::make_repo().await;
    let r1 = common::draft_response("g1", "202606", "q1", "u1");
    let r2 = {
        let mut r = common::draft_response("g1", "202606", "q1", "u2");
        r.response_id = domain::ResponseId::new("resp-02");
        r
    };
    common::put_response(&repo, &r1).await;
    common::put_response(&repo, &r2).await;

    let mut answers = responses::list_answers(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
    )
    .await
    .unwrap();
    answers.sort_by(|a, b| a.user_id.as_str().cmp(b.user_id.as_str()));
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0].user_id, r1.user_id);
    assert_eq!(answers[1].user_id, r2.user_id);
}

#[tokio::test]
async fn it_lists_my_responses_in_cycle_via_gsi1() {
    let (_c, repo) = common::make_repo().await;
    let r1 = common::draft_response("g1", "202606", "q1", "u1");
    let r2 = {
        let mut r = common::draft_response("g1", "202606", "q2", "u1");
        r.response_id = domain::ResponseId::new("resp-02");
        r
    };
    let other = {
        let mut r = common::draft_response("g1", "202606", "q1", "u2");
        r.response_id = domain::ResponseId::new("resp-03");
        r
    };
    // Same user, same calendar-month cycle id, different group: GSI1's key
    // has no group, so this must be filtered out, not listed.
    let other_group = {
        let mut r = common::draft_response("g2", "202606", "q9", "u1");
        r.response_id = domain::ResponseId::new("resp-04");
        r
    };
    common::put_response(&repo, &r1).await;
    common::put_response(&repo, &r2).await;
    common::put_response(&repo, &other).await;
    common::put_response(&repo, &other_group).await;

    let my = responses::list_my_responses_in_cycle(
        &repo,
        &UserId::new("u1"),
        &GroupId::new("g1"),
        &CycleId::new("202606"),
    )
    .await
    .unwrap();
    assert_eq!(my.len(), 2);
    assert!(my
        .iter()
        .all(|r| r.user_id.as_str() == "u1" && r.group_id.as_str() == "g1"));
}

#[tokio::test]
async fn save_response_creates_a_draft_and_keeps_the_response_id_stable_across_saves() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g1", "202606"))
        .await
        .unwrap();

    let group_id = GroupId::new("g1");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let now = utc("2026-06-02T00:00:00Z");

    let save = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "first draft",
        &[],
        false,
    );
    responses::save_response(&repo, &save, now).await.unwrap();

    let first = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.status, ResponseStatus::Draft);
    assert_eq!(first.body.as_deref(), Some("first draft"));

    let later = utc("2026-06-02T01:00:00Z");
    let save2 = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "second draft",
        &[],
        false,
    );
    responses::save_response(&repo, &save2, later)
        .await
        .unwrap();

    let second = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.body.as_deref(), Some("second draft"));
    assert_eq!(
        second.response_id, first.response_id,
        "responseId must stay stable across saves"
    );
}

#[tokio::test]
async fn save_response_with_publish_sets_published_at_but_does_not_bump_editions_answered() {
    // `editions_answered` is decided once at cycle close
    // (`docs/design/lifecycle.md` §5.3), not on a manual publish through this
    // transaction.
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g2", "202606"))
        .await
        .unwrap();
    let m = common::membership("u1", "g2", domain::Role::Member);
    common::put_membership(&repo, &m).await;

    let group_id = GroupId::new("g2");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let now = utc("2026-06-02T00:00:00Z");

    let save = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "my answer",
        &[],
        true,
    );
    responses::save_response(&repo, &save, now).await.unwrap();

    let published = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.status, ResponseStatus::Published);
    assert_eq!(published.published_at, Some(now));

    assert_eq!(editions_answered(&repo, &group_id, &user_id).await, 0);
}

#[tokio::test]
async fn publishing_after_a_draft_keeps_the_same_publish_call_semantics() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g3", "202606"))
        .await
        .unwrap();

    let group_id = GroupId::new("g3");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let draft_at = utc("2026-06-02T00:00:00Z");
    let publish_at = utc("2026-06-03T00:00:00Z");

    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "draft",
            &[],
            false,
        ),
        draft_at,
    )
    .await
    .unwrap();
    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "final",
            &[],
            true,
        ),
        publish_at,
    )
    .await
    .unwrap();

    let published = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.status, ResponseStatus::Published);
    assert_eq!(published.published_at, Some(publish_at));
    assert_eq!(published.body.as_deref(), Some("final"));
}

#[tokio::test]
async fn a_non_publishing_save_after_publish_keeps_the_response_published() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g4", "202606"))
        .await
        .unwrap();

    let group_id = GroupId::new("g4");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let publish_at = utc("2026-06-02T00:00:00Z");
    let edit_at = utc("2026-06-02T01:00:00Z");

    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "published answer",
            &[],
            true,
        ),
        publish_at,
    )
    .await
    .unwrap();

    // A later autosave with publish=false must not unpublish the response.
    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "edited after publish",
            &[],
            false,
        ),
        edit_at,
    )
    .await
    .unwrap();

    let still_published =
        responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(still_published.status, ResponseStatus::Published);
    assert_eq!(still_published.published_at, Some(publish_at));
    assert_eq!(
        still_published.body.as_deref(),
        Some("edited after publish")
    );
}

#[tokio::test]
async fn save_response_is_a_lost_race_when_the_cycle_is_not_open() {
    let (_c, repo) = common::make_repo().await;
    let mut published = open_newsletter("g6", "202606");
    published.status = domain::NewsletterStatus::Published;
    newsletters::write_status_transition(&repo, &published)
        .await
        .unwrap();

    let group_id = GroupId::new("g6");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let now = utc("2026-06-02T00:00:00Z");

    let err = responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "too late",
            &[],
            false,
        ),
        now,
    )
    .await
    .expect_err("cycle is not open");
    assert!(err.is_lost_race());
}

#[tokio::test]
async fn save_response_is_a_lost_race_once_the_deadline_has_passed() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g7", "202606"))
        .await
        .unwrap();

    let group_id = GroupId::new("g7");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    // open_newsletter's responseCloseAt is 2026-06-05T00:00:00Z.
    let past_deadline = utc("2026-06-06T00:00:00Z");

    let err = responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &question_id,
            &user_id,
            "past deadline",
            &[],
            false,
        ),
        past_deadline,
    )
    .await
    .expect_err("deadline has passed");
    assert!(err.is_lost_race());
}

#[tokio::test]
async fn save_response_stores_a_poll_vote() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g8", "202606"))
        .await
        .unwrap();

    let group_id = GroupId::new("g8");
    let cycle_id = CycleId::new("202606");
    let question_id = QuestionId::new("q1");
    let user_id = UserId::new("u1");
    let option_id = PollOptionId::new("opt-1");
    let now = utc("2026-06-02T00:00:00Z");

    let save = ResponseSave {
        group_id: &group_id,
        cycle_id: &cycle_id,
        question_id: &question_id,
        user_id: &user_id,
        kind: QuestionKind::Poll,
        body: None,
        poll_option_id: Some(&option_id),
        image_media_ids: &[],
        publish: true,
    };
    responses::save_response(&repo, &save, now).await.unwrap();

    let saved = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.kind, QuestionKind::Poll);
    assert_eq!(saved.poll_option_id, Some(option_id));
    assert_eq!(saved.body, None);
    assert_eq!(saved.status, ResponseStatus::Published);
}

/// A Query page stops at 1 MB; 60 answers at the 20,000-char body cap are
/// ~1.2 MB, so this only passes if `list_answers` follows `LastEvaluatedKey`.
#[tokio::test]
async fn list_answers_reads_past_the_one_megabyte_query_page() {
    let (_c, repo) = common::make_repo().await;
    let body = "x".repeat(shared::config::MAX_RESPONSE_BODY_CHARS);
    for n in 0..60 {
        let mut r = common::draft_response("g-page", "202606", "q1", &format!("u{n:02}"));
        r.response_id = domain::ResponseId::new(format!("resp-{n:02}"));
        r.body = Some(body.clone());
        common::put_response(&repo, &r).await;
    }

    let answers = responses::list_answers(
        &repo,
        &GroupId::new("g-page"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
    )
    .await
    .unwrap();
    assert_eq!(answers.len(), 60);
}

/// `editions_answered` lives on `MemberStats` (`docs/design/data-model.md`
/// §2.17), so leaving and rejoining keeps it, like every other stat. The bump
/// itself is `lambda-cycle-tick`'s close-time sweep's job
/// (`docs/design/lifecycle.md` §5.3), exercised directly here.
#[tokio::test]
async fn editions_answered_survives_a_leave_and_rejoin() {
    let (_c, repo) = common::make_repo().await;
    let mut g = common::group("g-ed-rejoin", "u0");
    g.member_count = 2;
    persistence::groups::put_group(&repo, &g).await.unwrap();
    let m = common::membership("u1", "g-ed-rejoin", domain::Role::Member);
    common::put_membership(&repo, &m).await;

    let group_id = GroupId::new("g-ed-rejoin");
    let cycle_id = CycleId::new("202606");
    let user_id = UserId::new("u1");

    persistence::member_stats::bump_editions_answered(&repo, &group_id, &user_id, &cycle_id)
        .await
        .unwrap();

    persistence::groups::leave_group_tx(&repo, &user_id, &group_id, None)
        .await
        .unwrap();
    common::put_membership(&repo, &m).await;

    assert_eq!(editions_answered(&repo, &group_id, &user_id).await, 1);
}

/// A retried close-time bump for the same cycle must not double-count
/// (`docs/design/lifecycle.md` §5.3's `last_answered_cycle` marker).
#[tokio::test]
async fn bump_editions_answered_counts_the_same_cycle_once() {
    let (_c, repo) = common::make_repo().await;
    let group_id = GroupId::new("g-bump-once");
    let cycle_id = CycleId::new("202606");
    let user_id = UserId::new("u1");

    persistence::member_stats::bump_editions_answered(&repo, &group_id, &user_id, &cycle_id)
        .await
        .unwrap();
    let err =
        persistence::member_stats::bump_editions_answered(&repo, &group_id, &user_id, &cycle_id)
            .await
            .expect_err("already counted");
    assert!(err.is_lost_race());

    assert_eq!(editions_answered(&repo, &group_id, &user_id).await, 1);
}
