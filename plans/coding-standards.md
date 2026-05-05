# Coding Standards

**Required reading for any agent that will write, edit, or review code in this repo.**
Pair this with [`00-overview.md`](00-overview.md) and the plan documents relevant to your task.

The goal of this document is to keep the codebase legible and uniform across the four languages it touches (Rust, TypeScript, Python, YAML/CDK config) without imposing rules that don't earn their keep. If a rule below feels wrong for a specific situation, the right move is to surface it (and propose an edit to this file), not to silently break it.

---

## 1. Universal principles

These apply regardless of language.

### 1.1 Build the smallest thing that works
- Don't add features, configuration, abstractions, or generality the current task doesn't require. Three similar lines beat a premature helper. A bug fix doesn't need surrounding cleanup.
- Don't design for hypothetical future requirements. When the second use case arrives, refactor *then*.
- Half-finished implementations are forbidden. If a task can't be completed, leave the code in the prior working state and surface the blocker.

### 1.2 Trust the type system and framework guarantees
- Don't add defensive null/empty/range checks for values the type system already guarantees. Validate only at system boundaries: incoming HTTP requests, S3 events, JWT claims, Cognito triggers, env vars at startup.
- Inside the system, internal callers can be trusted. If a function takes a `Group`, it doesn't need to re-check that `group.id` is non-empty.
- Don't add fallbacks for scenarios that can't happen. They rot and accumulate.

### 1.3 Match existing patterns before inventing new ones
- Read 2–3 nearby files before adding a new one. If there's an existing pattern (key-builder helper, error type, request handler shape), use it.
- New patterns are fine when justified — but call them out in the PR description.

### 1.4 Comments
- Default to writing none. A well-named function and obvious code don't need narration.
- Write a comment only when the **why** is non-obvious: a hidden constraint, a subtle invariant, a workaround for a specific bug, behavior that would surprise a reader.
- Never explain **what** the code does. If the code isn't self-evident, rename or refactor instead.
- Never reference the current task, ticket, or commit ("added for the X flow", "fixes issue 123"). That belongs in the PR/commit, not the source.
- Public APIs (Rust pub fns/types, exported TS functions, CDK construct constructors) get a single-line doc when the name alone isn't enough. Multi-paragraph docstrings are reserved for genuinely tricky semantics.

### 1.5 Naming
- Names communicate intent. Variables, functions, and types should read like a sentence describing the thing they represent.
- Avoid abbreviations except for: `id`, `db`, `req`/`res`, `ctx`, `tx` (transaction), `fn`, single-letter loop indices in tight scopes, well-known acronyms (`URL`, `HTTP`, `JWT`, `S3`).
- A name carrying a unit must spell the unit out: `expires_in_seconds`, `max_bytes`, `image_max_bytes`, never `timeout` or `size` alone.
- Booleans read as predicates: `is_admin`, `has_membership`, `should_publish`. Not `admin`, `membership_exists` (use the membership directly), or `publish` (verb collision).

### 1.6 Errors
- Errors should describe the failure with enough context to debug from logs alone. Include identifiers (groupId, cycleId, userId) where relevant.
- Don't swallow errors with a generic catch-all and a generic message. If you can't handle it specifically, propagate.
- The API error catalog in [`03-api-contract.md`](03-api-contract.md) §1.1 is the single source of truth for error codes. Don't invent new ones without adding them there.

### 1.7 Logging
- Logs are structured (JSON). Always include `correlation_id` taken from the request context.
- Log levels:
  - `error`: a request/job failed in a way that requires investigation
  - `warn`: a recoverable condition we want visibility on (e.g. push subscription expired, retried transaction)
  - `info`: state transitions, request boundaries, scheduled-job completions
  - `debug`: per-iteration detail, only useful when actively debugging
- Don't log secrets, tokens, full JWT bodies, or full image bytes.
- Don't log inside hot loops at info level.

### 1.8 Tests
- Tests describe behavior, not implementation. Renaming an internal function shouldn't break a test.
- Test names are sentences: `it_rejects_oversized_uploads`, `tick_promotes_top_n_candidates`. Skip `_test`/`should_` prefixes — they add noise without information.
- Arrange / Act / Assert. Keep arrange short by using factories.
- One logical assertion per test. "Logical" allows multiple `assert_eq` that verify a single behavior, but a test that exercises two unrelated things is two tests.
- Don't share mutable state between tests. Build fresh fixtures.
- Cover the happy path AND the failure modes documented for the function.

### 1.9 Performance
- Don't optimize speculatively. Write the obvious code; measure if it matters.
- For the few cases where performance is part of the spec (cold-start budget on Lambdas, autosave debounce, image pipeline duration): the spec lives in the relevant plan doc; honor it.

### 1.10 Security
- No secrets in source, env files committed to git, comments, logs, error messages, or test fixtures.
- Validate user input at the API boundary using the schemas in [`03-api-contract.md`](03-api-contract.md).
- Use parameterized DynamoDB expression placeholders (`#name`, `:value`). Never interpolate user input into expression strings.
- Tenant isolation is non-negotiable: every group-scoped handler MUST call `require_membership` (or admin variant) before touching DynamoDB. There is no exception.
- When in doubt, refer to [`05-auth-flow.md`](05-auth-flow.md) §12 (threat model).

### 1.11 Files and folders
- File path is part of the API. Don't move files casually; if you do, update every reference and grep for stale ones.
- Match the layout in [`00-overview.md`](00-overview.md) §4. New files go where their kind goes; if no obvious home exists, propose adding the home.
- One concept per file. A file > 400 lines is a smell; consider splitting.

### 1.12 Don't write what you can generate
- TypeScript API types come from `shared/openapi.yaml` via `openapi-typescript`. Don't hand-edit `frontend/src/types/api.ts`.
- Rust API types: hand-written but contract-tested against the YAML (see [`11-testing-ci-cd.md`](11-testing-ci-cd.md) §2.3). Keep them in sync; a failing contract test means the YAML is the truth.
- Don't duplicate the OpenAPI spec into prose elsewhere. Reference it.

---

## 2. Rust

### 2.1 Toolchain
- Pinned via `rust-toolchain.toml` (1.79+ stable). Don't override locally.
- Edition 2021.
- `cargo-lambda` for Lambda builds.

### 2.2 Formatting and linting
- `cargo fmt` is law. Never check in unformatted code.
- `cargo clippy --all-targets --all-features -- -D warnings` must be clean. If a clippy lint must be allowed for a justified reason, allow it at the smallest scope possible (function, not file, not crate) with a one-line comment explaining why.
- `cargo deny check` (license + advisory + duplicate dep policies) clean.

### 2.3 Error handling
- **Library crates** (`domain`, `persistence`, `shared`): use [`thiserror`]. Each crate's errors are a focused enum (`DomainError`, `RepoError`, `ApiError`).
- **Binary crates** (each `lambda-*`): use the focused errors from libraries; map them at the handler boundary into `ApiError` (which is the wire-shape per [`03-api-contract.md`](03-api-contract.md) §1.1). `anyhow` is acceptable only in `main.rs` glue code that wires the runtime.
- Never `.unwrap()` or `.expect()` outside `#[cfg(test)]` and the lambda runtime entry point. The compiler's `?` and `match` cover everything else.
- Use `From` impls to bubble errors up — manual `.map_err(|e| ...)` only when you genuinely want to add context.
- Add context via `tracing::error!(error = ?e, group_id = %g, "...")` rather than wrapping every error in a new variant.

### 2.4 Async
- All Lambdas run on `tokio`. Don't introduce a second runtime.
- Never call blocking I/O from an async context. Use `tokio::task::spawn_blocking` for CPU work in `lambda-image-process`.
- Don't `await` inside a tight loop when you could batch (e.g. use `try_join_all` for parallelizable independent calls).

### 2.5 Module organization
- One concern per module. The persistence crate's per-entity files (`users.rs`, `groups.rs`, etc.) follow this — emulate that pattern when adding new entities.
- `mod.rs` files are for re-exports and module-level docs only. Put logic in named files.
- Public API of a crate is its `lib.rs` re-exports. If something isn't there, it's private.

### 2.6 Types
- Prefer newtypes over raw `String`/`Uuid` for IDs that have semantic meaning: `UserId(String)`, `GroupId(String)`, `CycleId(String)`. Implement `Display`, `FromStr`, and `Serialize`/`Deserialize`.
- Enums, not stringly-typed status fields, in domain code. Convert at the persistence boundary.
- Derive what you need, no more: `Debug` always, `Clone` only if you actually clone, `PartialEq`/`Eq` when you compare, `Default` only if a default makes semantic sense.

### 2.7 Tests
- Unit tests in the same file under `#[cfg(test)] mod tests`.
- Integration tests in `crate-name/tests/*.rs` (one file per scenario suite).
- Test factory functions live in `persistence/src/test_factories.rs`, gated `#[cfg(any(test, feature = "test-utils"))]` so other crates' tests can pull them via the `test-utils` feature.
- Run integration tests against DDB-local via `testcontainers`. Each test gets a unique table name; no shared state.
- Use `pretty_assertions::assert_eq` for non-trivial comparisons (better diff output).

### 2.8 Logging
- `tracing` + `tracing-subscriber`. JSON formatter in production; pretty in tests.
- One `#[instrument]` per Lambda handler entry point, with `skip(repo, body)` to keep secrets/payloads out.
- Field names use snake_case: `group_id`, `cycle_id`, not `GroupId`.

### 2.9 Concurrency
- Static caches use `once_cell::sync::Lazy<DashMap<...>>` — never global `Mutex` for read-heavy data.
- `Arc<T>` for shared owned data. Don't reach for `Rc` (single-threaded only; we're tokio).
- Never `.lock().unwrap()` in async; use `tokio::sync::Mutex` if you genuinely need an async-safe lock (rarely).

### 2.10 Dependencies
- Don't add a dependency for a one-liner. Prefer the standard library.
- Justify new dependencies in the PR description. License must be MIT/Apache-2.0/BSD-equivalent (`cargo deny` enforces this).
- Pin major versions in `Cargo.toml`; allow patch updates.

---

## 3. TypeScript / React

### 3.1 Toolchain
- TypeScript 5, `strict: true`. No `// @ts-ignore` without an inline justification.
- React 18, function components only. No class components.
- Vite for dev/build.

### 3.2 Formatting and linting
- Prettier — defaults except `printWidth: 100`, `singleQuote: false`, `trailingComma: "all"`.
- ESLint flat config with `@typescript-eslint`, `react`, `react-hooks`, `import`, `jsx-a11y`. All `error`-level rules must be clean.
- `tsc --noEmit` clean (the CI step).
- Stylelint on CSS files (mostly enforces nothing beyond Tailwind class ordering; the `prettier-plugin-tailwindcss` does the heavy lifting in TSX).

### 3.3 Types
- Never `any`. Use `unknown` and narrow.
- Prefer `type` over `interface` for shape aliases; use `interface` only when extension matters (rare in this app).
- Inline component props as a `type Props = {...}` directly above the component unless reused.
- API types come from `frontend/src/types/api.ts` (generated). Do not redefine.
- No `as` casts except: (a) when narrowing `unknown` after a runtime check, (b) when interacting with badly-typed third-party libs (justified inline).

### 3.4 Components
- One component per file. File name matches export (`AnswerCard.tsx` exports `AnswerCard`).
- Props destructured in the parameter list, not inside the body.
- No default exports. Named exports only — easier to grep, easier to refactor.
- No business logic in components. Components consume hooks; hooks consume the API client.
- Don't `useEffect` for things that aren't effects. State derived from props is just an expression. Server data is a TanStack query.

### 3.5 State
- Server state: TanStack Query. Period. No `useState` mirror of server data.
- Client state: Zustand stores in `state/`. Stores are small and focused (`currentGroup.ts`, `draftBuffers.ts`, `toast.ts`). Don't build a single mega-store.
- URL state belongs in the URL (search params), not local state, when the user might bookmark or share.
- React Context is for read-only, rarely-changing data: auth, theme. Not for app state.

### 3.6 Hooks
- Custom hooks live in `api/queries.ts` / `api/mutations.ts` for server, or co-located with the component that uses them when component-specific.
- Hook naming: `use<Noun>` for queries, `use<Verb>` for mutations and effects (`useSaveDraft`, `useEnsurePushSubscription`).
- Follow `react-hooks/exhaustive-deps`. If you must disable, justify inline.

### 3.7 Styling
- Tailwind-first. Custom CSS only for things Tailwind can't express cleanly (complex animations, prose typography overrides).
- Class lists: prefer `clsx` for conditional classes. Don't string-concatenate.
- No inline styles except for genuinely dynamic values (e.g., a stacked-bar width from a vote percentage).
- Mobile-first: classes without prefix are mobile; layer `md:` and `lg:` for larger screens.
- Constrain content to `max-w-2xl mx-auto` on `lg:` screens — the "middle third" requirement.

### 3.8 Forms
- `react-hook-form` + `zod`. Don't write controlled inputs by hand.
- The zod schema is the validation source of truth; never validate again in the submit handler.
- Surface errors via `aria-describedby` + an `aria-live="polite"` region; not just visual.

### 3.9 Tests
- Vitest + Testing Library. Test files co-located: `Button.test.tsx` next to `Button.tsx`.
- Test from the user's perspective: query by role, label, text. Avoid `getByTestId` unless there's no other accessible way.
- MSW (Mock Service Worker) intercepts API calls; use the OpenAPI-derived handlers in `tests/msw/`.
- Don't assert on internal state, render output structure, or class names. Assert on what the user sees.

### 3.10 Accessibility
- Every interactive element is reachable by keyboard.
- Every `<img>` has an `alt`. User-uploaded images require user-provided alt text (field next to the upload thumbnail).
- Every form control has an associated `<label>`.
- `prefers-reduced-motion` respected for transitions and toasts.
- Color-only signaling is forbidden — pair with text or icon.
- `axe-core` runs as part of E2E tests (Playwright + `@axe-core/playwright`).

---

## 4. Python (CDK)

### 4.1 Toolchain
- Python 3.12.
- `ruff` for lint + format (replaces black/isort).
- `mypy --strict` for typing.
- `pytest` for tests.

### 4.2 Style
- Type hints required on every function signature.
- Dataclasses with `frozen=True` for config; never raw dicts for structured config.
- Use the CDK Python idiom: pass props via constructor kwargs, expose outputs as `@property`.
- Never use `cdk.aws_*` aliases inconsistently. Pick one import style per file.

### 4.3 Stack composition
- One stack per concern (matches [`01-infrastructure-cdk.md`](01-infrastructure-cdk.md) §1).
- Cross-stack references via constructor parameters, never via global lookup.
- IAM policies use explicit `iam.PolicyStatement` with `actions` and `resources` lists. Never `actions=["*"]` outside CloudWatch Logs (and even there, scope to the function's log group ARN).

### 4.4 Tests
- `infra/tests/test_<stack>.py` per stack.
- Use `aws_cdk.assertions.Template.from_stack` and assert on resource counts, properties, and IAM policies — not on the synthesized JSON's exact shape.

---

## 5. YAML / config / scripts

- 2-space indent, no tabs.
- Comments at the top of every workflow file explaining its trigger and purpose.
- `scripts/` files: shebang line, `set -euo pipefail` for bash; `if __name__ == "__main__":` guard for Python. Single-purpose.
- Never check in `.env` files. Commit `.env.example` instead.

---

## 6. Git, commits, PRs

### 6.1 Commit messages
- Format: `<type>(<scope>): <subject>` where `type` ∈ {feat, fix, refactor, test, docs, chore, perf, build, ci}. Scope is optional but encouraged: `feat(media): generate webp thumbnails`.
- Subject in imperative mood, no trailing period, ≤ 72 chars.
- Body (optional) explains **why**. Wrap at 80 cols. Reference the milestone or plan doc when relevant: "per `06-newsletter-lifecycle.md` §5".
- Co-author footer when AI-generated.

### 6.2 Commit hygiene
- One concept per commit. A milestone is multiple commits, not one.
- Commits compile and pass tests in isolation. Don't push a "WIP" commit followed by a "fix WIP" commit; squash before review.
- Never commit secrets, ARNs from a sibling environment, or operator-specific paths.

### 6.3 PRs
- Title matches commit subject convention.
- Description sections (use as a template):
  - **What** — one or two sentences
  - **Why** — link the plan doc / milestone
  - **How tested** — local commands run, screenshots if UI
  - **Risks / follow-ups**
- Keep PRs small. A PR > 600 lines of diff is a smell; consider splitting.
- The PR author runs the test commands locally before requesting review. CI is the safety net, not the first signal.

### 6.4 What CI gates merging
Per [`11-testing-ci-cd.md`](11-testing-ci-cd.md):
- Lint, format, type-check
- All unit + integration tests
- `cdk synth` for infra changes
- (Nightly, not per-PR) E2E
A red CI is never merged on the basis of "the failure is unrelated." Investigate or revert the unrelated thing first.

---

## 7. When something here is wrong

This document is wrong sometimes. The right reaction is:
1. Note it (in the PR description, or as a `// FIXME(coding-standards):` comment with explanation).
2. Propose an edit to this file in the same PR.
3. Implement to the proposed standard, not the current one.

Letting the standards drift silently from practice is worse than imperfect standards.

---

## 8. Pre-flight checklist for any code-writing task

Before opening a PR:
- [ ] `00-overview.md` and the relevant plan doc(s) re-read for the section you're touching.
- [ ] Linters / formatters / type-checkers all clean locally.
- [ ] Tests added for new behavior; existing tests still green.
- [ ] No new dependencies added without justification in the PR description.
- [ ] No secrets, env-specific values, or local paths in the diff.
- [ ] OpenAPI YAML updated if any route or schema changed; codegen re-run.
- [ ] Plan documents updated if you discovered a gap or made a non-obvious decision.
- [ ] PR description follows §6.3.
