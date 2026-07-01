mod common;

use chrono::{TimeZone, Utc};
use domain::{CycleId, GroupId, NewsletterStatus};
use persistence::newsletters;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn it_writes_and_reads_newsletter() {
    let (_c, repo) = common::make_repo().await;
    let nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl).await.unwrap();
    let got = newsletters::get_newsletter(&repo, &nl.group_id, &nl.cycle_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, nl);
}

#[tokio::test]
async fn it_returns_none_for_missing_newsletter() {
    let (_c, repo) = common::make_repo().await;
    let result = newsletters::get_newsletter(&repo, &GroupId::new("g1"), &CycleId::new("202606"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn it_lists_newsletters_for_group_newest_first() {
    let (_c, repo) = common::make_repo().await;
    let nl1 = common::voting_newsletter("g1", "202604");
    let nl2 = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl1).await.unwrap();
    newsletters::write_status_transition(&repo, &nl2).await.unwrap();

    let list = newsletters::list_newsletters_for_group(&repo, &GroupId::new("g1"))
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    // DDB scan_index_forward(false) returns items in descending sk order.
    assert_eq!(list[0].cycle_id, nl2.cycle_id);
    assert_eq!(list[1].cycle_id, nl1.cycle_id);
}

#[tokio::test]
async fn it_finds_voting_cycles_due_via_gsi2() {
    let (_c, repo) = common::make_repo().await;

    // nl_due has next_transition_at in the past.
    let mut nl_due = common::voting_newsletter("g1", "202605");
    nl_due.next_transition_at = Some(Utc.with_ymd_and_hms(2026, 5, 31, 23, 59, 0).unwrap());
    newsletters::write_status_transition(&repo, &nl_due).await.unwrap();

    // nl_future has next_transition_at in the future.
    let mut nl_future = common::voting_newsletter("g1", "202606");
    nl_future.next_transition_at = Some(Utc.with_ymd_and_hms(2026, 12, 1, 0, 0, 0).unwrap());
    newsletters::write_status_transition(&repo, &nl_future).await.unwrap();

    let due = newsletters::list_cycles_due(
        &repo,
        NewsletterStatus::Voting,
        "2026-06-01T00:00:00+00:00",
    )
    .await
    .unwrap();

    assert_eq!(due.len(), 1);
    assert_eq!(due[0].cycle_id, nl_due.cycle_id);
}

#[tokio::test]
async fn write_status_transition_updates_gsi2_keys_on_status_change() {
    let (_c, repo) = common::make_repo().await;
    let nl = common::voting_newsletter("g1", "202606");
    newsletters::write_status_transition(&repo, &nl).await.unwrap();

    // Transition to Open status.
    let mut nl_open = nl.clone();
    nl_open.status = NewsletterStatus::Open;
    nl_open.next_transition_at = Some(Utc.with_ymd_and_hms(2026, 6, 5, 0, 0, 0).unwrap());
    newsletters::write_status_transition(&repo, &nl_open).await.unwrap();

    // Old status (Voting) must return empty.
    let still_voting = newsletters::list_cycles_due(
        &repo,
        NewsletterStatus::Voting,
        "2026-12-31T00:00:00+00:00",
    )
    .await
    .unwrap();
    assert_eq!(still_voting.len(), 0);

    // New status (Open) must be found.
    let open_due = newsletters::list_cycles_due(
        &repo,
        NewsletterStatus::Open,
        "2026-12-31T00:00:00+00:00",
    )
    .await
    .unwrap();
    assert_eq!(open_due.len(), 1);
    assert_eq!(open_due[0].status, NewsletterStatus::Open);
}
