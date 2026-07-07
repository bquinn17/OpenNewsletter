mod common;

use domain::{GroupId, UserId};
use persistence::push;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_subscribes_and_lists_push_subs() {
    let (_c, repo) = common::make_repo().await;
    let s1 = common::push_subscription("u1", "hash-aaa");
    let s2 = common::push_subscription("u1", "hash-bbb");
    let other = common::push_subscription("u2", "hash-ccc");
    push::put_subscription(&repo, &s1).await.unwrap();
    push::put_subscription(&repo, &s2).await.unwrap();
    push::put_subscription(&repo, &other).await.unwrap();

    let mut subs = push::list_subs_for_user(&repo, &UserId::new("u1"))
        .await
        .unwrap();
    subs.sort_by(|a, b| a.endpoint_hash.cmp(&b.endpoint_hash));
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0], s1);
    assert_eq!(subs[1], s2);
}

#[tokio::test]
async fn it_deletes_a_push_subscription() {
    let (_c, repo) = common::make_repo().await;
    let s = common::push_subscription("u1", "hash-aaa");
    push::put_subscription(&repo, &s).await.unwrap();

    push::delete_subscription(&repo, &UserId::new("u1"), "hash-aaa")
        .await
        .unwrap();

    let subs = push::list_subs_for_user(&repo, &UserId::new("u1"))
        .await
        .unwrap();
    assert_eq!(subs.len(), 0);
}

#[tokio::test]
async fn it_puts_and_gets_notification_pref() {
    let (_c, repo) = common::make_repo().await;
    let p = common::notification_pref("u1", "g1");
    push::put_pref(&repo, &p).await.unwrap();
    let got = push::get_pref(&repo, &UserId::new("u1"), &GroupId::new("g1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, p);
}

#[tokio::test]
async fn it_returns_none_for_missing_notification_pref() {
    let (_c, repo) = common::make_repo().await;
    let result = push::get_pref(&repo, &UserId::new("u1"), &GroupId::new("g1"))
        .await
        .unwrap();
    assert_eq!(result, None);
}
