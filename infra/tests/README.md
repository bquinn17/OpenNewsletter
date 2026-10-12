# tests

CDK stack tests using `aws_cdk.assertions.Template`. One `test_<stack>.py` per stack. See [`../../plans/11-testing-ci-cd.md`](../../plans/11-testing-ci-cd.md) §3.

Also:

- `test_config.py` covers `opennewsletter/config.py`'s `load_config`.
- `test_bootstrap_admin.py` unit-tests `../../scripts/bootstrap_admin.py`'s DynamoDB item builders (prod mode, `05-auth-flow.md` §9.1a). It loads the script by path; no AWS calls.
