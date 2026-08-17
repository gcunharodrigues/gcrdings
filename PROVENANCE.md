# Provenance

gcrdings is a public derivative of **Meetily Community**, imported per [`docs/adr/0001-fork-meetily-community.md`](docs/adr/0001-fork-meetily-community.md).
[`docs/adr/0002-selective-upstream-updates.md`](docs/adr/0002-selective-upstream-updates.md) governs how
future upstream changes are considered: as explicit, independently verified commits, never an automatic
merge. This repository has no GitHub fork relationship with Meetily. This document is the anchor that
workflow reads from.

## Upstream repository

- **Repository:** `https://github.com/Zackriya-Solutions/meetily.git` (tracked locally as the git
  remote `upstream`; product name in that repo is "Meetily").
- **Adopted release:** tag `v0.4.0` ("Meetily Community 0.4.0").
- **Revision (commit SHA): `0281737d87d26352fb0adc78c8c0975f691b23d1`.**
  - Verified with `git ls-remote --tags upstream refs/tags/v0.4.0 refs/tags/v0.4.0^{}`.
  - `v0.4.0` is an annotated tag object at `877225f19787a1a9acf3a3bd585cb40c2df14065`;
    dereferencing it resolves to the commit above.
  - The imported version markers in `frontend/package.json` and
    `frontend/src-tauri/Cargo.toml` both match `0.4.0`.

## Local import

- **Import commit:** `8d2dad8` — "chore: import Meetily Community 0.4.0 baseline".

## Unpinned upstream references and their disposition

### `.gitmodules` — `backend/whisper.cpp` (Zackriya-Solutions/whisper.cpp, branch `develop`)

- **State found:** `.gitmodules` declared a submodule at `backend/whisper.cpp` pointing at
  `https://github.com/Zackriya-Solutions/whisper.cpp` on branch `develop` (a moving branch, not a pinned
  SHA). `git ls-files -s backend/whisper.cpp` returns nothing — there is no tracked gitlink in the index —
  and the `backend/whisper.cpp/` working directory is empty. It is an orphan pointer: present in
  `.gitmodules`, absent from the actual git object graph.
- **Consumers checked:** `backend/build_whisper.sh` and `backend/build_whisper.cmd` (Python backend build
  scripts) `cd` into `whisper.cpp/` and build it from source. Neither script is invoked by
  `adws/feature/gates/gauntlet.sh` or by any Rust/Tauri build step. The Tauri app links Whisper via the
  `whisper-rs` crate (`frontend/src-tauri/Cargo.toml`, features `metal`/`coreml`/`cuda`/`vulkan`/`hipblas`),
  which vendors/builds whisper.cpp itself as part of the crate — it does not read `backend/whisper.cpp/`.
  The Rust workspace (`Cargo.toml` at repo root: member `frontend/src-tauri`) does not
  include the Python backend, so it is outside the gauntlet path entirely.
- **Disposition:** the orphan `.gitmodules` entry is removed. `git submodule update --remote` on a moving
  branch is a supply-chain risk (an unreviewed upstream commit lands in a C++ build with no diff review),
  and since nothing on the gauntlet path consumes it, there is no reason to keep or pin it in this ticket.
  A later ticket that needs to build the Python backend's whisper.cpp server from source should re-add the
  submodule pinned to a specific commit SHA (not `branch = develop`) and `git submodule update --init` it
  at that time. `.gitmodules` has been deleted from the working tree as part of this change.

### `frontend/src-tauri/build/ffmpeg.rs` — pinned local FFmpeg build input

- **State found:** the imported baseline fetched a prebuilt archive from the third-party GitHub release
  `Zackriya-Solutions/ffmpeg-binaries` (`0.0.1`) during the build.
- **Disposition:** production builds no longer use the network or an archive extractor. They require an
  already-local Apple Silicon executable whose SHA-256 is exactly
  `77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a`, verify its regular-file and
  executable identity, verify that it reports FFmpeg, and only then stage it for Tauri. Runtime resolves
  only the packaged sibling and verifies its post-signing SHA-256. Missing or substituted inputs fail
  closed. Origin, GPL status, and the corresponding-source offer are inventoried in
  [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) and [`SOURCE_OFFER.md`](SOURCE_OFFER.md).

### ONNX Runtime — pinned static link input

- **Input:** the local Apple Silicon `libonnxruntime.a` whose SHA-256 is
  `e5c83560aa9e88afa39d9dca9fb5f5a767e28adb5458d1c36fe0357131b6af8b`.
- **Build normalization:** the upstream archive contains an absolute macOS CI home prefix in diagnostic
  strings. Before linking, the local ad-hoc builder copies the verified archive and replaces that
  13-byte prefix with the equal-length `/workspace/ci` prefix. The normalized link input must hash to
  `a2ab3572c0dbf30ec3c0346e8e46a6df7ccf79d0295b0d7810264b0958718a97`; the original archive and the
  completed application are never edited in place. A mismatch fails the build closed.

### FluidAudio — canonical local static link input

- **Source identity:** FluidAudio `0.14.1` at commit
  `d302273d49ef4d8914b27f20d342be482e8810f1`; the local package resolution hashes to
  `059ed24d83649808e1df796b91b4dd3b138fa3ffd2211fcaed2e695526db90bf`, its package manifest hashes to
  `fa801f3889cbd46298c6155e0889d36216d634595c897cacadfb3c38a7ac4b2c`, and the local Swift bridge
  source hashes to `9c6af20960a4f2411f3fa3d6042e89788b0c5c90b9ca0ef08272d9b2739f1348`.
- **Canonical archive:** local ad-hoc release builds require an explicit regular, non-symlink Apple
  Silicon `libFluidAudioLocalBridge.a` input whose SHA-256 is
  `e14173844a6c296995c9e8ca0fea574168ef2204d0c682a828f46f002892ed5d`. The build verifies the hash,
  sole `arm64` architecture, and all five C bridge symbols before copying it into the isolated Cargo
  output directory. Missing or substituted inputs fail closed; a local ad-hoc build never falls back to
  compiling or downloading FluidAudio. Development builds retain the source build.
- **Toolchain identity:** the canonical input was produced locally from the source identity above with
  Apple Swift 6.3.3 (`swiftlang-6.3.3.1.3`, Clang `2100.1.1.101`), Xcode 26.6 build `17F113`, macOS SDK
  26.5, and macOS 26.5.2. Swift 6.3.3's own deterministic checker reports non-deterministic object code
  for this upstream package even with one job and one compiler thread; fixed LLVM RNG seed, disabled
  cross-module optimization, and disabled whole-module optimization do not remove it. The canonical
  archive is therefore an explicit hash-bound release input, like ONNX Runtime, rather than an
  unrepeatable hidden build step. It contains no user-home, temporary-directory, or compiler-cache path.

## Retained upstream screenshot/logo assets

The imported Meetily-branded screenshots and GIFs were removed before public publication because they were
unreferenced and could misrepresent the product. Future screenshots must be captured from gcrdings, checked
for personal data and metadata, and referenced by public documentation before they are committed.

### `frontend/src-tauri/src/parakeet_engine/parakeet_engine.rs` — Parakeet ONNX model revisions

- **State found:** the default v3 conversion used unversioned Meetily-hosted infrastructure. The v2
  conversion used the converter's Hugging Face repository through a moving `main` URL.
- **Disposition:** both conversions now use the converter's Hugging Face repositories at immutable
  revisions: v2 `0bbb45a3365852604aef28b538a8f066f4ccaa85` and v3
  `8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce`. Exact per-file hashes remain the explicit model-installer
  gate in Ticket 5; ordinary V1 distribution does not accept an unverified first-run model download.

### Downloadable Whisper and summary models

- **Whisper:** `ggerganov/whisper.cpp` revision `5359861c739e955e79d9a303bcbc70fb988958b1`.
- **Qwen 3.5 2B:** `unsloth/Qwen3.5-2B-GGUF` revision `f6d5376be1edb4d416d56da11e5397a961aca8ae`.
- **Qwen 3.5 4B:** `unsloth/Qwen3.5-4B-GGUF` revision `e87f176479d0855a907a41277aca2f8ee7a09523`.
- **Gemma 3 4B:** `bartowski/google_gemma-3-4b-it-GGUF` revision `71506238f970075ca85125cd749c28b1b0eee84e`.
- **Gemma 3 1B:** `bartowski/google_gemma-3-1b-it-GGUF` revision `116f76234503685a98f572982177b11d44ec8ff1`.
- **Integrity:** each active download URL names its immutable revision. The adjacent SHA-256 value is verified before the application marks the downloaded model as available.

## Excluded commercial upstream editions

- **Search performed:** a case-insensitive repository scan for commercial-edition names and markers,
  plus a manual review of every hit.
- **Result:** "Pro" and "Enterprise" appear only as marketing prose in the upstream `README.md` (promotional
  copy for a hosted offering, e.g. the `LAUNCH20` coupon block) — there is no Pro/Enterprise source code,
  binary, config, or asset anywhere in the imported tree. The rebrand in this change deletes that
  marketing prose from `README.md` (see the builder's summary); `frontend/tests/lib/licenses.test.ts`
  enforces that no path in the tree matches `*pro*` under a Meetily namespace, guarding this going forward.
