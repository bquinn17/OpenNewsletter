# backend

Rust workspace. Library crates hold domain types, persistence, and shared utilities; one `lambda-*` binary crate per Lambda function. Built with `cargo-lambda` for `provided.al2023`. See [`docs/overview.md` §4](../docs/overview.md#4-repo-layout) for the crate roster and [`docs/development/coding-standards.md` §2](../docs/development/coding-standards.md#2-rust) for Rust-specific conventions.

Build: `cargo build --workspace`. Test: `cargo test --workspace`.

First-time Rust/Docker setup differs by machine (Windows/WSL2 vs. macOS) — see [`docs/development/environments.md` §0](../docs/development/environments.md#0-workstation-setup) before running the Docker-backed integration tests, and §10 for which test layers need Docker at all.
