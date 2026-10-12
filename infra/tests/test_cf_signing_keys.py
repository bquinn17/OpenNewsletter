"""CloudFront signing keys: one RSA-2048 public key per env (`05-auth-flow.md` §10).

`lambda-media` signs with RSA-SHA1 only, so an EC key would deploy cleanly and
then fail every `GET /media-cookie`. These tests catch that at synth time.

Run with: pytest infra/tests/
"""

from __future__ import annotations

import base64
import sys
from pathlib import Path

# Ensure the infra package is importable regardless of working directory.
sys.path.insert(0, str(Path(__file__).parent.parent))

import aws_cdk as cdk
import pytest
from aws_cdk import assertions

from opennewsletter.config import load_config
from opennewsletter.data_stack import DataStack
from opennewsletter.frontend_stack import FrontendStack
from opennewsletter.media_persistent_stack import MediaPersistentStack

AWS_ENV = cdk.Environment(region="us-east-1")

# DER for the rsaEncryption algorithm identifier (OID 1.2.840.113549.1.1.1).
_RSA_ENCRYPTION_OID = bytes.fromhex("06092a864886f70d010101")
# SubjectPublicKeyInfo size for a 2048-bit modulus with e = 65537.
_RSA_2048_SPKI_LEN = 294


def _encoded_key(env: str) -> str:
    config = load_config(env)
    app = cdk.App()
    data_stack = DataStack(app, f"KeyDataStack-{env}", config=config, env=AWS_ENV)
    frontend_stack = FrontendStack(
        app, f"KeyFrontendStack-{env}", config=config, env=AWS_ENV
    )
    stack = MediaPersistentStack(
        app,
        f"KeyMediaPersistentStack-{env}",
        config=config,
        table=data_stack.table,
        certificate=frontend_stack.certificate,
        env=AWS_ENV,
    )
    (key,) = (
        assertions.Template.from_stack(stack)
        .find_resources("AWS::CloudFront::PublicKey")
        .values()
    )
    encoded: str = key["Properties"]["PublicKeyConfig"]["EncodedKey"]
    return encoded


def _spki_der(pem: str) -> bytes:
    body = "".join(
        line for line in pem.strip().splitlines() if not line.startswith("-----")
    )
    return base64.b64decode(body)


@pytest.mark.parametrize("env", ["dev", "prod"])
def test_signing_key_is_rsa_2048(env: str) -> None:
    pem = _encoded_key(env)
    assert pem.startswith("-----BEGIN PUBLIC KEY-----")
    der = _spki_der(pem)
    assert _RSA_ENCRYPTION_OID in der, f"{env} CloudFront key is not RSA"
    assert len(der) == _RSA_2048_SPKI_LEN, f"{env} CloudFront key is not RSA-2048"


def test_dev_and_prod_use_different_signing_keys() -> None:
    assert _encoded_key("dev") != _encoded_key("prod")
