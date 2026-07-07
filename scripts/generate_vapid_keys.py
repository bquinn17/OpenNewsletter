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

    vapid = Vapid()
    vapid.generate_keys()
    payload = {
        "publicKey": vapid.public_key_urlsafe_base64().decode("ascii"),
        "privateKey": vapid.private_key_urlsafe_base64().decode("ascii"),
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
