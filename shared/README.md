# shared

Cross-cutting artifacts consumed by both frontend and backend. `openapi.yaml` is the
API contract; TypeScript types are generated from it via
[`../scripts/codegen_types.sh`](../scripts/codegen_types.sh) into
`frontend/src/types/api.ts`, and Rust types in
[`../backend/crates/domain/src/api.rs`](../backend/crates/domain/src/api.rs) are
hand-written but contract-tested against it
([`../plans/11-testing-ci-cd.md`](../plans/11-testing-ci-cd.md) §2.3).

It currently covers every route through Milestone 4
([`../plans/03-api-contract.md`](../plans/03-api-contract.md) §2-§4, §13). A route
added in a later milestone updates this file in the same PR
([`../plans/coding-standards.md`](../plans/coding-standards.md) §8).
