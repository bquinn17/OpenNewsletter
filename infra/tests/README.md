# tests

CDK stack tests using `aws_cdk.assertions.Template`. One `test_<stack>.py` per stack. See [`docs/development/testing-and-ci.md` §3](../../docs/development/testing-and-ci.md#3-cdk-tests).

Also:

- `test_config.py` covers `opennewsletter/config.py`'s `load_config`.
- `test_bootstrap_admin.py` unit-tests `../../scripts/bootstrap_admin.py`'s DynamoDB item builders (prod mode, [`auth.md` §9.1a](../../docs/design/auth.md#91a-scriptsbootstrap_adminpy--prod-d5)). It loads the script by path; no AWS calls.
