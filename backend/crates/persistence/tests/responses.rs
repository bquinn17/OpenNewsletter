mod common;

use chrono::{TimeZone, Utc};
use domain::{
    CycleId, GroupId, ImageId, PollOptionId, QuestionId, QuestionKind, ResponseStatus, UserId,
};
use persistence::responses::{self, ResponseSave};
use persistence::{groups, newsletters};
use pretty_assertions::assert_eq;

fn open_newsletter(group_id: &str, cycle_id: &str) -> domain::Newsletter {
    let mut nl = common::voting_newsletter(group_id, cycle_id);
    nl.status = domain::NewsletterStatus::Open;
    nl.response_open_at = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
    nl.response_close_at = Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap();
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
    let now = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();

    let save = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "first draft",
        &[],
        false,
    );
    responses::save_response(&repo, &save, now, false)
        .await
        .unwrap();

    let first = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.status, ResponseStatus::Draft);
    assert_eq!(first.body.as_deref(), Some("first draft"));

    let later = Utc.with_ymd_and_hms(2026, 6, 2, 1, 0, 0).unwrap();
    let save2 = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "second draft",
        &[],
        false,
    );
    responses::save_response(&repo, &save2, later, false)
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
async fn save_response_with_publish_sets_published_at_and_bumps_editions_answered() {
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
    let now = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();

    let save = text_save(
        &group_id,
        &cycle_id,
        &question_id,
        &user_id,
        "my answer",
        &[],
        true,
    );
    responses::save_response(&repo, &save, now, true)
        .await
        .unwrap();

    let published = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(published.status, ResponseStatus::Published);
    assert_eq!(published.published_at, Some(now));

    let updated_membership = groups::get_membership(&repo, &user_id, &group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_membership.editions_answered, 1);
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
    let draft_at = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();
    let publish_at = Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap();

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
        false,
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
        false,
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
    let publish_at = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();
    let edit_at = Utc.with_ymd_and_hms(2026, 6, 2, 1, 0, 0).unwrap();

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
        false,
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
        false,
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
async fn a_second_publish_in_the_same_cycle_does_not_bump_editions_answered_again() {
    let (_c, repo) = common::make_repo().await;
    newsletters::write_status_transition(&repo, &open_newsletter("g5", "202606"))
        .await
        .unwrap();
    let m = common::membership("u1", "g5", domain::Role::Member);
    common::put_membership(&repo, &m).await;

    let group_id = GroupId::new("g5");
    let cycle_id = CycleId::new("202606");
    let user_id = UserId::new("u1");
    let now = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();

    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &QuestionId::new("q1"),
            &user_id,
            "answer one",
            &[],
            true,
        ),
        now,
        true,
    )
    .await
    .unwrap();
    responses::save_response(
        &repo,
        &text_save(
            &group_id,
            &cycle_id,
            &QuestionId::new("q2"),
            &user_id,
            "answer two",
            &[],
            true,
        ),
        now,
        false,
    )
    .await
    .unwrap();

    let updated_membership = groups::get_membership(&repo, &user_id, &group_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated_membership.editions_answered, 1);
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
    let now = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();

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
        false,
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
    let past_deadline = Utc.with_ymd_and_hms(2026, 6, 6, 0, 0, 0).unwrap();

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
        false,
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
    let now = Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap();

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
    responses::save_response(&repo, &save, now, false)
        .await
        .unwrap();

    let saved = responses::get_my_response(&repo, &group_id, &cycle_id, &question_id, &user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.kind, QuestionKind::Poll);
    assert_eq!(saved.poll_option_id, Some(option_id));
    assert_eq!(saved.body, None);
    assert_eq!(saved.status, ResponseStatus::Published);
}
