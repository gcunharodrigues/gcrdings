# Third-Party Notices

gcrdings is built on the MIT-licensed Meetily Community codebase (see [`PROVENANCE.md`](PROVENANCE.md))
and bundles or downloads the third-party code, binaries, and models listed below.

**Methodology note:** this baseline inventory was compiled from `Cargo.toml`/`Cargo.lock`,
`frontend/package.json`/`pnpm-lock.yaml`, vendored notices, and every model-download call site. It records
all code and model families that the adopted V1 baseline bundles or can download.
`scripts/verify-baseline.sh` resolves the locked Rust graph and generates the production npm license
report. Release builds must rerun it because dependency resolution changes over time.

## Vendored source

### cpp-httplib (`backend/whisper-custom/server/httplib.h`)

- **License**: MIT
- **Copyright**: Copyright (c) 2023 Yuji Hirose. All rights reserved.
- **Upstream**: https://github.com/yhirose/cpp-httplib
- Header notice, quoted verbatim from the vendored file:
  ```
  //
  //  httplib.h
  //
  //  Copyright (c) 2023 Yuji Hirose. All rights reserved.
  //  MIT License
  //
  ```
- **Scope note**: this file lives under the archived Python backend (`backend/`), which is not part of
  the Rust workspace or the gauntlet build path (see `PROVENANCE.md`).

## Pinned third-party binary supplied as a local build input

### FFmpeg (build-time bundled sidecar)

- `frontend/src-tauri/build/ffmpeg.rs` accepts only a local executable originating from the immutable
  `Zackriya-Solutions/ffmpeg-binaries` release `0.0.1`; the production build performs no download. Tauri
  bundles it as the `externalBin` sidecar `binaries/ffmpeg`.
- **License**: GPL-2.0-or-later. The pinned Apple Silicon binary reports `--enable-gpl` in
  `ffmpeg -buildconf` and includes GPL components such as `libx264`/`libx265`; it is therefore not the
  LGPL-only FFmpeg variant. Redistribution must include the corresponding GPL notices and source-offer
  obligations.
- **Integrity**: the Apple Silicon binary is accepted only when its SHA-256 equals
  `77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a`; the build verifies this before
  executing or bundling it. Runtime accepts only the hash-pinned packaged sibling. Other targets are
  intentionally unapproved in the macOS-only V1 baseline.
- **Corresponding source**: the concrete three-year offer and provenance limitations accompany every
  package in [`SOURCE_OFFER.md`](SOURCE_OFFER.md).

## Models

### Whisper GGML models (ggerganov/whisper.cpp on Hugging Face)

- `frontend/src-tauri/src/whisper_engine/whisper_engine.rs` downloads GGML model weights (e.g.
  `ggml-tiny.bin` through `ggml-large-v3.bin`, plus `q5_0`/`q5_1` quantized variants) from
  `ggerganov/whisper.cpp` revision `5359861c739e955e79d9a303bcbc70fb988958b1`. The application
  verifies the selected file's SHA-256 before installation.
- **License**: MIT (same license as `whisper.cpp`; the underlying Whisper model weights are released by
  OpenAI under MIT).
- **Origin**: Hugging Face, `ggerganov/whisper.cpp` repository.

### Parakeet ONNX models

- `frontend/src-tauri/src/parakeet_engine/parakeet_engine.rs` downloads:
  - v2 quantization: `https://huggingface.co/istupakov/parakeet-tdt-0.6b-v2-onnx/resolve/0bbb45a3365852604aef28b538a8f066f4ccaa85/`
    (ONNX conversion by [istupakov](https://huggingface.co/istupakov)).
  - v3 (default) quantization: `https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx/resolve/8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce/`.
- **License**: both pinned ONNX model repositories declare CC-BY-4.0. Ticket 5 adds per-file SHA-256
  verification before model installation.

### FluidAudio diarization models

- gcrdings installs the Core ML speaker diarization files only after an explicit user action. The
  installer pins `FluidInference/speaker-diarization-coreml` revision
  `1ed7a662fdc7109e36d822db793ee6eebdaf8594` and verifies every file against
  `qualification/audio-corpus/model-manifest.json` before activation.
- Ordinary processing uses the local-only bridge in `frontend/src-tauri/vendor/fluidaudio-local`.
  That bridge pins FluidAudio `0.14.1` at `d302273d49ef4d8914b27f20d342be482e8810f1` and has no
  model-download path.
- **Model attribution and license**: Fluid Inference, [CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/). The repository identifies
  `pyannote/speaker-diarization-community-1` as its parent model. FluidAudio itself remains
  Apache-2.0; the Rust bridge remains MIT.
- **Origin**: https://huggingface.co/FluidInference/speaker-diarization-coreml

## Rust crates (direct dependencies)

From `frontend/src-tauri/Cargo.toml` and the root `Cargo.toml` workspace.
`scripts/verify-baseline.sh` resolves these packages with `cargo metadata --locked`, rejects missing or
unknown licenses, and records the normalized inventory with the `Cargo.lock` SHA-256.

Common permissive licenses (MIT or dual MIT/Apache-2.0), per standard crates.io metadata: `serde`,
`serde_json`, `anyhow`, `once_cell`, `uuid`, `cpal`, `clap`, `chrono`, `log`, `env_logger`, `tracing`,
`which` (build-only), `bytemuck`, `futures-util`, `thiserror`, `tokio`, `tokio-util`, `async-trait`, `reqwest`,
`crossbeam`, `dashmap`, `dirs`, `url`, `sysinfo`, `lazy_static`, `regex`, `ndarray`, `bytes`, `rand`,
`rayon`, `tempfile`, `tauri`, `tauri-build`, and the `tauri-plugin-*` family
(`fs`, `dialog`, `store`, `notification`, `single-instance`, `log`), `objc`,
`core-graphics`, `time`, `dasp`, `futures-channel`, `sqlx`, `criterion`, `tracing-subscriber`.

Named separately (non-standard or notable licensing/provenance):

- **Local FluidAudio Rust/Swift bridge** (`frontend/src-tauri/vendor/fluidaudio-local`) — MIT;
  gcrdings-owned, local-only replacement for the upstream `fluidaudio-rs` API used by diarization.
  It builds the Apache-2.0 FluidAudio SDK at tag `0.14.1` and commit
  `d302273d49ef4d8914b27f20d342be482e8810f1` through Swift Package Manager.
- **FluidAudio** (0.14.1) — Apache-2.0; local Core ML speaker diarization SDK for Apple platforms.
- **`posthog-rs`** (0.23.3) — MIT. PostHog's official Rust client; see `PROVENANCE.md`/`AC7` for how
  gcrdings uses it (no default credential).
- **`whisper-rs`** (0.13.2) — MIT/Apache-2.0 dual; binds `whisper.cpp` (MIT, © ggerganov).
- **`ort`** (2.0.0-rc.10) — MIT/Apache-2.0 dual; ONNX Runtime bindings (Microsoft ONNX Runtime is
  MIT-licensed).
- **`silero_rs`** (package `silero`) — fetched from `https://github.com/emotechlab/silero-rs`, pinned to
  rev `26a6460`.
- **`cidre`** — fetched from `https://github.com/yury/cidre`, pinned to rev `a9587fa` (macOS-only).
- **`cpal`** (0.15.3) — crates.io source pinned by the checksum in `Cargo.lock`.
- **`esaxx-rs`** (0.1.10) — crates.io source pinned by the checksum in `Cargo.lock`.
- **`nnnoiseless`** — MIT/Apache-2.0; Rust port of Xiph's RNNoise (BSD-3-Clause upstream C library).
- **`ebur128`** — MIT; implements the EBU R128 loudness standard.
- **`symphonia`** — MPL-2.0 (notably copyleft-per-file, unlike the mostly MIT/Apache tree around it).
All build-path Git dependencies use exact revisions. `scripts/verify-baseline.sh` rejects a dependency
that combines `git` and `branch` in the project manifests.

## npm packages (direct dependencies)

From `frontend/package.json`. `scripts/verify-baseline.sh` generates the production dependency license
report from the frozen `frontend/pnpm-lock.yaml`; this direct-dependency summary remains human-readable.

MIT-licensed (per standard npm-ecosystem convention): `@heroicons/react`, `@hookform/resolvers`, all
`@radix-ui/react-*` packages, `@tanstack/react-virtual`, `@tauri-apps/api` and the `@tauri-apps/plugin-*`
family, `@tiptap/*`, `@types/*`, `class-variance-authority`, `clsx`, `cmdk`, `date-fns`, `framer-motion`,
`lodash`, `lucide-react`, `radix-ui`, `react`, `react-dom`, `react-hook-form`, `react-markdown`,
`remark-gfm`, `sonner`, `tailwind-merge`, `tailwindcss-animate`, `zod`, `autoprefixer`, `concurrently`,
`postcss`, `tailwindcss`, `typescript`, `wait-on`, `@tailwindcss/typography`, `@tauri-apps/cli`.

Named separately (non-MIT or notable):

- **`next`** — MIT.
- **`@blocknote/core`, `@blocknote/react`, `@blocknote/shadcn`** — MPL-2.0 (copyleft-per-file; verify
  before static-linking assumptions elsewhere in the tree).
- **`@remirror/*`** — MIT.
- **`prosemirror-*`** (pinned via the `pnpm.overrides` block) — MIT.

## Excluded commercial upstream editions

No commercial upstream-edition code, binary, or asset is present in this repository. See
`PROVENANCE.md` for the search performed and its result, and `frontend/tests/lib/licenses.test.ts` for
the automated guard.
