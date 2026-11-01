//! Process-wide state built once per cold start.

use crate::notify::Notifier;
use persistence::Repo;
use shared::cycle_time::CycleCadence;

pub struct AppState {
    pub repo: Repo,
    /// `dev` | `staging` | `prod` — the dev-only HTTP route refuses with
    /// `404 NOT_FOUND` unless this is exactly `dev` (`docs/design/api.md` §11a).
    pub env: String,
    /// Where cycle-open / publication fan-outs go (`notify::LambdaNotifier`
    /// in a deployed stack).
    pub notifier: Box<dyn Notifier>,
    /// This deployment's cycle cadence (`docs/design/lifecycle.md` §13),
    /// read once at cold start from `CYCLE_FREQUENCY`/`CYCLE_WEEKLY_START`.
    pub cadence: CycleCadence,
}

impl AppState {
    pub fn is_dev(&self) -> bool {
        self.env == "dev"
    }
}
