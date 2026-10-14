#!/usr/bin/env bash
# Fail unless every Lambda binary exists under backend/target/lambda.
#
# CDK falls back to a shell stub for any binary it can't find
# (infra/opennewsletter/lambda_assets.py), so a deploy without this check could
# ship stubs silently. The binary list is the Makefile's FUNCTION_* map, the one
# place every Lambda binary is already named.
#
# Usage: scripts/check_lambda_binaries.sh   (from anywhere in the repo)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binaries="$(sed -n 's/^FUNCTION_\([a-z-]*\)[[:space:]]*=.*/\1/p' "$repo_root/Makefile")"

if [ -z "$binaries" ]; then
    echo "No FUNCTION_* entries found in the Makefile." >&2
    exit 1
fi

missing=0
for binary in $binaries; do
    path="$repo_root/backend/target/lambda/$binary/bootstrap"
    if [ ! -f "$path" ]; then
        echo "Missing Lambda binary: backend/target/lambda/$binary/bootstrap" >&2
        missing=1
    elif [ ! -x "$path" ]; then
        echo "Lambda binary is not executable: backend/target/lambda/$binary/bootstrap" >&2
        missing=1
    fi
done
[ "$missing" -eq 0 ] || exit 1
echo "All $(echo "$binaries" | wc -w | tr -d ' ') Lambda binaries present."
