//! `PATCH /groups/{g}` applies `responseWindowDays` to the current cycles
//! (`docs/design/lifecycle.md` §6, `docs/design/api.md` §4.3).
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`, and tests run concurrently.

mod common;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{Duration, TimeZone, Utc};
use common::{seed, state};
use domain::api::{CycleSettingsPatch, PatchGroupRequest};
use domain::{ApiErrorCode, GroupId, Newsletter, NewsletterStatus, Role, UserId};
use groups::group_routes;
use persistence::keys::{attr, group_pk, newsletter_gsi2sk, newsletter_sk};
use persistence::{groups as group_repo, newsletters, questions, test_factories, Repo};
use pretty_assertions::assert_eq;

fn window_patch(days: u32) -> PatchGroupRequest {
    PatchGroupRequest {
        name: None,
        timezone: None,
        gradient: None,
        cycle_settings: Some(CycleSettingsPatch {
            questions_per_cycle: None,
            votes_per_user_per_cycle: None,
            response_window_days: Some(days),
        }),
        notification_settings: None,
        member_soft_cap: None,
    }
}

/// An `open` cycle that opened three days ago on the default 7-day window.
async fn seed_open_cycle(repo: &Repo, group_id: &str) -> Newsletter {
    seed_open_cycle_opened_days_ago(repo, group_id, 3).await
}

/// An `open` cycle on the default 7-day window that opened `days` ago. Past
/// 7, its close has passed but the tick hasn't published it yet.
async fn seed_open_cycle_opened_days_ago(repo: &Repo, group_id: &str, days: i64) -> Newsletter {
    let now = Utc::now();
    let open_at = now - Duration::days(days);
    let nl = Newsletter {
        group_id: GroupId::new(group_id),
        cycle_id: domain::CycleId::new("202610"),
        status: NewsletterStatus::Open,
        vote_window_open_at: open_at - Duration::days(30),
        vote_window_close_at: open_at,
        response_open_at: open_at,
        response_close_at: open_at + Duration::days(7),
        published_at: None,
        next_transition_at: Some(open_at + Duration::days(7)),
        locked_question_ids: Vec::new(),
        notified_offsets_hours: Vec::new(),
        notified_on_open: false,
    };
    newsletters::write_status_transition(repo, &nl)
        .await
        .unwrap();
    nl
}

async fn raw_gsi2sk(repo: &Repo, nl: &Newsletter) -> String {
    let item = repo
        .client
        .get_item()
        .table_name(&repo.table)
        .key(attr::PK, AttributeValue::S(group_pk(&nl.group_id)))
        .key(attr::SK, AttributeValue::S(newsletter_sk(&nl.cycle_id)))
        .send()
        .await
        .unwrap()
        .item
        .unwrap();
    item[attr::GSI2SK].as_s().unwrap().clone()
}

async fn reload(repo: &Repo, nl: &Newsletter) -> Newsletter {
    newsletters::get_newsletter(repo, &nl.group_id, &nl.cycle_id)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn lengthening_the_window_moves_the_open_cycle_s_close() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-extend", &[("a1", Role::Admin)]).await;
    let nl = seed_open_cycle(&repo, "g-extend").await;
    let state = state(repo);

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-extend"),
        window_patch(10),
    )
    .await
    .unwrap();

    let moved = reload(&state.repo, &nl).await;
    let expected_close = nl.response_open_at + Duration::days(10);
    assert_eq!(moved.response_close_at, expected_close);
    assert_eq!(moved.next_transition_at, Some(expected_close));
    assert_eq!(moved.status, NewsletterStatus::Open);
    assert_eq!(
        raw_gsi2sk(&state.repo, &nl).await,
        newsletter_gsi2sk(expected_close, &nl.group_id, &nl.cycle_id)
    );
}

#[tokio::test]
async fn a_window_that_would_close_the_open_cycle_in_the_past_is_rejected_unchanged() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-shrink", &[("a1", Role::Admin)]).await;
    let nl = seed_open_cycle(&repo, "g-shrink").await;
    let state = state(repo);

    let err = group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-shrink"),
        window_patch(2),
    )
    .await
    .expect_err("opened 3 days ago, so a 2-day window closed yesterday");

    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "responseWindowDays");
    assert_eq!(err.field_errors[0].code, "CLOSES_IN_PAST");
    let group = group_repo::get_group(&state.repo, &GroupId::new("g-shrink"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.cycle_settings.response_window_days, 7);
    assert_eq!(reload(&state.repo, &nl).await, nl);
}

#[tokio::test]
async fn a_cycle_past_its_close_is_not_reopened_by_a_longer_window() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-closed", &[("a1", Role::Admin)]).await;
    let nl = seed_open_cycle_opened_days_ago(&repo, "g-closed", 8).await;
    let state = state(repo);

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-closed"),
        window_patch(10),
    )
    .await
    .expect("the setting still saves for later cycles");

    let group = group_repo::get_group(&state.repo, &GroupId::new("g-closed"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.cycle_settings.response_window_days, 10);
    assert_eq!(reload(&state.repo, &nl).await, nl);
}

#[tokio::test]
async fn rescheduling_refuses_a_cycle_whose_close_has_passed() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-closed-race", &[("a1", Role::Admin)]).await;
    let nl = seed_open_cycle_opened_days_ago(&repo, "g-closed-race", 8).await;

    let err = newsletters::reschedule_open_cycle_close(
        &repo,
        &nl.group_id,
        &nl.cycle_id,
        Utc::now() + Duration::days(2),
    )
    .await
    .expect_err("the window already closed");

    assert!(err.is_lost_race());
    assert_eq!(reload(&repo, &nl).await, nl);
}

#[tokio::test]
async fn a_voting_cycle_with_candidates_is_rescheduled() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-voting", &[("a1", Role::Admin)]).await;
    let tz: chrono_tz::Tz = shared::config::DEFAULT_TIMEZONE.parse().unwrap();
    let voting =
        newsletters::create_next_voting_cycle(&repo, &GroupId::new("g-voting"), tz, 7, Utc::now())
            .await
            .unwrap();
    let candidate = test_factories::candidate("g-voting", voting.cycle_id.as_str(), "q1", "a1", 2);
    questions::put_candidate(&repo, &candidate).await.unwrap();
    let state = state(repo);

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-voting"),
        window_patch(10),
    )
    .await
    .unwrap();

    let rescheduled = reload(&state.repo, &voting).await;
    assert_eq!(
        rescheduled.response_close_at,
        voting.response_open_at + Duration::days(10)
    );
}

#[tokio::test]
async fn resubmitting_the_current_window_repairs_a_close_that_failed_to_move() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-repair", &[("a1", Role::Admin)]).await;
    let nl = seed_open_cycle(&repo, "g-repair").await;
    // As if an earlier PATCH saved the default 7-day window but its
    // best-effort move of this cycle's close failed, leaving a 5-day close.
    let stale_close = nl.response_open_at + Duration::days(5);
    newsletters::reschedule_open_cycle_close(&repo, &nl.group_id, &nl.cycle_id, stale_close)
        .await
        .unwrap();
    let state = state(repo);

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-repair"),
        window_patch(7),
    )
    .await
    .unwrap();

    let repaired = reload(&state.repo, &nl).await;
    assert_eq!(
        repaired.response_close_at,
        nl.response_open_at + Duration::days(7)
    );
}

#[tokio::test]
async fn a_timezone_change_reschedules_the_voting_cycle_without_changing_its_id() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-tz-west", &[("a1", Role::Admin)]).await;
    let ny: chrono_tz::Tz = shared::config::DEFAULT_TIMEZONE.parse().unwrap();
    // Created well after its own month began, like a successor the repair
    // pass creates late: its `voteWindowOpenAt` (now) is no anchor for it.
    let after = Utc.with_ymd_and_hms(2030, 11, 1, 5, 0, 0).unwrap();
    let voting =
        newsletters::create_next_voting_cycle(&repo, &GroupId::new("g-tz-west"), ny, 7, after)
            .await
            .unwrap();
    assert_eq!(voting.cycle_id.as_str(), "203012");
    let state = state(repo);

    let mut patch = window_patch(7);
    patch.cycle_settings = None;
    patch.timezone = Some("Pacific/Honolulu".into());
    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-tz-west"),
        patch,
    )
    .await
    .unwrap();

    let rescheduled = reload(&state.repo, &voting).await;
    // Midnight 1 Dec 2030 in Honolulu (UTC-10).
    let open_at = Utc.with_ymd_and_hms(2030, 12, 1, 10, 0, 0).unwrap();
    assert_eq!(rescheduled.cycle_id, voting.cycle_id);
    assert_eq!(rescheduled.response_open_at, open_at);
    assert_eq!(rescheduled.response_close_at, open_at + Duration::days(7));
    assert_eq!(rescheduled.next_transition_at, Some(open_at));
}
