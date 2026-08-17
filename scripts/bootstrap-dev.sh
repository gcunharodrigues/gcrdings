#!/usr/bin/env bash
# Check-then-report bootstrap: verifies the toolchain adws/feature/gates/gauntlet.sh
# needs before it runs. Never installs anything silently — on anything missing it
# prints the exact command to fix it and exits non-zero. Idempotent: pure checks,
# no state mutation, so running it twice in a row yields the same result.
set -euo pipefail

fail() {
    echo "bootstrap-dev: $1" >&2
    echo "  install with: $2" >&2
    exit 1
}

require_cmd() {
    local cmd="$1" install_hint="$2"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        fail "'$cmd' not found on PATH" "$install_hint"
    fi
}

require_cmd node "https://nodejs.org (or your system package manager)"
require_cmd pnpm "npm install -g pnpm"
require_cmd cargo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
require_cmd rustc "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
require_cmd cmake "brew install cmake (macOS) / apt-get install cmake (Linux)"

expected_node=$(tr -d '[:space:]' < .node-version)
actual_node=$(node --version)
actual_node=${actual_node#v}
if [[ "$actual_node" != "$expected_node" ]]; then
    fail "node $actual_node does not match required $expected_node" "install Node $expected_node from https://nodejs.org"
fi

expected_pnpm=$(node -p "require('./frontend/package.json').packageManager.split('@').at(-1)")
actual_pnpm=$(pnpm --version)
if [[ "$actual_pnpm" != "$expected_pnpm" ]]; then
    fail "pnpm $actual_pnpm does not match required $expected_pnpm" "npm install -g pnpm@$expected_pnpm"
fi

expected_rust=$(awk -F'"' '/^channel = / { print $2 }' rust-toolchain.toml)
actual_rust=$(rustc --version | awk '{print $2}')
if [[ "$actual_rust" != "$expected_rust" ]]; then
    fail "rustc $actual_rust does not match required $expected_rust" "rustup toolchain install $expected_rust"
fi

# The `cidre` crate (macOS AVFoundation bindings, used by audio capture) runs
# `xcodebuild` from its build script, which needs the full Xcode.app, not just
# the Command Line Tools — a bare CLT install fails with an unhelpful cidre
# build-script panic instead of a clear message.
if [[ "$(uname)" == "Darwin" ]]; then
    developer_dir=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
    if ! DEVELOPER_DIR="$developer_dir" xcodebuild -version >/dev/null 2>&1; then
        fail "full Xcode.app not active (only Command Line Tools found, or Xcode is missing)" \
             "install Xcode from the App Store, then export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer"
    fi
fi

# bun is required by the gauntlet's `pnpm --dir frontend exec bun test tests/lib`
# but is not a frontend devDependency — verify it actually resolves through pnpm.
if ! pnpm --dir frontend exec bun --version >/dev/null 2>&1; then
    fail "'bun' does not resolve via 'pnpm --dir frontend exec bun'" \
         "pnpm --dir frontend add -D bun (or install it globally: curl -fsSL https://bun.sh/install | bash)"
fi

# cargo-audit is a cargo subcommand, not a standalone binary on PATH.
if ! cargo audit --version >/dev/null 2>&1; then
    fail "'cargo audit' not available" "cargo install cargo-audit --locked"
fi

echo "bootstrap-dev: all required tools present (node $actual_node, pnpm $actual_pnpm, rustc $actual_rust, cmake $(cmake --version | head -1 | awk '{print $3}'))"
