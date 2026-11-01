//! `GET /config` carries `cycleCadence` (`docs/design/lifecycle.md` §13):
//! monthly by default, and the deployment's actual cadence otherwise.

mod common;

use chrono::NaiveTime;
use chrono::Weekday;
use common::{state, state_with_cadence};
use domain::api::{CycleCadence as CycleCadenceDto, CycleFrequency};
use domain::CognitoSub;
use groups::me;
use persistence::test_factories;
use pretty_assertions::assert_eq;
use shared::cycle_time::CycleCadence;
use shared::http::AuthClaims;

fn claims() -> AuthClaims {
    AuthClaims {
        sub: CognitoSub::new("sub-no-such-user"),
        email: Some("nobody@example.com".into()),
        email_verified: true,
        name: None,
    }
}

#[tokio::test]
async fn config_defaults_to_monthly_cadence() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let state = state(repo);

    let config = me::get_config(&state, &claims()).await.unwrap();

    assert_eq!(
        config.cycle_cadence,
        CycleCadenceDto {
            frequency: CycleFrequency::Monthly,
            max_response_window_days: 28,
        }
    );
}

#[tokio::test]
async fn config_carries_a_weekly_cadence_when_the_deployment_is_weekly() {
    let (_c, repo) = test_factories::make_test_repo().await;
    let weekly = CycleCadence::Weekly {
        weekday: Weekday::Sun,
        at: NaiveTime::from_hms_opt(21, 0, 0).unwrap(),
    };
    let state = state_with_cadence(repo, weekly);

    let config = me::get_config(&state, &claims()).await.unwrap();

    assert_eq!(
        config.cycle_cadence,
        CycleCadenceDto {
            frequency: CycleFrequency::Weekly,
            max_response_window_days: 7,
        }
    );
}
