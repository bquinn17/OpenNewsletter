# shared

Cross-cutting artifacts consumed by both frontend and backend. `openapi.yaml` is the
API contract; TypeScript types are generated from it and Rust types are hand-written
but contract-tested against it.

**`openapi.yaml` does not exist yet** — it is a Milestone 5 deliverable
([`../plans/12-build-order.md`](../plans/12-build-order.md), "OpenAPI contract").
Until it lands, [`../plans/03-api-contract.md`](../plans/03-api-contract.md) is the
contract, and the M4 routes were written from it directly.
