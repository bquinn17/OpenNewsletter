//! `PATCH /groups/{g}` and `responseWindowDays` (`docs/design/lifecycle.md`
//! §6, `docs/design/api.md` §4.3).
//!
//! A `timezone`/`responseWindowDays` change never reschedules or moves any
//! existing cycle — only a cycle created after the change (the tick's
//! successor, or a group's first cycle) reads the new settings.
//!
//! Each test uses its own group id: `persistence::auth` keeps a process-wide
//! membership cache keyed by `(userId, groupId)`, and tests run concurrently.

mod common;

use aws_sdk_dynamodb::types::AttributeValue;
use chrono::{Duration, NaiveTime, Utc, Weekday};
use common::{seed, state, state_with_cadence};
use domain::api::{CycleSettingsPatch, PatchGroupRequest};
use domain::{ApiErrorCode, GroupId, Newsletter, NewsletterStatus, Role, UserId};
use groups::group_routes;
use persistence::keys::{attr, group_pk, newsletter_sk};
use persistence::{groups as group_repo, newsletters, test_factories, Repo};
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;

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

fn questions_per_cycle_patch(n: u32) -> PatchGroupRequest {
    PatchGroupRequest {
        name: None,
        timezone: None,
        gradient: None,
        cycle_settings: Some(CycleSettingsPatch {
            questions_per_cycle: Some(n),
            votes_per_user_per_cycle: None,
            response_window_days: None,
        }),
        notification_settings: None,
        member_soft_cap: None,
    }
}

/// An `open` cycle that opened three days ago on the default 7-day window.
async fn seed_open_cycle(repo: &Repo, group_id: &str) -> Newsletter {
    let now = Utc::now();
    let open_at = now - Duration::days(3);
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

/// A `voting` cycle for the month after `open`'s, so the two never collide.
async fn seed_voting_cycle(repo: &Repo, group_id: &str) -> Newsletter {
    let nl = test_factories::voting_newsletter(group_id, "202611");
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

fn weekly_cadence() -> CycleCadence {
    CycleCadence::Weekly {
        weekday: Weekday::Sun,
        at: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
    }
}

/// A `timezone`/`responseWindowDays` change must leave every existing cycle
/// (`voting` and `open`) byte-for-byte untouched — status, every timestamp,
/// and the GSI2 sort key all stay exactly as they were.
#[tokio::test]
async fn a_timezone_and_window_change_leaves_existing_cycles_untouched() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-no-reschedule", &[("a1", Role::Admin)]).await;
    let open = seed_open_cycle(&repo, "g-no-reschedule").await;
    let voting = seed_voting_cycle(&repo, "g-no-reschedule").await;
    let open_gsi2sk_before = raw_gsi2sk(&repo, &open).await;
    let voting_gsi2sk_before = raw_gsi2sk(&repo, &voting).await;
    let state = state(repo);

    let mut patch = window_patch(14);
    patch.timezone = Some("Pacific/Honolulu".into());
    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-no-reschedule"),
        patch,
    )
    .await
    .unwrap();

    assert_eq!(reload(&state.repo, &open).await, open);
    assert_eq!(reload(&state.repo, &voting).await, voting);
    assert_eq!(raw_gsi2sk(&state.repo, &open).await, open_gsi2sk_before);
    assert_eq!(raw_gsi2sk(&state.repo, &voting).await, voting_gsi2sk_before);

    let group = group_repo::get_group(&state.repo, &GroupId::new("g-no-reschedule"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.timezone, "Pacific/Honolulu");
    assert_eq!(group.cycle_settings.response_window_days, 14);
}

/// A stored `responseWindowDays` above the deployment's current cadence cap
/// (inherited from a monthly deployment that switched to weekly) is reported
/// as its clamped value by `GET`, and self-heals the next time any
/// `cycleSettings` patch is saved — even one that doesn't mention
/// `responseWindowDays`. An explicit over-cap value is still rejected.
#[tokio::test]
async fn a_stored_window_above_the_weekly_cap_is_clamped_on_read_and_self_heals() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-over-cap", &[("a1", Role::Admin)]).await;
    let mut group = group_repo::get_group(&repo, &GroupId::new("g-over-cap"))
        .await
        .unwrap()
        .unwrap();
    group.cycle_settings.response_window_days = 14;
    group_repo::put_group(&repo, &group).await.unwrap();
    let state = state_with_cadence(repo, weekly_cadence());

    let got = group_routes::get_group(&state, &UserId::new("a1"), &GroupId::new("g-over-cap"))
        .await
        .unwrap();
    assert_eq!(got.cycle_settings.response_window_days, 7);

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-over-cap"),
        questions_per_cycle_patch(6),
    )
    .await
    .unwrap();
    let stored = group_repo::get_group(&state.repo, &GroupId::new("g-over-cap"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.cycle_settings.response_window_days, 7);

    let err = group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-over-cap"),
        window_patch(8),
    )
    .await
    .expect_err("8 days exceeds the weekly cap");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "responseWindowDays");
}

/// `docs/design/lifecycle.md` §13 — a weekly deployment's `responseWindowDays`
/// cap is 7 (the period itself), not the monthly 28.
#[tokio::test]
async fn a_weekly_deployment_accepts_a_seven_day_window_but_rejects_eight() {
    let (_c, repo) = test_factories::make_test_repo().await;
    seed(&repo, "g-weekly-cap", &[("a1", Role::Admin)]).await;
    let state = state_with_cadence(repo, weekly_cadence());

    let err = group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-weekly-cap"),
        window_patch(8),
    )
    .await
    .expect_err("8 days exceeds the weekly cap");
    assert_eq!(err.code, ApiErrorCode::ValidationFailed);
    assert_eq!(err.field_errors[0].field, "responseWindowDays");

    group_routes::patch_group(
        &state,
        &UserId::new("a1"),
        &GroupId::new("g-weekly-cap"),
        window_patch(7),
    )
    .await
    .expect("7 days is exactly the weekly period");
}
