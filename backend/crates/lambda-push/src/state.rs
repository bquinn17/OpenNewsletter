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
    /// Secrets Manager ARN of `opennewsletter/vapid/{env}`
    /// (`plans/07-notifications.md` §2).
    pub vapid_secret_arn: String,
    pub sender: Arc<dyn PushSender>,
    /// `dev` | `staging` | `prod` — the dev-only HTTP route refuses with
    /// `404 NOT_FOUND` unless this is exactly `dev`
    /// (`plans/03-api-contract.md` §11a). Unused by `push-api` itself today
    /// (its dev route lives on `notify-tick`) but kept for parity with the
    /// other Lambdas' `AppState` and in case a push-side dev route is added.
    pub env: String,
    vapid: OnceCell<VapidKeys>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VapidSecretJson {
    public_key: String,
    private_key: String,
    subject: String,
}

impl AppState {
    pub fn new(
        repo: Repo,
        secrets: aws_sdk_secretsmanager::Client,
        vapid_secret_arn: String,
        sender: Arc<dyn PushSender>,
        env: String,
    ) -> Self {
        Self {
            repo,
            secrets,
            vapid_secret_arn,
            sender,
            env,
            vapid: OnceCell::new(),
        }
    }

    pub fn is_dev(&self) -> bool {
        self.env == "dev"
    }

    /// The VAPID keypair, fetched from Secrets Manager on first use and
    /// cached for the life of the warm container (`07-notifications.md` §2,
    /// M11 decision D1).
    pub async fn vapid_keys(&self) -> Result<&VapidKeys, ApiError> {
        self.vapid
            .get_or_try_init(|| async {
                let resp = self
                    .secrets
                    .get_secret_value()
                    .secret_id(&self.vapid_secret_arn)
                    .send()
                    .await
                    .map_err(|e| {
                        tracing::error!(error = ?e, "failed to fetch vapid secret");
                        ApiError::internal("failed to load vapid keys")
                    })?;
                let raw = resp.secret_string().ok_or_else(|| {
                    tracing::error!("vapid secret has no SecretString");
                    ApiError::internal("vapid secret has no SecretString")
                })?;
                let parsed: VapidSecretJson = serde_json::from_str(raw).map_err(|e| {
                    tracing::error!(error = ?e, "vapid secret is not valid JSON");
                    ApiError::internal("vapid secret is not valid JSON")
                })?;
                VapidKeys::parse(&parsed.public_key, &parsed.private_key, parsed.subject).map_err(
                    |e| {
                        tracing::error!(error = ?e, "invalid vapid keypair");
                        ApiError::internal("invalid vapid keypair")
                    },
                )
            })
            .await
    }

    /// Escape hatch for tests: inject a keypair directly so they never need
    /// a Secrets Manager call (mirrors `lambda-media::state::AppState::set_signing_key_for_test`).
    /// A no-op if the keys have already been loaded.
    pub fn set_vapid_keys_for_test(&self, keys: VapidKeys) {
        let _ = self.vapid.set(keys);
    }
}
