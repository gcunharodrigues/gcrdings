# Bug Fix: Wave 7 release gates do not close

- **Slug**: wave7-release-gaps
- **Fixed**: 2026-08-17
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Pending rollback recovery now detaches its validated transaction mount before journal removal. The release dry-run binds the exact local app and DMG, and locked dependency and toolchain documentation now agree with repository inputs.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `scripts/qualify-local-adhoc-install.sh` | modified | Detach exact pending transaction mount; fail closed on invalid path or detach failure. |
| `scripts/release.sh` | modified | Pass exact app and DMG paths to the strict dry-run gate. |
| `THIRD_PARTY_NOTICES.md` | modified | Record locked `posthog-rs` 0.23.3. |
| `qualification/v1/package-allowlist.json` | modified | Bind the corrected notice SHA-256. |
| `specs/002-complete-v1/plan.md` | modified | Record pinned Rust 1.97.1. |
| `frontend/tests/lib/v1-release-safety.test.ts` | modified | Add dry-run execution, toolchain drift, and recovery-order regressions; set the existing slow gate's explicit timeout. |
| `frontend/tests/lib/licenses.test.ts` | modified | Compare the notice with the resolved `Cargo.lock` version. |

## Tests Added or Updated

- `release dry-run binds the exact local package paths` — executes the wrapper in an isolated synthetic repository.
- `governing plan matches the pinned Rust toolchain` — derives the expected version from `rust-toolchain.toml`.
- `T035 requires deterministic local_adhoc install and data-snapshot rollback` — requires detach before journal removal.
- `THIRD_PARTY_NOTICES.md names the locked posthog-rs version` — derives the expected version from `Cargo.lock`.

## Local Verification

- `bun test frontend/tests/lib/v1-release-safety.test.ts frontend/tests/lib/licenses.test.ts` → 12 passed in 15.21 seconds.
- Real target-Mac reproduction → qualifier killed after durable process identity and mounted DMG; patched `rollback-interrupted` returned `rollback_retry_completed`.
- Recovery cleanup → zero mounts, candidate processes, helpers, listeners, temporary Keychains, temporary artifacts, and pending journals.
- User state → exact real application data restored; SQLite `PRAGMA quick_check` returned `ok`.

## Deviations from Assessment

- Updated `qualification/v1/package-allowlist.json` because the closed package contract hashes the corrected notice.
- Added an explicit 30-second timeout to the pre-existing T034 test because its two audit-policy subprocesses exceeded Bun's implicit 5-second timeout during the RED run.

## Follow-ups

- Build and gate a new immutable package because the fix changes tracked release inputs.
