mod common;

use domain::{CognitoSub, UserId};
use persistence::users;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_returns_none_for_missing_user() {
    let (_c, repo) = common::make_repo().await;
    let result = users::get_user(&repo, &UserId::new("no-such-user"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_puts_and_gets_user() {
    let (_c, repo) = common::make_repo().await;
    let u = common::user("u1");
    users::put_user(&repo, &u).await.unwrap();
    let got = users::get_user(&repo, &u.user_id).await.unwrap().unwrap();
    assert_eq!(got, u);
}

#[tokio::test]
async fn it_resolves_cognito_sub_to_user_id() {
    let (_c, repo) = common::make_repo().await;
    let u = common::user("u2");

    // Write user + cognito-sub lookup row manually (as the join transaction would).
    users::put_user(&repo, &u).await.unwrap();

    // Put the cognito-sub lookup row directly via serde_dynamo.
    use aws_sdk_dynamodb::types::AttributeValue;
    use persistence::keys::{attr, cognito_sub_pk, COGNITO_SUB_SK};
    let mut item = std::collections::HashMap::new();
    item.insert(
        attr::PK.into(),
        AttributeValue::S(cognito_sub_pk(&u.cognito_sub)),
    );
    item.insert(attr::SK.into(), AttributeValue::S(COGNITO_SUB_SK.into()));
    item.insert("user_id".into(), AttributeValue::S(u.user_id.to_string()));
    item.insert(
        "cognito_sub".into(),
        AttributeValue::S(u.cognito_sub.to_string()),
    );
    item.insert(attr::ENTITY.into(), AttributeValue::S("CognitoSubLookup".into()));
    repo.client
        .put_item()
        .table_name(&repo.table)
        .set_item(Some(item))
        .send()
        .await
        .unwrap();

    let resolved = users::resolve_cognito_sub(&repo, &u.cognito_sub)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved, u.user_id);
}

#[tokio::test]
async fn it_returns_none_for_unregistered_cognito_sub() {
    let (_c, repo) = common::make_repo().await;
    let result = users::resolve_cognito_sub(&repo, &CognitoSub::new("unknown-sub"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_updates_last_login() {
    let (_c, repo) = common::make_repo().await;
    let u = common::user("u3");
    users::put_user(&repo, &u).await.unwrap();

    let new_login = "2026-06-15T10:00:00Z";
    users::update_last_login(&repo, &u.user_id, new_login)
        .await
        .unwrap();

    let got = users::get_user(&repo, &u.user_id).await.unwrap().unwrap();
    assert_eq!(
        got.last_login_at,
        chrono::DateTime::parse_from_rfc3339(new_login)
            .unwrap()
            .with_timezone(&chrono::Utc)
    );
}
