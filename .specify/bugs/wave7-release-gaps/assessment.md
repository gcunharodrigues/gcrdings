# Bug Assessment: Wave 7 release gates do not close

- **Slug**: wave7-release-gaps
- **Created**: 2026-08-17
- **Source**: local T028 reproduction and independent code review
- **Verdict**: valid
- **Severity**: high

## Report

The exact Wave 7 candidate exposed four release blockers:

1. Recovery after a real `SIGKILL` leaves the read-only DMG mounted. The rollback harness restores state, then fails while removing the mounted transaction directory.
2. `scripts/release.sh --dry-run` requires release artifacts but does not pass their paths to the strict gate.
3. `THIRD_PARTY_NOTICES.md` declares `posthog-rs` 0.3.7 while the locked dependency is 0.23.3.
4. The governing plan declares Rust 1.77 while the repository pins Rust 1.97.1.

## Symptom

The interrupted rollback returns a filesystem error instead of `rollback_retry_completed`. The documented dry-run release command returns `exact release artifacts are required`. Legal and planning evidence disagree with the immutable dependency and toolchain inputs.

## Reproduction

1. Start `qualify-local-adhoc-install.sh rollback` with the exact package.
2. Wait until the transaction records the candidate process identity, then send `SIGKILL` to the qualifier.
3. Run `qualify-local-adhoc-install.sh rollback-interrupted`; observe removal of the still-mounted DMG fails.
4. Run `scripts/release.sh --dry-run`; observe the strict gate receives no artifact paths.
5. Compare `THIRD_PARTY_NOTICES.md` with `Cargo.lock`, and compare `specs/002-complete-v1/plan.md` with `rust-toolchain.toml`.

## Suspected Code Paths

- `scripts/qualify-local-adhoc-install.sh:434` — recovery never detaches a transaction mount before removing the journal tree.
- `scripts/release.sh:20` — dry-run sets only `GCRDINGS_REQUIRE_RELEASE_PACKAGE`.
- `THIRD_PARTY_NOTICES.md:105` — stale locked dependency version.
- `specs/002-complete-v1/plan.md:25` — stale toolchain declaration.
- `frontend/tests/lib/v1-release-safety.test.ts` — current contracts do not execute the dry-run binding or prove mounted recovery ordering.
- `frontend/tests/lib/licenses.test.ts` — current notice test checks names, not the resolved version.

## Root Cause Hypothesis

Confidence is high. The rollback cleanup handles mounted images only in the live process trap, not in pending-transaction recovery. The other three findings are direct contract drift: the release wrapper omits required environment bindings, and documentation is not checked against locked inputs.

## Proposed Remediation

**Preferred**: Detach an exact transaction mount after stopping its recorded process and before restoring or removing its journal. Fail closed when detach fails. Bind the dry-run gate to the repository's exact app and DMG paths. Correct the locked dependency notice and Rust toolchain declaration.

**Files likely to change**:

- `scripts/qualify-local-adhoc-install.sh`
- `scripts/release.sh`
- `THIRD_PARTY_NOTICES.md`
- `specs/002-complete-v1/plan.md`
- `frontend/tests/lib/v1-release-safety.test.ts`
- `frontend/tests/lib/licenses.test.ts`

**Tests to add or update**:

- Require pending recovery to detach a mounted transaction before journal removal.
- Execute `release.sh --dry-run` in a synthetic repository whose commands are stubs, and assert exact artifact environment bindings.
- Compare the `posthog-rs` notice version with `Cargo.lock`.
- Re-run a real interrupted rollback against the exact local package.

## Risks & Considerations

- Detach must target only the mount path inside the validated transaction root.
- Recovery must not send a signal when process identity is missing or mismatched.
- The release wrapper must not publish; `--dry-run` remains validation-only.
- A source fix creates a new candidate. The package, review, Security Review, and release evidence must bind the new commit.

## Open Questions

- None.
