# Verified Development Baseline

This file records the commands and measured results for the imported gcrdings baseline on the target
Apple Silicon Mac. Run macOS Rust commands with:

```bash
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
```

The full Xcode application is required by the inherited `cidre` dependency. The generated
Foundation Models helper must also exist before Cargo evaluates the Tauri bundle configuration.

The baseline toolchains are Rust 1.97.1 (`rust-toolchain.toml`), Node.js 22.23.1 (`.node-version`), and
pnpm 10.34.5 (`frontend/package.json`). CI reads the same files and values.

## Clean setup

```bash
./scripts/bootstrap-dev.sh
pnpm --dir frontend install --frozen-lockfile
cargo fetch --locked
./scripts/prepare-foundation-helper.sh
```

`bootstrap-dev.sh` is a pure, idempotent toolchain check. `prepare-foundation-helper.sh` builds the
local Swift worker and atomically copies it to the target-triple filename expected by Tauri.

The ADW gauntlet retains this exact bootstrap line:

```bash
./scripts/bootstrap-dev.sh && pnpm --dir frontend install --frozen-lockfile && cargo fetch --locked
```

## Tests

```bash
cargo test --workspace --locked && pnpm --dir frontend exec bun test tests/lib
```

Verified after installing Xcode and preparing the sidecar. The Rust suite runs 237 library tests
(235 passed, 2 ignored), 2 helper tests, and 2 doc tests. The frontend suite runs 62 tests. A malformed
upstream doc example discovered during verification was corrected so the doc-test phase compiles.
The encoded-origin/mixed-track marker test accepts at most 100 ms of seek drift.

## Typecheck and build

```bash
cargo check --workspace --all-targets --locked && pnpm --dir frontend exec tsc --noEmit && pnpm --dir frontend build
```

The frontend typecheck and production build are verified green. The Cargo check uses the same compiled
workspace and sidecar prerequisites as the test command.

## Lint

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && pnpm --dir frontend lint
```

Verified red on the imported baseline:

- `cargo fmt --all --check` reports formatting differences in inherited Rust files.
- `cargo clippy ... -D warnings` stops at the inherited FFmpeg build-script warnings.
- `pnpm --dir frontend lint` launches Next.js's interactive ESLint setup because Next 14 does not use
  the existing flat `eslint.config.mjs`; it is not currently an unattended lint command.

These are baseline-quality gaps, not hidden or suppressed. A follow-up quality ticket should establish
the formatting and lint baseline without mixing a repository-wide rewrite into the rebrand commit.

## Security audit

```bash
cargo audit && pnpm --dir frontend audit --audit-level high
```

Verified green on 2026-08-17 by the exact Wave 7 gate. The reachable Rust graph has zero vulnerabilities
and seven exact warning reviews that expire on 2026-09-30. The npm production graph has zero high or
critical vulnerabilities. The command has no unreviewed ignore rule.

## Packaging

```bash
pnpm --dir frontend tauri:build
```

Verified on the target Mac through `scripts/build-local-adhoc.sh`. Two clean builds independently passed
the closed package verifier. The generated local package is bound to commit `a067c8e8769597a89075aa0d48c0218bbc8296d1`.
Application executable SHA-256: `75259cc42bacea949ce0fbc07b20ce2cdac4db1f51aa57383a24f9304fe83b5b`.
DMG SHA-256: `14bc47a640123cf5bbe7d32a7f53f8a988ee3a586a5a5772e1b0be553d7b1b70`.
Notarization remains unconfigured and unsatisfied.

## Release

```bash
./scripts/release.sh --dry-run
```

Declared, not run on the final package because it would repeat the strict integrated gate. Its exact
package-path forwarding ran through the executable test seam. `scripts/verify-release-gates.sh` passed
once on 2026-08-17 in about 2 minutes 26 seconds. It covered Rust, Swift, Bun, TypeScript, Next, audits,
and the exact package. No command published, notarized, installed permanently, or configured updates.

## Launch

```bash
pnpm --dir frontend tauri:dev
```

This command requires `scripts/prepare-foundation-helper.sh` first. Verified on the target Mac: the native process
started as `gcrdings`, completed application setup, served the local interface, and remained stable until
it was stopped manually. The inherited updater plugin was also removed from runtime initialization; after
removing its obsolete endpoint and signing key, leaving the plugin active caused startup to fail.

## Receipt: Delivery Wave 1 — 2026-08-14

- Verified source commit: `ab5053d`.
- Clean setup: passed with Node.js 22.23.1, pnpm 10.34.5, Rust 1.97.1, and CMake 4.4.0.
- Automated gates: Rust tests 235 passed/2 ignored; helper tests 2 passed; doc-tests 2 passed; frontend
  tests 62 passed; TypeScript, Next production build, Cargo check, baseline verifier, package, code-sign
  verification, and release dry-run passed. The verifier accepted 841 locked Rust license entries and
  554 frozen production npm license entries and wrote their lockfile-bound report under `target/baseline/`.
- Native launch: the packaged app reached the gcrdings home screen with its navigation and recording
  entry point visible. The process was stopped normally after observation.
- Model integrity: download, discovery, fallback scan, and load validate pinned SHA-256 values. A focused
  test proves that a 74 MiB Whisper file with a valid header and wrong digest is marked corrupted.
- Microphone: not requested or exercised. This does not affect Wave 1. Wave 3 capture UAT requires the
  macOS microphone permission before its microphone-origin cases.
- Recorded Wave 1 red gates: Rust format, strict Clippy, Next lint, Rust audit, and npm audit. These were
  inherited release blockers. Wave 7 closed them before acceptance.
- SHA-256 DMG: `c103ccf68944ab801c66420c0942c6cff4f16c78da537e71ee29ed065a2f4096`.
- SHA-256 app executable: `3568d90c2862312431959eecab62ae14f26c268e68b1c0512a186748440481fc`.
- Historical Wave 1 SHA-256 `llama-helper` (removed in Wave 4): `351a65f9cc8ef2ee67195e4d6450af6fde6d1672bf05e5d31f6c06e501ad3596`.
- SHA-256 FFmpeg: `f990ba09c910ece5129dace707301ae20ae754dd545a20abfd8fafa95da0accc`.

## Known baseline boundaries

- gcrdings V1 is macOS-only; inherited Windows/Linux scripts are not release surfaces.
- The bundle identifier is `com.gcrdings.app`, so no automatic migration from a prior Meetily data
  directory is implied.
- The old updater is removed; no silent update channel remains.
- Analytics has no bundled credential and is disabled unless
  `GCRDINGS_ANALYTICS_API_KEY` is explicitly supplied.
- Functional third-party URLs that still serve FFmpeg or model assets are inventoried in
  `PROVENANCE.md` and `THIRD_PARTY_NOTICES.md`; they are dependencies, not product branding.
- Speaker diarization requires the Swift toolchain supplied by Xcode. Settings installs the pinned
  FluidAudio Core ML model revision through the explicit verified installer. Ordinary processing uses
  only that local installation and never downloads models.

## Receipt: Delivery Wave 2 imported-audio qualification — 2026-08-14

- Verified code candidate: `c0c34ff0d3dd71a461eb15c780671d5e94f423f8`.
- Target: Apple M3 with 24 GiB, macOS 26.5.2 (25F84).
- Selected local pipeline: Silero `26a64600`, Parakeet `parakeet-tdt-0.6b-v3-int8`, FluidAudio 0.14.1
  `d302273d` with model revision `1ed7a662`.
- Passed aggregate thresholds: word error rate 0.264444 <= 0.314444; speaker-count accuracy 1.0 >=
  0.95; real-time factor 0.224177 <= 0.280221; peak memory 1576 MB <= 1970 MB.
- All eight corpus categories and four retry-safe failure cases ran on the target Mac. Network observation
  found zero external requests; diagnostic inspection found zero synthetic transcript matches.
- Every speech case received the expected speaker count. The overlap case also produced an explicit overlap
  assignment.

## Receipt: Delivery Wave 3 captured-timeline qualification — 2026-08-14

- Verified code candidate: `c5f0d11743b9da8d39fff86de14e31f0da2cbc2c`.
- Native microphone-only, system-only, and both-origin captures completed on the target Mac. The accepted
  both-origin Session retained two timed passages, one per origin, in one ambiguity group with an explicit
  `preserved-ambiguous-duplicate` decision.
- Removing the synthetic system origin produced a typed failed job while the existing two-passage principal
  transcript remained intact. Restoring the same bytes and selecting `Try Again` completed with two passages.
- Source SHA-256 values remained unchanged through processing, failure, and retry. Passage seek selected
  11.3 seconds for an 11.31-second source timestamp, a measured error of 10 ms.
- The maximum observed job duration was 19 seconds and the maximum sampled resident memory was 880 MB.
  Network observation found zero established TCP connections; diagnostic inspection found zero Session-data
  matches.
- Full receipt: private release workspace only; it is not part of the public source tree.

## Historical Wave 7 local qualification — 2026-08-17

- Candidate: `a067c8e8769597a89075aa0d48c0218bbc8296d1`; distribution: `local_adhoc`; target: Mac15,12, Apple M3, 24 GiB, macOS 26.5.2 (25F84).
- The closed T028 receipt passed in the private release workspace. This is historical local-build evidence only; it does not authorize public GitHub artifacts or notarization.
- Long capture and import ran at runtime candidate `4c76cd675cf9a78339c20912996bcd957e25dc75`. The final delta changes only release scripts, notices, tests, plan text, and bug reports. It changes no runtime, UI, or audio source. The final candidate therefore reuses those long measurements under explicit change-impact equivalence and reruns the exact package, integrated gate, and install/rollback checks.
- Capture: 1800.011 s, RSS growth 0 MiB, 360 power samples, mean/p95 0, nominal thermal state, zero observed sockets.
- Import: 1800.01 s, RSS growth 487.156 MiB, peak 1618.28 MB, 360 power samples, mean/p95 0, nominal thermal state, zero observed sockets.
- Cold startup: 1415.076 ms. Graceful shutdown: 249.935 ms. Initial 10,000-block render: 756.455 ms. Twenty native keyboard edits: p95 89.243 ms.
- Clean install, existing-data install, snapshot rollback, and killed-installer recovery passed on the exact final package. Each run restored hashes and SQLite integrity and left zero mount, process, listener, temporary Keychain, artifact, or journal delta.
- The real 3.2 GiB application-data tree was moved atomically during install tests, restored, and verified with `PRAGMA quick_check = ok`. No gcrdings application remains installed under `~/Applications`.
- The current delivery boundary is public GitHub source collaboration with locally built DMGs. No package,
  DMG, notarization, or update publication is authorized by this source-only profile.
- Ordinary review, Security Review, and External Assurance passed the local Wave 7 candidate recorded above. Those results predate the public GitHub source candidate and cannot be reused for this publication or any future binary channel.
