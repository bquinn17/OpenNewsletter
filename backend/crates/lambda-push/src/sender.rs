//! The HTTP client that delivers an encrypted push message to a push
//! service (`plans/07-notifications.md` §10, M11 decision D1). Behind a
//! trait so integration tests script delivery outcomes without a real
//! network call.

use async_trait::async_trait;
use shared::config::PUSH_SEND_TIMEOUT_SECS;
use std::time::Duration;

/// What happened when we tried to deliver one push message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendResult {
    /// The push service answered with this HTTP status code.
    Status(u16),
    /// The request never got a response (DNS, TLS, timeout, connection reset...).
    NetworkError,
}

// `async_trait` marks its boxed-future return `#[must_use]`, which clippy's
// `double_must_use` flags on every method. Macro-generated, not ours to fix.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait PushSender: Send + Sync {
    async fn send(&self, endpoint: &str, headers: &[(String, String)], body: Vec<u8>)
        -> SendResult;
}

pub struct ReqwestPushSender {
    client: reqwest::Client,
}

impl ReqwestPushSender {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(PUSH_SEND_TIMEOUT_SECS))
            .build()
            .expect("reqwest client builds from static, valid configuration");
        Self { client }
    }
}

impl Default for ReqwestPushSender {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PushSender for ReqwestPushSender {
    async fn send(
        &self,
        endpoint: &str,
        headers: &[(String, String)],
        body: Vec<u8>,
    ) -> SendResult {
        let mut request = self.client.post(endpoint).body(body);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        match request.send().await {
            Ok(response) => SendResult::Status(response.status().as_u16()),
            Err(e) => {
                tracing::warn!(error = ?e, endpoint, "push send network error");
                SendResult::NetworkError
            }
        }
    }
}
