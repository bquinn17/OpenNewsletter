# Project Metrics

## Lines of Code

### Summary

The OpenNewsletter project contains **64,445 total lines** across production code, infrastructure, and CI/CD:

| Metric | Count |
|--------|-------|
| Total Lines | 64,445 |
| Code (non-comment, non-blank) | 54,051 |
| Comments | 4,903 |
| Blank Lines | 5,491 |

### Breakdown by Language

| Language | Lines | Code | Files | % of Code |
|----------|-------|------|-------|-----------|
| Rust | 32,578 | 29,054 | 157 | 54% |
| TypeScript/TSX | 23,184 | 19,118 | 138 | 35% |
| Python (CDK) | 2,345 | 1,970 | 13 | 4% |
| YAML (CI/CD) | 3,376 | 3,262 | 1 | 6% |
| TOML (Config) | 546 | 463 | 15 | 1% |
| CSS | 190 | 184 | 1 | <1% |
| Markdown (Docs) | 65 | 0 | 15 | — |

### Composition

- **Backend**: 54% (Rust) — Lambda handlers, domain logic, persistence layer
- **Frontend**: 35% (TypeScript/TSX) — React components, UI logic, API client
- **Infrastructure**: 4% (Python CDK) — AWS resource definitions
- **Configuration**: 7% (YAML, TOML, CSS)

## How to Generate

Install tokei (lines-of-code counter) via Cargo:

```bash
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH" && \
cargo install tokei
```

Count production code (excluding tests, docs, build artifacts):

```bash
tokei --exclude "target,tests,node_modules,dist,build,.git,docs,plans,scripts,examples,.github" \
  backend/crates frontend/src infra/opennewsletter shared/openapi.yaml
```

This command:
- Counts only source files in `backend/crates`, `frontend/src`, `infra/opennewsletter`, and the API spec
- Excludes test directories, generated code, dependencies, build output, and documentation
- Reports code, comments, and blank lines separately

Run it from the repository root to refresh these metrics.
