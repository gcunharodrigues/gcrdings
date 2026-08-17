# Bug Verification: Bound long-import transcription memory

- **Slug**: long-import-rss
- **Tested**: 2026-08-16T23:39:44-03:00
- **Assessment**: ./assessment.md
- **Fix**: ./fix.md
- **Result**: verified

## Summary

The original 46:27 native import no longer exceeds either release memory limit. The exact packaged candidate completed the import and remained stable for the full 30-minute observation window.

## Checks Performed

| Check | Command / Action | Result | Notes |
|-------|------------------|--------|-------|
| Reproduction (post-fix) | Launch packaged candidate `4c76cd675cf9a78339c20912996bcd957e25dc75`, import the preserved 46:27 `system-audio.mp4`, and sample every 5 seconds for 30 minutes | pass | 487.156 MiB end-to-start RSS growth and 1618.28 MB peak; limits are 512 MiB and 1970 MB. |
| New / updated tests | `cargo test -p gcrdings audio::decoder::tests --lib --locked` | pass | 18 decoder tests passed. |
| Changed module tests | `cargo test -p gcrdings audio::retranscription --lib --locked` | pass | 35 retranscription tests passed. |
| Formatting | `cargo fmt --all --check` | pass | No formatting drift. |

## Output Excerpts

```text
duration_seconds=1800.01 sample_count=360 power_sample_count=360
rss_start_kib=158080 rss_end_kib=656928 rss_growth_mib=487.156
rss_peak_kib=1580352 rss_peak_mb=1618.28
mean_power=0.0 p95_power=0.0 thermal_states=[nominal]
maximum_socket_count=0
```

```text
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 295 filtered out
```

## Residual Risks

- The broader immutable-candidate release suite, independent code review, Security Review, and Release Gate remain separate release gates.
- This measurement covers the declared Apple M3 target and the preserved synthetic 46:27 input. Other hardware is outside the V1 release boundary.

## Recommendation

Close this bug. The native reproduction passed end-to-end at the exact packaged candidate. Continue T028 without changing the candidate source.
