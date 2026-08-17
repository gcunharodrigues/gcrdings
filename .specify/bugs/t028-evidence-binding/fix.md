# Bug Fix: Bind T028 evidence to the release receipt

- **Slug**: t028-evidence-binding
- **Fixed**: 2026-08-17
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

The release contract now persists and hashes all measured T028 evidence. The validator checks the evidence commit, workload metrics, package identity, installation result, restoration, SQLite integrity, recovery, and cleanup.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `qualification/v1/evidence/*.json` | added | Exact sanitized workload, package, install, and rollback evidence |
| `qualification/v1/cases.json` | modified | Fixed release/runtime candidates and seven evidence declarations |
| `qualification/v1/target-mac.json` | modified | Added evidence hashes and evidence commit; corrected capture metrics from the persisted file |
| `qualification/v1/validate.ts` | modified | Added closed hash, commit, workload, package, install, restore, and cleanup validation |
| `frontend/tests/lib/v1-qualification.test.ts` | modified | Added missing, forged-hash, metric-drift, and restore-drift regressions |
| `docs/BASELINE.md`, `specs/002-complete-v1/quickstart.md` | modified | Corrected capture duration and power sample count |

## Tests Added or Updated

- `binds persisted T028 evidence by hash and semantics` — rejects missing evidence, forged hashes, changed workload metrics, and changed rollback restoration.
- The complete receipt fixture now uses the fixed release candidate and semantically consistent synthetic evidence.

## Local Verification

- `bun test frontend/tests/lib/v1-qualification.test.ts` → 8 passed, 0 failed.
- `pnpm --dir frontend exec tsc --noEmit` → passed.
- `bun qualification/v1/validate.ts qualification/v1/target-mac.json` → `V1 release receipt valid`.
- Exact source-to-persisted evidence byte comparisons → passed.
- Evidence privacy scan → zero matches.

## Deviations from Assessment

- Persisted the exact release package manifest as a seventh evidence file. This is required to bind the install manifests to the executable and DMG hashes in the receipt.

## Follow-ups

- Run the official bug verification and independent re-review. Do not repeat builds or 30-minute workloads.
