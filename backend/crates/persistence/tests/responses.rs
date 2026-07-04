mod common;

use domain::{CycleId, GroupId, QuestionId, ResponseStatus, UserId};
use persistence::responses;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_saves_and_reads_a_draft_response() {
    let (_c, repo) = common::make_repo().await;
    let r = common::draft_response("g1", "202606", "q1", "u1");
    responses::save_draft(&repo, &r).await.unwrap();

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
    responses::save_draft(&repo, &r1).await.unwrap();
    responses::save_draft(&repo, &r2).await.unwrap();

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
    responses::save_draft(&repo, &r1).await.unwrap();
    responses::save_draft(&repo, &r2).await.unwrap();
    responses::save_draft(&repo, &other).await.unwrap();

    let my =
        responses::list_my_responses_in_cycle(&repo, &UserId::new("u1"), &CycleId::new("202606"))
            .await
            .unwrap();
    assert_eq!(my.len(), 2);
    assert!(my.iter().all(|r| r.user_id.as_str() == "u1"));
}

#[tokio::test]
async fn publish_response_tx_flips_status_to_published() {
    let (_c, repo) = common::make_repo().await;
    use aws_sdk_dynamodb::types::AttributeValue;
    use domain::Role;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
    use serde_dynamo::to_item;

    let m = common::membership("u1", "g1", Role::Admin);
    let mut mem_item: std::collections::HashMap<String, AttributeValue> = to_item(&m).unwrap();
    mem_item.insert(attr::PK.into(), AttributeValue::S(user_pk(&m.user_id)));
    mem_item.insert(
        attr::SK.into(),
        AttributeValue::S(membership_sk(&m.group_id)),
    );
    mem_item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(membership_gsi1pk(&m.group_id)),
    );
    mem_item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(membership_gsi1sk(&m.user_id)),
    );
    mem_item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("GroupMembership".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(mem_item))
        .send()
        .await
        .unwrap();

    let r = common::draft_response("g1", "202606", "q1", "u1");
    responses::save_draft(&repo, &r).await.unwrap();

    responses::publish_response_tx(&repo, &r, "2026-06-04T12:00:00Z", true)
        .await
        .unwrap();

    let published = responses::get_my_response(
        &repo,
        &GroupId::new("g1"),
        &CycleId::new("202606"),
        &QuestionId::new("q1"),
        &UserId::new("u1"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(published.status, ResponseStatus::Published);
    assert!(published.published_at.is_some());
}

#[tokio::test]
async fn publish_response_tx_bumps_editions_answered_on_first_publish() {
    let (_c, repo) = common::make_repo().await;
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, membership_gsi1pk, membership_gsi1sk, membership_sk, user_pk};
    use serde_dynamo::to_item;

    let m = common::membership("u1", "g1", domain::Role::Member);
    let mut mem_item: std::collections::HashMap<String, AttributeValue> = to_item(&m).unwrap();
    mem_item.insert(attr::PK.into(), AttributeValue::S(user_pk(&m.user_id)));
    mem_item.insert(
        attr::SK.into(),
        AttributeValue::S(membership_sk(&m.group_id)),
    );
    mem_item.insert(
        attr::GSI1PK.into(),
        AttributeValue::S(membership_gsi1pk(&m.group_id)),
    );
    mem_item.insert(
        attr::GSI1SK.into(),
        AttributeValue::S(membership_gsi1sk(&m.user_id)),
    );
    mem_item.insert(
        attr::ENTITY.into(),
        AttributeValue::S("GroupMembership".into()),
    );
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(mem_item))
        .send()
        .await
        .unwrap();

    let r = common::draft_response("g1", "202606", "q1", "u1");
    responses::save_draft(&repo, &r).await.unwrap();
    responses::publish_response_tx(&repo, &r, "2026-06-04T12:00:00Z", true)
        .await
        .unwrap();

    let updated_m =
        persistence::groups::get_membership(&repo, &UserId::new("u1"), &GroupId::new("g1"))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(updated_m.editions_answered, 1);
}
