# Bug Verification: Wave 7 release gates do not close

- **Slug**: wave7-release-gaps
- **Tested**: 2026-08-17
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

The mounted-DMG recovery now completes after a real qualifier `SIGKILL`. Dry-run artifact bindings and both documentation drifts are covered by executable regressions; no focused regression failed.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction | Kill qualifier after durable process identity and mounted DMG; run `rollback-interrupted` | pass | Returned `rollback_retry_completed` without manual detach. |
| Focused regressions | `bun test frontend/tests/lib/v1-release-safety.test.ts frontend/tests/lib/licenses.test.ts` | pass | 12 passed in 15.21 seconds. |
| Shell syntax | `bash -n scripts/qualify-local-adhoc-install.sh scripts/release.sh` | pass | No syntax error. |
| State restoration | Inspect mount, process, listener, Keychain, artifact, journal, and SQLite state | pass | All cleanup counts returned to zero; real data restored; `quick_check=ok`. |

## Output Excerpts

```text
qualify-local-adhoc-install: rollback_retry_completed
12 pass
0 fail
ok
```

## Residual Risks

- The new source commit requires a new package identity and the normal independent review, Security Review, and Release Gate.
- Public notarization remains intentionally unsatisfied for this local ad-hoc release.

## Recommendation

Close this bug. Build and assure the new immutable Wave 7 candidate; do not reuse the prior package approval.
