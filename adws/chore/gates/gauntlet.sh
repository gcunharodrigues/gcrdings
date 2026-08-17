#!/usr/bin/env bash
# Chore gauntlet — lint + tests only (no typecheck ceremony for chores).
set -e
./scripts/bootstrap-dev.sh && pnpm --dir frontend install --frozen-lockfile && cargo fetch --locked
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && pnpm --dir frontend lint
cargo test --workspace --locked && pnpm --dir frontend exec bun test tests/lib
cargo audit && pnpm --dir frontend audit --audit-level high
