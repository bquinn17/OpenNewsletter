# backend

Rust workspace. Library crates hold domain types, persistence, and shared utilities; one `lambda-*` binary crate per Lambda function. Built with `cargo-lambda` for `provided.al2023`. See [`../plans/00-overview.md`](../plans/00-overview.md) §4 for the crate roster and [`../plans/coding-standards.md`](../plans/coding-standards.md) §2 for Rust-specific conventions.

Build: `cargo build --workspace`. Test: `cargo test --workspace`.

First-time Rust/Docker setup differs by machine (Windows/WSL2 vs. macOS) — see [`../plans/13-dev-environments.md` §0](../plans/13-dev-environments.md#0-workstation-setup) before running the Docker-backed integration tests, and §10 for which test layers need Docker at all.
