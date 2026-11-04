//! `lambda-cycle-tick`'s first-cycle pass (`docs/design/lifecycle.md` §4.2):
//! the bootstrap/seed scripts leave a newly created group's META row marked
//! "awaiting first cycle" instead of pre-seeding a Newsletter row, and the
//! tick creates that group's first `voting` cycle, monthly or weekly.

mod common;

use aws_sdk_dynamodb::types::AttributeValue;
use cycle_tick::tick::run_tick;
use domain::{GroupId, NewsletterStatus};
use persistence::keys::{
    attr, group_awaiting_first_cycle_gsi2sk, group_pk, GROUP_AWAITING_FIRST_CYCLE_GSI2PK,
};
use persistence::{groups, newsletters, Repo};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

/// Writes a group the way the bootstrap/seed scripts do: a plain `Group`
/// META row plus the sparse GSI2 "awaiting first cycle" marker, and no
/// Newsletter row at all.
async fn seed_marked_group(repo: &Repo, group_id: &str) -> domain::Group {
    let group = common::group(group_id, "u1");
    groups::put_group(repo, &group).await.unwrap();
    repo.client
        .update_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(&group.group_id)))
        .key(
            attr::SK,
            AttributeValue::S(persistence::keys::GROUP_META_SK.into()),
        )
        .update_expression("SET #g2pk = :g2pk, #g2sk = :g2sk")
        .expression_attribute_names("#g2pk", attr::GSI2PK)
        .expression_attribute_names("#g2sk", attr::GSI2SK)
        .expression_attribute_values(
            ":g2pk",
            AttributeValue::S(GROUP_AWAITING_FIRST_CYCLE_GSI2PK.into()),
        )
        .expression_attribute_values(
            ":g2sk",
            AttributeValue::S(group_awaiting_first_cycle_gsi2sk(
                group.created_at,
                &group.group_id,
            )),
        )
        .send()
        .await
        .unwrap();
    group
}

async fn has_marker(repo: &Repo, group_id: &GroupId) -> bool {
    let item = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(group_id)))
        .key(
            attr::SK,
            AttributeValue::S(persistence::keys::GROUP_META_SK.into()),
        )
        .send()
        .await
        .unwrap()
        .item
        .unwrap();
    item.contains_key(attr::GSI2PK)
}

#[tokio::test]
async fn a_tick_creates_a_groups_first_monthly_voting_cycle_and_clears_the_marker() {
    let (_c, repo) = common::make_test_repo().await;
    let group = seed_marked_group(&repo, "g-first-monthly").await;

    run_tick(&repo, CycleCadence::Monthly).await;

    let tz: chrono_tz::Tz = group.timezone.parse().unwrap();
    let expected = shared::cycle_time::next_cycle_schedule(
        CycleCadence::Monthly,
        chrono::Utc::now(),
        tz,
        group.cycle_settings.response_window_days,
    );
    let voting = newsletters::find_voting_cycle(&repo, &group.group_id)
        .await
        .unwrap()
        .expect("the tick creates the first voting cycle");
    assert_eq!(voting.cycle_id, expected.cycle_id);
    assert_eq!(voting.status, NewsletterStatus::Voting);
    assert_eq!(voting.response_open_at, expected.response_open_at);
    assert_eq!(voting.response_close_at, expected.response_close_at);
    assert!(!has_marker(&repo, &group.group_id).await);
}

#[tokio::test]
async fn a_tick_creates_a_groups_first_weekly_voting_cycle() {
    let (_c, repo) = common::make_test_repo().await;
    let group = seed_marked_group(&repo, "g-first-weekly").await;
    let cadence = CycleCadence::Weekly {
        weekday: chrono::Weekday::Sun,
        at: chrono::NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
    };

    run_tick(&repo, cadence).await;

    let voting = newsletters::find_voting_cycle(&repo, &group.group_id)
        .await
        .unwrap()
        .expect("the tick creates the first voting cycle");
    assert_eq!(voting.cycle_id.as_str().len(), 8, "weekly ids are yyyymmdd");
    assert!(!has_marker(&repo, &group.group_id).await);
}

#[tokio::test]
async fn a_second_tick_creates_nothing_once_the_first_cycle_exists() {
    let (_c, repo) = common::make_test_repo().await;
    let group = seed_marked_group(&repo, "g-first-idempotent").await;

    run_tick(&repo, CycleCadence::Monthly).await;
    let first = newsletters::find_voting_cycle(&repo, &group.group_id)
        .await
        .unwrap()
        .expect("created by the first tick");

    run_tick(&repo, CycleCadence::Monthly).await;
    let still = newsletters::find_voting_cycle(&repo, &group.group_id)
        .await
        .unwrap()
        .expect("unchanged by the second tick");
    assert_eq!(still, first);
}

/// A group that already has a `voting` cycle (e.g. a bootstrap retry that
/// wrote the marker but the cycle was created some other way) just has its
/// marker cleared — no second cycle is created.
#[tokio::test]
async fn a_marked_group_that_already_has_a_voting_cycle_just_clears_the_marker() {
    let (_c, repo) = common::make_test_repo().await;
    let group = seed_marked_group(&repo, "g-first-already-voting").await;
    let existing = common::group("g-first-already-voting", "u1");
    let tz: chrono_tz::Tz = existing.timezone.parse().unwrap();
    let schedule = shared::cycle_time::next_cycle_schedule(
        CycleCadence::Monthly,
        chrono::Utc::now(),
        tz,
        existing.cycle_settings.response_window_days,
    );
    let pre_existing = newsletters::create_next_voting_cycle(&repo, &group.group_id, schedule)
        .await
        .unwrap();

    run_tick(&repo, CycleCadence::Monthly).await;

    let voting = newsletters::find_voting_cycle(&repo, &group.group_id)
        .await
        .unwrap()
        .expect("the pre-existing voting cycle is untouched");
    assert_eq!(voting, pre_existing);
    assert!(!has_marker(&repo, &group.group_id).await);
}
