#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--dry-run" ]]; then
  echo "release: publication is disabled; run with --dry-run to validate the local release baseline" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

./scripts/bootstrap-dev.sh
pnpm --dir frontend install --frozen-lockfile
cargo fetch --locked
./scripts/prepare-foundation-helper.sh
test -x ./scripts/verify-release-package.sh

# The strict gate invokes scripts/verify-release-package.sh when these exact
# artifact paths are present; release mode refuses to proceed without them.
GCRDINGS_RELEASE_APP="$repo_root/target/release/bundle/macos/gcrdings.app" \
GCRDINGS_RELEASE_DMG="$repo_root/target/release/bundle/dmg/gcrdings_0.4.0_aarch64.dmg" \
GCRDINGS_REQUIRE_RELEASE_PACKAGE=1 \
  ./scripts/verify-release-gates.sh
echo "release: dry-run passed; signing and publication remain intentionally unconfigured"
