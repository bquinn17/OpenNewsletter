# shared

Cross-cutting artifacts consumed by both frontend and backend. `openapi.yaml` is the
API contract; TypeScript types are generated from it via
[`../scripts/codegen_types.sh`](../scripts/codegen_types.sh) into
`frontend/src/types/api.ts`, and Rust types in
[`../backend/crates/domain/src/api.rs`](../backend/crates/domain/src/api.rs) are
hand-written but contract-tested against it
([`docs/development/testing-and-ci.md` §2.3](../docs/development/testing-and-ci.md#23-contract-test)).

It covers every route, and the contract test round-trips its examples through the Rust types. A
new or changed route updates this file in the same PR
([`docs/development/coding-standards.md` §8](../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
