# backend

Rust workspace. Library crates hold domain types, persistence, and shared utilities; one `lambda-*` binary crate per Lambda function. Built with `cargo-lambda` for `provided.al2023`. See [`../plans/00-overview.md`](../plans/00-overview.md) §4 for the crate roster and [`../plans/coding-standards.md`](../plans/coding-standards.md) §2 for Rust-specific conventions.

Build: `cargo build --workspace`. Test: `cargo test --workspace`.
