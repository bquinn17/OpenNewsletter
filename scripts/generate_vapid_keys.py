#!/usr/bin/env python3
"""Generate a VAPID keypair for Web Push and print it as JSON.

Per plans/05-auth-flow.md §10 and plans/07-notifications.md: the resulting JSON
({"publicKey": "...", "privateKey": "...", "subject": "mailto:..."}) is stored
in AWS Secrets Manager at `opennewsletter/vapid/{env}`.

Usage:
    python scripts/generate_vapid_keys.py --subject mailto:ops@example.com
        [--out vapid.json]

Requires: `pip install py-vapid`. The keys are uncompressed P-256 EC keys
encoded as URL-safe base64 (unpadded), per RFC 8292.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--subject",
        required=True,
        help="VAPID 'sub' claim, e.g. mailto:ops@opennewsletter.example.com",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Write JSON to this path instead of stdout.",
    )
    args = parser.parse_args()

    try:
        from py_vapid import Vapid
        from py_vapid.utils import b64urlencode, num_to_bytes
    except ImportError:
        print(
            "ERROR: py-vapid not installed. Run: pip install py-vapid",
            file=sys.stderr,
        )
        return 1

    if not args.subject.startswith(("mailto:", "https://")):
        print(
            "ERROR: --subject must start with 'mailto:' or 'https://'",
            file=sys.stderr,
        )
        return 2

    # py-vapid >=1.8 exposes raw cryptography key objects via `.public_key` /
    # `.private_key` rather than the `*_urlsafe_base64()` helpers this script
    # used to call (removed upstream). Encode RFC 8292's wire format
    # ourselves: public key as the uncompressed X9.62 point, private key as
    # the 32-byte big-endian scalar, both base64url without padding — this
    # is exactly what `Vapid01.from_raw`/`from_raw_public` expect back.
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

    vapid = Vapid()
    vapid.generate_keys()

    public_raw = vapid.public_key.public_bytes(
        encoding=Encoding.X962, format=PublicFormat.UncompressedPoint
    )
    private_raw = num_to_bytes(
        vapid.private_key.private_numbers().private_value, pad_to=32
    )

    payload = {
        "publicKey": b64urlencode(public_raw),
        "privateKey": b64urlencode(private_raw),
        "subject": args.subject,
    }

    text = json.dumps(payload, indent=2)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
        print(f"wrote {args.out}", file=sys.stderr)
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
