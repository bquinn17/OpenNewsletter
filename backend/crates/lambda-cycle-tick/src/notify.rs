//! Notification fan-out hooks for lifecycle transitions.
//!
//! Real delivery is `lambda-notify-tick` / `lambda-push`, an M11 deliverable
//! (`plans/07-notifications.md`). These are named no-ops so the exact points
//! in the lifecycle that must eventually trigger a fan-out — cycle open,
//! cycle publish — already exist and are exercised by every tick run, rather
//! than being a TODO someone has to remember to wire in later.

use domain::{CycleId, GroupId};

/// Called once a cycle transitions `voting -> open`. Will fan out the
/// cycle-open push notification (`07-notifications.md` §4) once M11 lands.
pub fn cycle_opened(group_id: &GroupId, cycle_id: &CycleId) {
    tracing::debug!(
        group_id = %group_id,
        cycle_id = %cycle_id,
        "cycle-open notification fanout is a no-op until M11"
    );
}

/// Called once a cycle transitions `open -> published`. Will fan out the
/// publication push notification (`07-notifications.md` §5) once M11 lands.
pub fn cycle_published(group_id: &GroupId, cycle_id: &CycleId) {
    tracing::debug!(
        group_id = %group_id,
        cycle_id = %cycle_id,
        "publication notification fanout is a no-op until M11"
    );
}
