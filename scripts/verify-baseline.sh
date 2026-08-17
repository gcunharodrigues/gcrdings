#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

bun test frontend/tests/lib/baseline.test.ts \
  frontend/tests/lib/provenance.test.ts \
  frontend/tests/lib/licenses.test.ts

evidence_dir="$repo_root/target/baseline"
mkdir -p "$evidence_dir"
cargo metadata --locked --format-version 1 > "$evidence_dir/cargo-metadata.json"
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend licenses list --prod --json > "$evidence_dir/npm-licenses.json"
node scripts/verify-licenses.mjs \
  "$evidence_dir/cargo-metadata.json" \
  "$evidence_dir/npm-licenses.json" \
  "$evidence_dir/licenses.json"
