# workflows

GitHub Actions:

- `frontend-ci.yml` — build + test on PR
- `frontend-deploy.yml` — push to `gh-pages` on `main`
- `backend-ci.yml` — `cargo test` + `cargo clippy`
- `backend-deploy.yml` — `cargo lambda build` + `cdk deploy`
- `infra-ci.yml` — `cdk synth` + `cdk diff` on PR

Each workflow file starts with a top-level comment describing its trigger and purpose. See [`../../plans/11-testing-ci-cd.md`](../../plans/11-testing-ci-cd.md).
