// API client.
//
// ⚠️ Currently backed by mocks in `src/mocks/`. Nothing in this app talks to a
// real backend yet. To wire up the real API, replace `mockApi` with a fetch-based
// implementation that matches the same method shape and update this re-export.

export { mockApi as api } from "../mocks/api";
export type { NewCandidatePayload, SaveResponseBody } from "../mocks/api";
