//! Process-wide state built once per cold start.

use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
    /// `dev` | `staging` | `prod` — the dev-only HTTP route refuses with
    /// `404 NOT_FOUND` unless this is exactly `dev` (`plans/03-api-contract.md` §11a).
    pub env: String,
}

impl AppState {
    pub fn is_dev(&self) -> bool {
        self.env == "dev"
    }
}
