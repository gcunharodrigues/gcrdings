#!/usr/bin/env bash
# Hotfix smoke — the fast subset that proves the bleeding stopped. No full
# gauntlet here by design (Dan: the hotfix lane trades ceremony for speed);
# the postmortem ticket pays the ceremony back later.
set -e
./scripts/bootstrap-dev.sh && pnpm --dir frontend install --frozen-lockfile && cargo fetch --locked
cargo test --workspace --locked --lib && pnpm --dir frontend exec bun test tests/lib
cargo audit && pnpm --dir frontend audit --audit-level high
