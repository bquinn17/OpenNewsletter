# shared

Cross-cutting artifacts consumed by both frontend and backend. `openapi.yaml` is the
API contract; TypeScript types are generated from it via
[`../scripts/codegen_types.sh`](../scripts/codegen_types.sh) into
`frontend/src/types/api.ts`, and Rust types in
[`../backend/crates/domain/src/api.rs`](../backend/crates/domain/src/api.rs) are
hand-written but contract-tested against it
([`docs/development/testing-and-ci.md` §2.3](../docs/development/testing-and-ci.md#23-contract-test)).

It currently covers every route through Milestone 4
([`docs/design/api.md` §2](../docs/design/api.md#2-bootstrap--metadata-routes)-§4, §13). A route
added in a later milestone updates this file in the same PR
([`docs/development/coding-standards.md` §8](../docs/development/coding-standards.md#8-pre-flight-checklist-for-any-code-writing-task)).
