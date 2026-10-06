//! EMF metric emission for push fan-outs and the test-push route
//! (`plans/07-notifications.md` §13, M11 decision D9). One EMF log line per
//! fan-out/test call; CloudWatch Logs extracts the metrics directly from
//! the structured log, no separate `PutMetricData` call needed.

use crate::delivery::DeliveryOutcome;
use serde_json::json;

#[derive(Debug)]
pub struct FanoutMetrics {
    kind: String,
    sent: u64,
    failed: u64,
    expired: u64,
    latencies_ms: Vec<u64>,
}

impl FanoutMetrics {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            sent: 0,
            failed: 0,
            expired: 0,
            latencies_ms: Vec::new(),
        }
    }

    /// Successful deliveries so far.
    pub fn delivered_count(&self) -> u32 {
        self.sent as u32
    }

    /// Failed + expired sends so far — the dev notify-tick route's `failed`
    /// field counts both (`plans/03-api-contract.md` §11a.2).
    pub fn failed_count(&self) -> u32 {
        (self.failed + self.expired) as u32
    }

    pub fn record(&mut self, outcome: DeliveryOutcome, latency_ms: u64) {
        match outcome {
            DeliveryOutcome::Delivered => self.sent += 1,
            DeliveryOutcome::Expired => self.expired += 1,
            DeliveryOutcome::Failed => self.failed += 1,
        }
        self.latencies_ms.push(latency_ms);
    }

    /// Emits the EMF log line to stdout. A no-op call (no sends recorded)
    /// still emits a zeroed line — useful for confirming a fan-out ran at
    /// all.
    pub fn emit(&self) {
        let env = std::env::var("ENV").unwrap_or_default();
        let line = json!({
            "_aws": {
                "Timestamp": chrono::Utc::now().timestamp_millis(),
                "CloudWatchMetrics": [{
                    "Namespace": format!("OpenNewsletter/{env}"),
                    "Dimensions": [["kind"], []],
                    "Metrics": [
                        {"Name": "PushSent", "Unit": "Count"},
                        {"Name": "PushFailed", "Unit": "Count"},
                        {"Name": "PushExpired", "Unit": "Count"},
                        {"Name": "PushLatencyMs", "Unit": "Milliseconds"},
                    ],
                }],
            },
            "kind": self.kind,
            "PushSent": self.sent,
            "PushFailed": self.failed,
            "PushExpired": self.expired,
            "PushLatencyMs": self.latencies_ms,
        });
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_tally_by_outcome() {
        let mut m = FanoutMetrics::new("cycle_open");
        m.record(DeliveryOutcome::Delivered, 100);
        m.record(DeliveryOutcome::Delivered, 150);
        m.record(DeliveryOutcome::Expired, 5);
        m.record(DeliveryOutcome::Failed, 20);
        assert_eq!(m.sent, 2);
        assert_eq!(m.expired, 1);
        assert_eq!(m.failed, 1);
        assert_eq!(m.latencies_ms, vec![100, 150, 5, 20]);
    }
}
