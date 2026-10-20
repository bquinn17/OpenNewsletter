//! Process-wide state built once per cold start.

use crate::sender::PushSender;
use crate::webpush::VapidKeys;
use domain::ApiError;
use persistence::Repo;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub struct AppState {
    pub repo: Repo,
    pub secrets: aws_sdk_secretsmanager::Client,
    /// Secrets Manager ARN of the per-env app secret `opennewsletter/{env}`,
    /// whose JSON carries the VAPID keys (`docs/design/notifications.md` §2).
    pub app_secret_arn: String,
    pub sender: Arc<dyn PushSender>,
    /// `dev` | `staging` | `prod` — the dev-only HTTP route refuses with
    /// `404 NOT_FOUND` unless this is exactly `dev`
    /// (`docs/design/api.md` §11a). Unused by `push-api` itself today
    /// (its dev route lives on `notify-tick`) but kept for parity with the
    /// other Lambdas' `AppState` and in case a push-side dev route is added.
    pub env: String,
    vapid: OnceCell<VapidKeys>,
}

/// The VAPID part of the app secret's JSON. The secret holds other keys
/// too (OAuth credentials); serde ignores them.
#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct VapidSecret {
    pub vapid_public_key: String,
    pub vapid_private_key: String,
    pub vapid_subject: String,
}

/// Parse the app secret's `SecretString` for its VAPID keys.
pub fn parse_vapid_secret(raw: &str) -> Result<VapidSecret, serde_json::Error> {
    serde_json::from_str(raw)
}

impl AppState {
    pub fn new(
        repo: Repo,
        secrets: aws_sdk_secretsmanager::Client,
        app_secret_arn: String,
        sender: Arc<dyn PushSender>,
        env: String,
    ) -> Self {
        Self {
            repo,
            secrets,
            app_secret_arn,
            sender,
            env,
            vapid: OnceCell::new(),
        }
    }

    pub fn is_dev(&self) -> bool {
        self.env == "dev"
    }

    /// The VAPID keypair, fetched from Secrets Manager on first use and
    /// cached for the life of the warm container (`docs/design/notifications.md` §2,
    /// M11 decision D1).
    pub async fn vapid_keys(&self) -> Result<&VapidKeys, ApiError> {
        self.vapid
            .get_or_try_init(|| async {
                let resp = self
                    .secrets
                    .get_secret_value()
                    .secret_id(&self.app_secret_arn)
                    .send()
                    .await
                    .map_err(|e| {
                        tracing::error!(error = ?e, "failed to fetch the app secret");
                        ApiError::internal("failed to load vapid keys")
                    })?;
                let raw = resp.secret_string().ok_or_else(|| {
                    tracing::error!("app secret has no SecretString");
                    ApiError::internal("app secret has no SecretString")
                })?;
                // serde_json's error names the missing key, never a value.
                let parsed = parse_vapid_secret(raw).map_err(|e| {
                    tracing::error!(error = %e, "app secret lacks valid vapid keys");
                    ApiError::internal("app secret lacks valid vapid keys")
                })?;
                VapidKeys::parse(
                    &parsed.vapid_public_key,
                    &parsed.vapid_private_key,
                    parsed.vapid_subject,
                )
                .map_err(|e| {
                    tracing::error!(error = ?e, "invalid vapid keypair");
                    ApiError::internal("invalid vapid keypair")
                })
            })
            .await
    }

    /// Escape hatch for tests: inject a keypair directly so they never need
    /// a Secrets Manager call.
    /// A no-op if the keys have already been loaded.
    pub fn set_vapid_keys_for_test(&self, keys: VapidKeys) {
        let _ = self.vapid.set(keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vapid_keys_are_read_from_the_app_secret_ignoring_other_keys() {
        let raw = r#"{
            "google_client_id": "g",
            "vapid_public_key": "pub",
            "vapid_private_key": "priv",
            "vapid_subject": "mailto:ops@example.com"
        }"#;
        assert_eq!(
            parse_vapid_secret(raw).unwrap(),
            VapidSecret {
                vapid_public_key: "pub".into(),
                vapid_private_key: "priv".into(),
                vapid_subject: "mailto:ops@example.com".into(),
            }
        );
    }

    #[test]
    fn an_app_secret_without_vapid_keys_is_rejected() {
        let raw = r#"{"vapid_public_key": "pub", "vapid_private_key": "priv"}"#;
        assert!(parse_vapid_secret(raw).is_err());
    }
}
