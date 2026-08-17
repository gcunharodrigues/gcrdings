# Quickstart: Complete Audio-First V1 Validation

## Prerequisites

- Target Apple Silicon Mac with the pinned Xcode, Rust, Node.js, pnpm, and Bun versions.
- Clean isolated worktree.
- Private corpus root outside Git, generated from the versioned manifest.
- Microphone and system-audio permissions for the native capture cases.
- Apple Intelligence state recorded as available and unavailable in separate generation cases.
- Provider integration credentials only for a separately authorized final integration case.

## Baseline gates

```bash
./scripts/bootstrap-dev.sh
pnpm --dir frontend install --frozen-lockfile
cargo fetch --locked
./scripts/prepare-foundation-helper.sh
cargo test --workspace --locked
pnpm --dir frontend exec tsc --noEmit
pnpm --dir frontend build
bun test frontend/tests/lib
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
./scripts/release.sh --dry-run
pnpm --dir frontend tauri:build
```

Each Wave adds its focused command and preserves the relevant baseline gates. The integrated candidate runs the complete list once.

## Wave acceptance seams

1. **Baseline**: clean setup, build, tests, lint, package, launch, provenance, license/model manifest, and dry-run release.
2. **Imported transcript**: generate corpus, run local qualification with network observation, validate blocks and durable retry states, and record thresholds/results.
3. **Captured timeline**: record microphone, system, and both origins; process independently; verify provenance, ambiguity, immutable source hashes, ordered timeline, and seek tolerance.
4. **Local findings**: run Meeting, Interview, and Content; validate schemas and evidence; force unavailable, malformed, stale-revision, cancellation, and helper-death cases; observe zero network fallback.
5. **Agent Handoff**: save and share both formats; compare semantic facts; test cancellation, unwritable destination, stale prior artifacts, and temporary cleanup.
6. **External provider**: test Keychain lifecycle and migration; confirm exact preview; prove cancel sends no socket; run failure/retry without fallback; scan DB, export, UI, and logs for secrets.
7. **Release**: execute the full normal/boundary/failure/concurrency table; measure resource and long-duration behavior; package, install, run without Terminal, accept the workflow, and verify rollback.

## Distribution boundary

- Source intent: private GitHub collaboration at `gcunharodrigues/gcrdings` after separate authorization.
- Artifact intent: local only. Do not upload or share the DMG in this feature.
- Collaborator build: clean checkout of the exact commit, locked dependencies, pinned toolchain inputs, closed package verification, and a local receipt with observed hashes.
- Package comparison: verify each build independently. Do not require byte-identical executables across builds.

## Integration evidence

Record task commits and focused results in `tasks.md`. Record native observations and release receipts in this file under dated append-only receipt headings. Do not paste private Session content.

### Delivery Wave 1 receipt — 2026-08-14

- Source commit: `ab5053d`.
- Passed: clean setup, 235/237 Rust library tests with 2 ignored, 2 helper tests, 2 doc-tests, 62 frontend
  tests, TypeScript, Next build, Cargo check, baseline verifier, Tauri package, code-sign verification,
  native launch, and release dry-run.
- Package SHA-256: `c103ccf68944ab801c66420c0942c6cff4f16c78da537e71ee29ed065a2f4096`.
- Recorded red: Rust format, strict Clippy, Next lint, Rust audit, and npm audit. Security remediation is
  required before the Wave 7 release gate can pass.
- Native launch did not require microphone access. Wave 3 requires that permission for microphone-origin
  capture acceptance.

### Delivery Wave 2 imported-audio receipt — 2026-08-14

- Code candidate: `c0c34ff0d3dd71a461eb15c780671d5e94f423f8`.
- Native corpus: eight synthetic categories completed on the Apple M3 target; the missing-audio,
  malformed-output, model-failure, and interrupted-run seams preserved retryable state.
- Aggregate quality passed: WER 0.264444, speaker-count accuracy 1.0, real-time factor 0.224177,
  and 1576 MB peak memory.
- Every speech case received the expected speaker count; the overlap case recorded explicit overlap.
- Safety observation: zero external requests and zero synthetic transcript matches in operational diagnostics.
- Full receipt: private qualification workspace only; it is not published with source.

### Delivery Wave 3 captured-timeline receipt — 2026-08-14

- Code candidate: `c5f0d11743b9da8d39fff86de14e31f0da2cbc2c`.
- Native microphone-only, system-only, both-origin, silence, overlap, ambiguity, source-loss, retry, and seek
  cases passed on the Apple M3 target.
- The accepted both-origin Session retained one microphone passage and one system-audio passage with one
  explicit ambiguity group and decision. A punctuation-only transcription difference did not erase the
  ambiguity marker.
- Source-loss preserved the existing two-passage principal transcript. `Try Again` completed after the exact
  source bytes were restored. All source hashes remained unchanged.
- Measured seek error was 10 ms against the fixed 100 ms limit. Network and diagnostic Session-data matches
  were zero.
- Full receipt: private qualification workspace only; it is not published with source.

### Delivery Wave 4 local-findings receipt — 2026-08-14

- Code candidate: `b3b2f82919c3747b71b698451f41eb3f86b57fcf`.
- Native Meeting, Interview, and Content generation completed on the Apple M3 target. All grounded evidence
  resolved to the exact principal block and playable timestamp. The native evidence action sought to 11.3 seconds.
- A principal-transcript edit made the result stale. Restoring the text did not reuse the old revision.
- Unsupported locale and context-limit cases returned typed native results. Malformed input, invalid evidence,
  cancellation, helper death, model unavailability, and the Apple Intelligence disabled framework mapping
  returned typed local states without changing the principal transcript.
- Follow-up adversarial checks rejected forged receipts, oversized or unknown helper output, private diagnostic
  strings, a non-reading helper, an interrupted generation, and cancellation during registration.
- Peak helper memory was 21 MB. Maximum observed generation duration was 14 seconds. Generation produced zero
  external requests and zero private-content matches in operational diagnostics.
- Full receipt: private qualification workspace only; it is not published with source.

### Delivery Wave 5 Agent Handoff receipt — 2026-08-14

- Code candidate: `611f1dd567c3b2694c665eb6b36d76f54588e587`.
- Native Finder save replaced a stale Markdown artifact and saved JSON; native macOS share popovers opened for both formats on Apple Silicon with macOS 26.5.2.
- The Markdown fenced JSON and standalone JSON were semantically identical at principal revision 6, with two participants, two complete corrected transcript blocks, and the completed generation whose SQLite input revision was also 6.
- Save cancellation and the unwritable `/System` destination returned the exact typed UI outcomes, left no output under `/System`, and preserved the canonical Session database digest.
- Content scans found zero forbidden fields and zero stale text. The run observed zero TCP sockets and zero temporary artifacts, and deep strict code-signature verification passed.
- Full receipt: private qualification workspace only; it is not published with source.

### Delivery Wave 6 provider-authorization receipt — 2026-08-15

- Code candidate: `069d1079294eca80b3659183f3e0aa421b1c51a6`.
- An isolated temporary macOS Keychain accepted synthetic save, test, replacement, task disable/enable, persisted Settings rehydration, and removal. A locked-Keychain first launch preserved both database copies and its durable reservation; the same-source retry migrated the secret after unlock, scrubbed the active and retained SQLite bytes, and removed the reservation. Deterministic exact-candidate gates also rejected a different source, retained legacy state passed to fresh initialization, and a broken active symlink without writes.
- Cancelling the exact preview closed the native modal and produced zero provider requests. A fresh confirmation produced one request; the canonical payload contained only `generation`, `participants`, `schema_version`, `session`, and `transcript`, with zero video, media, private-path, fallback, or substitution fields.
- The forced provider failure produced one typed failure and no automatic retry. Only `Review fresh preview` plus a new one-use confirmation produced the successful retry. A separately accepted POST with a dropped response entered `outcome_unknown`, hid the retry action, and blocked another request; all four observed POSTs used distinct durable idempotency keys.
- Final DB, export, UI, and log scans found zero complete synthetic credentials. Replacement removed generation 1 before generation 2 became current, removal cleared both generations, and the loopback listener and exact app process were stopped after qualification. The temporary Keychain was unregistered, the login Keychain plus System search list was restored exactly, and the isolated state was moved recoverably to Trash.
- The application SHA-256 was `20f3262a62762551af1438a4eddba9ecd446cacd128f7574c4043e5b53449fc2`; the installer SHA-256 was `ee03bd28a770b82dbfdc71b1c0022e269829a592a8fd742d7e295b451ab466db`; the embedded marker matched the candidate and deep strict code-signature verification passed on Mac15,12 / Apple M3 / 24 GiB / arm64 / macOS 26.5.2 (25F84).
- Full receipt: private qualification workspace only; it is not published with source.

### Delivery Wave 7 qualification record — 2026-08-17

- Exact package candidate: `a067c8e8769597a89075aa0d48c0218bbc8296d1`.
- Exact package hashes: application executable `75259cc42bacea949ce0fbc07b20ce2cdac4db1f51aa57383a24f9304fe83b5b`; DMG `14bc47a640123cf5bbe7d32a7f53f8a988ee3a586a5a5772e1b0be553d7b1b70`.
- Two clean builds independently passed fixed-input and closed-package verification. The strict integrated gate passed once. Four exact-package install and rollback modes passed, including recovery after a forced installer kill.
- Long capture/import metrics from runtime candidate `4c76cd675cf9a78339c20912996bcd957e25dc75` apply through explicit change-impact equivalence. The delta to the final candidate changes no runtime, UI, or audio source. Exact package, gate, launch, install, and rollback checks were rerun after the release-only fixes.
- Performance: capture 1800.011 s and 0 MiB RSS growth; import 1800.01 s, 487.156 MiB RSS growth, and 1618.28 MB peak; startup 1415.076 ms; shutdown 249.935 ms; 10,000-block render 756.455 ms; keyboard-edit p95 89.243 ms.
- Full receipt: private release workspace only; it is not published with source.
- Distribution remains `local_adhoc`. Public notarization remains unsatisfied. No publication is authorized.
- Ordinary review and separate Security Review passed source candidate `1030c68ef576e6a8fc379106e07102ade1d1f373`.
- External Assurance candidate `37385ab4d62a065f2848117edde66426601321cc086ec855caf315d12f8787c1` passed all four selected controls with no accepted risk.
- The operator accepted the V1 and no-Terminal workflow on 2026-08-17. Governance commit `f94e82589af28aa604f03ce4e18f1317a08dceb5` closed B-86 with 64/64 criteria.
