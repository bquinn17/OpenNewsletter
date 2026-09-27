//! Handler-level tests for `lambda-newsletters` (`plans/03-api-contract.md` §5).

mod common;

use chrono::{TimeZone, Utc};
use domain::api::NewsletterDetailResponse;
use domain::{
    ApiErrorCode, Comment, CommentId, CycleId, GroupId, LockedQuestion, PollOption, PollOptionId,
    QuestionId, QuestionKind, Reaction, Response, ResponseId, ResponseStatus, Role, UserId,
};
use newsletters::handlers;
use newsletters::state::AppState;
use persistence::{engagement, newsletters as nl_repo, questions, responses};
use pretty_assertions::assert_eq;

fn state(repo: persistence::Repo) -> AppState {
    AppState {
        repo,
        cdn_base_url: String::new(),
    }
}

#[tokio::test]
async fn non_member_is_forbidden_on_list_and_detail() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gnm", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    let voting = common::voting_newsletter("gnm", "202606");
    nl_repo::write_status_transition(&repo, &voting)
        .await
        .unwrap();

    let state = state(repo);
    let outsider = UserId::new("intruder");
    let group_id = GroupId::new("gnm");

    let list_err = handlers::list_newsletters(&state, &outsider, &group_id, None, 20, None)
        .await
        .expect_err("non-member must be refused");
    assert_eq!(list_err.code, ApiErrorCode::Forbidden);

    let detail_err =
        handlers::get_newsletter_detail(&state, &outsider, &group_id, &CycleId::new("202606"))
            .await
            .expect_err("non-member must be refused");
    assert_eq!(detail_err.code, ApiErrorCode::Forbidden);
}

#[tokio::test]
async fn get_newsletter_detail_404s_for_a_missing_cycle() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("g404", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;

    let state = state(repo);
    let err = handlers::get_newsletter_detail(
        &state,
        &UserId::new("u1"),
        &GroupId::new("g404"),
        &CycleId::new("202699"),
    )
    .await
    .expect_err("cycle does not exist");
    assert_eq!(err.code, ApiErrorCode::NotFound);
}

#[tokio::test]
async fn list_newsletters_reports_my_draft_and_published_counts() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gct", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;

    let mut open = common::voting_newsletter("gct", "202606");
    open.status = domain::NewsletterStatus::Open;
    open.locked_question_ids = vec![QuestionId::new("q1"), QuestionId::new("q2")];
    nl_repo::write_status_transition(&repo, &open)
        .await
        .unwrap();

    responses::save_draft(
        &repo,
        &Response {
            response_id: ResponseId::new("r1"),
            user_id: UserId::new("u1"),
            question_id: QuestionId::new("q1"),
            group_id: GroupId::new("gct"),
            cycle_id: CycleId::new("202606"),
            kind: QuestionKind::Text,
            status: ResponseStatus::Draft,
            body: Some("draft answer".into()),
            poll_option_id: None,
            image_media_ids: Vec::new(),
            updated_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
            published_at: None,
        },
    )
    .await
    .unwrap();

    let state = state(repo);
    let response = handlers::list_newsletters(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gct"),
        None,
        20,
        None,
    )
    .await
    .unwrap();

    let item = response
        .items
        .iter()
        .find(|i| i.cycle_id == CycleId::new("202606"))
        .expect("the open cycle is in the list");
    assert_eq!(item.question_count, 2);
    assert_eq!(item.my_draft_count, 1);
    assert_eq!(item.my_published_count, 0);
}

#[tokio::test]
async fn voting_detail_hydrates_the_candidate_pool() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gvd", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    let voting = common::voting_newsletter("gvd", "202606");
    nl_repo::write_status_transition(&repo, &voting)
        .await
        .unwrap();
    persistence::users::put_user(&repo, &common::user("u1"))
        .await
        .unwrap();
    questions::put_candidate(&repo, &common::candidate("gvd", "202606", "q1", "u1", 5))
        .await
        .unwrap();

    let state = state(repo);
    let detail = handlers::get_newsletter_detail(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gvd"),
        &CycleId::new("202606"),
    )
    .await
    .unwrap();

    match detail {
        NewsletterDetailResponse::Voting { candidates } => {
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].vote_count, 5);
            assert_eq!(
                candidates[0].asked_by.as_ref().unwrap().user_id,
                UserId::new("u1")
            );
        }
        other => panic!("expected Voting, got {other:?}"),
    }
}

#[tokio::test]
async fn open_detail_shows_only_the_callers_own_draft() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("god", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    for id in ["u1", "u2"] {
        persistence::users::put_user(&repo, &common::user(id))
            .await
            .unwrap();
    }

    let mut open = common::voting_newsletter("god", "202606");
    open.status = domain::NewsletterStatus::Open;
    open.locked_question_ids = vec![QuestionId::new("q1")];
    nl_repo::write_status_transition(&repo, &open)
        .await
        .unwrap();
    let lq = LockedQuestion {
        question_id: QuestionId::new("q1"),
        group_id: GroupId::new("god"),
        cycle_id: CycleId::new("202606"),
        kind: QuestionKind::Text,
        prompt: "Best hike?".into(),
        poll_options: None,
        display_order: 0,
        submitted_by: UserId::new("u1"),
        is_anonymous: false,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    };
    put_locked_question_directly(&repo, &lq).await;

    responses::save_draft(
        &repo,
        &Response {
            response_id: ResponseId::new("r1"),
            user_id: UserId::new("u2"),
            question_id: QuestionId::new("q1"),
            group_id: GroupId::new("god"),
            cycle_id: CycleId::new("202606"),
            kind: QuestionKind::Text,
            status: ResponseStatus::Draft,
            body: Some("u2's secret draft".into()),
            poll_option_id: None,
            image_media_ids: Vec::new(),
            updated_at: Utc.with_ymd_and_hms(2026, 6, 2, 0, 0, 0).unwrap(),
            published_at: None,
        },
    )
    .await
    .unwrap();

    let state = state(repo);
    let detail_as_u1 = handlers::get_newsletter_detail(
        &state,
        &UserId::new("u1"),
        &GroupId::new("god"),
        &CycleId::new("202606"),
    )
    .await
    .unwrap();

    match detail_as_u1 {
        NewsletterDetailResponse::Open { questions, .. } => {
            assert_eq!(questions.len(), 1);
            assert!(
                questions[0].my_response.is_none(),
                "u1 has no draft; must not see u2's"
            );
        }
        other => panic!("expected Open, got {other:?}"),
    }
}

#[tokio::test]
async fn published_detail_hydrates_answers_comments_and_reactions() {
    let (_c, repo) = common::make_repo().await;
    let group = common::group("gpd", "u1");
    common::seed_group_and_membership(&repo, &group, "u1", Role::Admin).await;
    common::seed_group_and_membership(&repo, &group, "u2", Role::Member).await;
    for id in ["u1", "u2"] {
        persistence::users::put_user(&repo, &common::user(id))
            .await
            .unwrap();
    }

    let mut published = common::voting_newsletter("gpd", "202606");
    published.status = domain::NewsletterStatus::Published;
    published.published_at = Some(Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap());
    published.locked_question_ids = vec![QuestionId::new("q1"), QuestionId::new("q2")];
    nl_repo::write_status_transition(&repo, &published)
        .await
        .unwrap();

    let text_lq = LockedQuestion {
        question_id: QuestionId::new("q1"),
        group_id: GroupId::new("gpd"),
        cycle_id: CycleId::new("202606"),
        kind: QuestionKind::Text,
        prompt: "Best hike?".into(),
        poll_options: None,
        display_order: 0,
        submitted_by: UserId::new("u1"),
        is_anonymous: false,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    };
    let poll_lq = LockedQuestion {
        question_id: QuestionId::new("q2"),
        group_id: GroupId::new("gpd"),
        cycle_id: CycleId::new("202606"),
        kind: QuestionKind::Poll,
        prompt: "Best trailhead?".into(),
        poll_options: Some(vec![
            PollOption {
                option_id: PollOptionId::new("opt-north"),
                label: "North lot".into(),
            },
            PollOption {
                option_id: PollOptionId::new("opt-south"),
                label: "South lot".into(),
            },
        ]),
        display_order: 1,
        submitted_by: UserId::new("u2"),
        is_anonymous: true,
        locked_at: Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap(),
    };
    for lq in [&text_lq, &poll_lq] {
        put_locked_question_directly(&repo, lq).await;
    }

    responses::save_draft(
        &repo,
        &Response {
            response_id: ResponseId::new("r1"),
            user_id: UserId::new("u2"),
            question_id: QuestionId::new("q1"),
            group_id: GroupId::new("gpd"),
            cycle_id: CycleId::new("202606"),
            kind: QuestionKind::Text,
            status: ResponseStatus::Published,
            body: Some("Lake 22, hands down.".into()),
            poll_option_id: None,
            image_media_ids: Vec::new(),
            updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
            published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
        },
    )
    .await
    .unwrap();
    responses::save_draft(
        &repo,
        &Response {
            response_id: ResponseId::new("r2"),
            user_id: UserId::new("u1"),
            question_id: QuestionId::new("q2"),
            group_id: GroupId::new("gpd"),
            cycle_id: CycleId::new("202606"),
            kind: QuestionKind::Poll,
            status: ResponseStatus::Published,
            body: None,
            poll_option_id: Some(PollOptionId::new("opt-north")),
            image_media_ids: Vec::new(),
            updated_at: Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap(),
            published_at: Some(Utc.with_ymd_and_hms(2026, 6, 3, 0, 0, 0).unwrap()),
        },
    )
    .await
    .unwrap();

    engagement::put_comment(
        &repo,
        &Comment {
            comment_id: CommentId::new("c1"),
            group_id: GroupId::new("gpd"),
            cycle_id: CycleId::new("202606"),
            question_id: QuestionId::new("q1"),
            answer_user_id: UserId::new("u2"),
            author_user_id: UserId::new("u1"),
            body: "Nice answer!".into(),
            image_media_id: None,
            created_at: Utc.with_ymd_and_hms(2026, 6, 4, 0, 0, 0).unwrap(),
            edited_at: None,
            deleted_at: None,
        },
    )
    .await
    .unwrap();
    engagement::put_reaction(
        &repo,
        &Reaction {
            group_id: GroupId::new("gpd"),
            cycle_id: CycleId::new("202606"),
            question_id: QuestionId::new("q1"),
            answer_user_id: UserId::new("u2"),
            reactor_user_id: UserId::new("u1"),
            emoji: "🔥".into(),
            created_at: Utc.with_ymd_and_hms(2026, 6, 4, 0, 0, 0).unwrap(),
        },
    )
    .await
    .unwrap();

    let state = state(repo);
    let detail = handlers::get_newsletter_detail(
        &state,
        &UserId::new("u1"),
        &GroupId::new("gpd"),
        &CycleId::new("202606"),
    )
    .await
    .unwrap();

    match detail {
        NewsletterDetailResponse::Published { questions, .. } => {
            assert_eq!(questions.len(), 2);

            let text_q = &questions[0];
            let answers = text_q.answers.as_ref().expect("text question has answers");
            assert_eq!(answers.len(), 1);
            assert_eq!(answers[0].body.as_deref(), Some("Lake 22, hands down."));
            assert_eq!(answers[0].comments.len(), 1);
            assert_eq!(answers[0].reaction_groups.len(), 1);
            assert_eq!(answers[0].reaction_groups[0].emoji, "🔥");
            assert_eq!(answers[0].reaction_groups[0].count, 1);
            assert!(answers[0].reaction_groups[0].reacted_by_me);

            let poll_q = &questions[1];
            assert!(poll_q.is_anonymous);
            // The caller (u1) is a group admin, so anonymity is redacted even
            // though u1 isn't the submitter (u2) — admins always see attribution.
            assert!(poll_q.asked_by.is_some());
        }
        other => panic!("expected Published, got {other:?}"),
    }
}

async fn put_locked_question_directly(repo: &persistence::Repo, lq: &LockedQuestion) {
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, locked_pk, locked_sk};

    let mut item: std::collections::HashMap<String, AttributeValue> =
        serde_dynamo::to_item(lq).expect("locked question serializes");
    item.insert(
        attr::PK.into(),
        AttributeValue::S(locked_pk(&lq.group_id, &lq.cycle_id)),
    );
    item.insert(
        attr::SK.into(),
        AttributeValue::S(locked_sk(&lq.question_id)),
    );
    item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("LockedQuestion".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .expect("locked question written");
}
