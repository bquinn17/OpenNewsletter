//! Notification fan-out hooks for lifecycle transitions.
//!
//! The tick itself never sends a push: it has no VAPID secret and a large
//! group's fan-out shouldn't hold up every other group's transitions. Instead,
//! after a cycle opens or publishes, it async-invokes `lambda-push`
//! (`InvocationType::Event`) with the internal envelope
//! `{"internal": "cycle_open_fanout"|"publication_fanout", "groupId", "cycleId"}`
//! (`plans/07-notifications.md` §6, M11). `lambda-push` owns the
//! `NOTIFIED#OPEN`/`NOTIFIED#PUBLISH` idempotency markers, so a duplicate
//! invoke from a retried tick is harmless.

use async_trait::async_trait;
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::types::InvocationType;
use domain::{CycleId, GroupId};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanoutKind {
    /// `voting -> open` (`07-notifications.md` §4).
    CycleOpen,
    /// `open -> published` (`07-notifications.md` §5).
    Publication,
}

impl FanoutKind {
    pub fn internal_tag(self) -> &'static str {
        match self {
            FanoutKind::CycleOpen => "cycle_open_fanout",
            FanoutKind::Publication => "publication_fanout",
        }
    }
}

/// The payload `lambda-push`'s `dispatch::handle_internal` parses.
pub fn fanout_envelope(kind: FanoutKind, group_id: &GroupId, cycle_id: &CycleId) -> Value {
    json!({
        "internal": kind.internal_tag(),
        "groupId": group_id.to_string(),
        "cycleId": cycle_id.to_string(),
    })
}

/// Triggers a fan-out after a transition. Implementations must never fail
/// the tick: errors are logged and swallowed.
// `async_trait` marks its boxed-future return `#[must_use]`, which clippy's
// `double_must_use` flags on every method. Macro-generated, not ours to fix.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn fanout(&self, kind: FanoutKind, group_id: &GroupId, cycle_id: &CycleId);
}

/// Used by [`crate::tick::run_tick`] (other crates' lifecycle tests) and when
/// `PUSH_FUNCTION_NAME` is unset.
pub struct NoopNotifier;

#[async_trait]
impl Notifier for NoopNotifier {
    async fn fanout(&self, kind: FanoutKind, group_id: &GroupId, cycle_id: &CycleId) {
        tracing::debug!(kind = kind.internal_tag(), group_id = %group_id, cycle_id = %cycle_id, "notification fanout skipped (no-op notifier)");
    }
}

/// Async-invokes `lambda-push` by function name.
pub struct LambdaNotifier {
    pub client: aws_sdk_lambda::Client,
    pub function_name: String,
}

#[async_trait]
impl Notifier for LambdaNotifier {
    async fn fanout(&self, kind: FanoutKind, group_id: &GroupId, cycle_id: &CycleId) {
        let payload = fanout_envelope(kind, group_id, cycle_id).to_string();
        let result = self
            .client
            .invoke()
            .function_name(&self.function_name)
            .invocation_type(InvocationType::Event)
            .payload(Blob::new(payload))
            .send()
            .await;
        match result {
            Ok(_) => {
                tracing::info!(kind = kind.internal_tag(), group_id = %group_id, cycle_id = %cycle_id, "notification fanout invoked")
            }
            Err(e) => {
                tracing::error!(error = ?e, kind = kind.internal_tag(), group_id = %group_id, cycle_id = %cycle_id, "failed to invoke notification fanout")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn builds_the_envelope_lambda_push_expects() {
        let g = GroupId::new("01HG2");
        let c = CycleId::new("202606");
        assert_eq!(
            fanout_envelope(FanoutKind::CycleOpen, &g, &c),
            json!({ "internal": "cycle_open_fanout", "groupId": "01HG2", "cycleId": "202606" })
        );
        assert_eq!(
            fanout_envelope(FanoutKind::Publication, &g, &c)["internal"],
            "publication_fanout"
        );
    }
}
