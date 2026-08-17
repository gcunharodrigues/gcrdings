# Bug Verification: T028 evidence binding

- **Slug**: t028-evidence-binding
- **Tested**: 2026-08-17
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

The old symptom no longer reproduces. The validator now blocks a missing or changed evidence file and blocks semantic drift even when the attacker updates the declared file hash.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction | Remove one fixture evidence file and validate | pass | Validator reports the missing `capture_30m` evidence |
| Hash bypass | Replace one receipt evidence SHA-256 | pass | Validator reports on-disk hash mismatch |
| Semantic bypass | Change workload duration or restored data hash and update the receipt hash | pass | Validator reports workload or restoration mismatch |
| Focused regression | `bun test frontend/tests/lib/v1-qualification.test.ts frontend/tests/lib/v1-release-safety.test.ts frontend/tests/lib/baseline.test.ts` | pass | 20 passed, 0 failed |
| Type check | `pnpm --dir frontend exec tsc --noEmit` | pass | No TypeScript error |
| Real receipt CLI | `bun qualification/v1/validate.ts qualification/v1/target-mac.json` | pass | `V1 release receipt valid` |
| Formatting | `git diff --check` | pass | No whitespace error |

## Output Excerpts

```text
20 pass
0 fail
V1 release receipt valid
```

## Residual Risks

- The release package remains local ad-hoc. Public notarization is intentionally unsatisfied.
- Workload evidence uses the recorded change-impact equivalence from runtime candidate `4c76cd6` to release candidate `a067c8e`; the validator proves ancestry and exact evidence identity but does not rerun the workloads.

## Recommendation

Close the bug. Run independent re-review of the new exact integrated candidate, then proceed to the separate Security Review and Release Gate.
