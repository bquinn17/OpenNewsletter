//! Process-wide state built once per cold start.

use persistence::Repo;

pub struct AppState {
    pub repo: Repo,
}
