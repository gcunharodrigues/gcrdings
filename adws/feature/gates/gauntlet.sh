#!/usr/bin/env bash
# Deterministic gauntlet: zero tokens, same result every run. A command that
# doesn't apply to this repo was set to `true` at instantiation — a declared
# absence, not silence.
set -e
./scripts/bootstrap-dev.sh && pnpm --dir frontend install --frozen-lockfile && cargo fetch --locked
cargo test --workspace --locked && pnpm --dir frontend exec bun test tests/lib
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && pnpm --dir frontend lint
cargo check --workspace --all-targets --locked && pnpm --dir frontend exec tsc --noEmit && pnpm --dir frontend build
cargo audit && pnpm --dir frontend audit --audit-level high
