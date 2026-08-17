#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
target_triple=$(rustc -vV | awk '/^host:/ {print $2}')
destination_dir="$repo_root/frontend/src-tauri/binaries"
destination="$destination_dir/foundation-helper-$target_triple"
temporary="$destination.tmp"
build_root=${FOUNDATION_HELPER_BUILD_DIR:-"$repo_root/foundation-helper/.build"}
canonical_repo=/workspace/gcrdings
canonical_build=/workspace/swift-build

swift_args=(
    --package-path "$repo_root/foundation-helper"
    --scratch-path "$build_root"
    --disable-automatic-resolution
    -c release
    -Xswiftc -debug-prefix-map
    -Xswiftc "$build_root=$canonical_build"
    -Xswiftc -file-prefix-map
    -Xswiftc "$build_root=$canonical_build"
    -Xswiftc -debug-prefix-map
    -Xswiftc "$repo_root=$canonical_repo"
    -Xswiftc -file-prefix-map
    -Xswiftc "$repo_root=$canonical_repo"
    -Xswiftc -file-compilation-dir
    -Xswiftc "$canonical_repo"
    -Xlinker -oso_prefix
    -Xlinker "$build_root"
)

swift build "${swift_args[@]}"
binary_dir=$(swift build "${swift_args[@]}" --show-bin-path)
mkdir -p "$destination_dir"
cp "$binary_dir/foundation-helper" "$temporary"
chmod 0755 "$temporary"
mv "$temporary" "$destination"
echo "prepare-foundation-helper: ready"
