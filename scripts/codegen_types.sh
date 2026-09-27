#!/usr/bin/env bash
# Generates frontend/src/types/api.ts from shared/openapi.yaml via openapi-typescript.
#
# Run this after any change to shared/openapi.yaml (plans/coding-standards.md §8
# pre-flight: "OpenAPI YAML updated ... codegen re-run"). Never hand-edit the
# generated file (plans/coding-standards.md §1.12).
#
# Usage:
#   scripts/codegen_types.sh
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
spec="${repo_root}/shared/openapi.yaml"
out="${repo_root}/frontend/src/types/api.ts"

mkdir -p "$(dirname "${out}")"

# Major version pinned per plans/coding-standards.md §2.10 ("pin major versions").
npx --yes openapi-typescript@7 "${spec}" --output "${out}"

echo "Generated ${out#"${repo_root}/"} from ${spec#"${repo_root}/"}"
