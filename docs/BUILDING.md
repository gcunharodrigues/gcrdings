# Building gcrdings V1

gcrdings V1 targets Apple Silicon Macs. Windows, Linux, and Intel Mac builds inherited from upstream are
not supported release surfaces.

## Requirements

- macOS 26 or later
- Apple Silicon
- Full Xcode installation
- Rust 1.97.1 (`rust-toolchain.toml`)
- Node.js 22.23.1 (`.node-version`)
- pnpm 10.34.5 (`frontend/package.json`)
- CMake

Use the full Xcode toolchain without changing the machine-wide selection:

```bash
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
```

## Clean setup

```bash
./scripts/bootstrap-dev.sh
pnpm --dir frontend install --frozen-lockfile
cargo fetch --locked
./scripts/prepare-foundation-helper.sh
```

## Run

```bash
pnpm --dir frontend tauri:dev
```

## Verify

```bash
cargo test --workspace --locked
pnpm --dir frontend exec tsc --noEmit
pnpm --dir frontend build
bun test frontend/tests/lib
```

The complete measured baseline, including inherited lint and audit debt, is recorded in
[`BASELINE.md`](BASELINE.md).

## Package and release

`tauri:build` is for development only. Its output is not a qualified release package:

```bash
pnpm --dir frontend tauri:build
```

For a qualified `local_adhoc` package, obtain these three pinned inputs from the maintainer through an
authenticated private transfer. This is a local build input channel, not a GitHub repository or artifact
publication channel. Do not use an anonymous download or a moving URL.

| Environment variable | Required local input | SHA-256 |
|---|---|---|
| `GCRDINGS_FFMPEG_SOURCE` | Apple Silicon FFmpeg executable | `77d2c853f431318d55ec02676d9b2f185ebfdddb9f7677a251fbe453affe025a` |
| `ORT_LIB_LOCATION` | Directory containing `lib/libonnxruntime.a` | `e5c83560aa9e88afa39d9dca9fb5f5a767e28adb5458d1c36fe0357131b6af8b` |
| `GCRDINGS_FLUIDAUDIO_SOURCE` | Apple Silicon `libFluidAudioLocalBridge.a` | `e14173844a6c296995c9e8ca0fea574168ef2204d0c682a828f46f002892ed5d` |

Verify the transferred bytes, export the three paths, and run the closed builder:

```bash
export GCRDINGS_FFMPEG_SOURCE=/absolute/path/to/ffmpeg
export ORT_LIB_LOCATION=/absolute/path/to/onnxruntime
export GCRDINGS_FLUIDAUDIO_SOURCE=/absolute/path/to/libFluidAudioLocalBridge.a
shasum -a 256 "$GCRDINGS_FFMPEG_SOURCE"
shasum -a 256 "$ORT_LIB_LOCATION/lib/libonnxruntime.a"
shasum -a 256 "$GCRDINGS_FLUIDAUDIO_SOURCE"
./scripts/build-local-adhoc.sh
./scripts/release.sh --dry-run
```

The builder rejects a wrong hash, toolchain, dirty tracked tree, network-dependent Cargo build, or package
member outside the allowlist. It publishes the verified app, DMG, clean-build comparison, and release
manifest under `target/release/bundle/`. `release.sh --dry-run` executes the strict gate against those exact
artifacts. Signing, notarization, upload, and publication remain disabled.

## Publication boundary

The public GitHub repository contains source only. The qualified local DMG, signing inputs, models, logs,
recordings, and package receipts remain outside Git and must never be copied into a public issue, pull
request, artifact, or release. A future binary release requires a new distribution-intent profile, a new
Security Review, and a new External Release Gate. The active source-only boundary is recorded in
`.external-assurance.json`.
