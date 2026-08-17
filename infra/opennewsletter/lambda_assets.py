"""Locating `cargo lambda build` output for CDK assets."""

from __future__ import annotations

from pathlib import Path

from aws_cdk import aws_lambda

_REPO_ROOT = Path(__file__).parent.parent.parent
_LAMBDA_BUILD_ROOT = _REPO_ROOT / "backend" / "target" / "lambda"
_STUB_PATH = _REPO_ROOT / "backend" / "lambda-stubs" / "lambda-image-process"


def lambda_code(binary_name: str) -> aws_lambda.Code:
    """Point at `cargo lambda build --release --arm64` output for `binary_name`.

    Synth (and therefore `infra-ci`) has to work on a machine that has never run the
    Rust build, so a missing artifact falls back to the shell stub rather than
    failing. Deploying that stub would be a bug, which is why `make deploy-dev`
    builds the binaries first.
    """
    built = _LAMBDA_BUILD_ROOT / binary_name
    if (built / "bootstrap").is_file():
        return aws_lambda.Code.from_asset(str(built))
    return aws_lambda.Code.from_asset(str(_STUB_PATH))
