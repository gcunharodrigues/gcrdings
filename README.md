<div align="center" style="border-bottom: none">
    <h1>
        gcrdings
    </h1>
    <h3>
    Turn recordings into private, verifiable records
    </h3>
</div>

---

## Introduction

gcrdings turns captured or imported audio into private, verifiable records that can be reviewed by
people and reused by AI assistants. Capture and transcription run locally by default. Optional external
summary providers send transcript text only after you configure and select them.

gcrdings is an independent public derivative of the Meetily Community codebase. It is not a GitHub fork
relationship; see [`PROVENANCE.md`](PROVENANCE.md) for the upstream revision, attribution, and license
inventory.

## Features

- **Local First:** Audio capture and batch transcription run on your Mac.
- **Post-recording Transcription:** Process recordings after capture; real-time transcription is planned
  for V2.
- **AI-Powered Summaries:** Generate summaries using powerful language models.
- **macOS V1:** The verified baseline targets Apple Silicon Macs.
- **Open Source:** gcrdings is open source under the MIT License.
- **Flexible AI Provider Support:** Use a local model or explicitly configure an external provider for
  summaries.

## Installation

The public repository distributes source only. DMG files, signing inputs, models, recordings, and release
packages remain local and are not published. Build from source for development:

- [General Build Instructions](docs/BUILDING.md)

**Quick start:**

```bash
./scripts/bootstrap-dev.sh
pnpm --dir frontend install --frozen-lockfile
cargo fetch --locked
./scripts/prepare-foundation-helper.sh
pnpm --dir frontend tauri:dev
```

## Key Features in Action

### 🎯 Local Transcription

Transcribe recordings entirely on your device using **Whisper** or **Parakeet** models. No cloud required.

### 📥 Import & Enhance

Import existing audio files to generate transcripts, or re-transcribe any recording with a different
model or language, all processed locally.

### 🤖 AI-Powered Summaries

Generate summaries locally or with an external provider that you explicitly configure. External providers
receive transcript text for the selected task.

### 🔒 Privacy-First Design

Recordings, transcription models, and transcripts are stored locally. External summary providers are
optional and never active until configured.

### 🌐 Custom OpenAI Endpoint Support

Use your own OpenAI-compatible endpoint for AI summaries.

### 🎙️ Professional Audio Mixing

Capture microphone and system audio simultaneously with intelligent ducking and clipping prevention.

### ⚡ GPU Acceleration

V1 uses Apple Silicon acceleration through Metal and Core ML.

Automatically enabled at build time - no configuration needed.

## System Architecture

gcrdings is a single, self-contained application built with [Tauri](https://tauri.app/). It uses a
Rust-based backend to handle all the core logic, and a Next.js frontend for the user interface.

For more details, see the [Architecture documentation](docs/architecture.md).

## For Developers

If you want to contribute to gcrdings or build it from source, you'll need to have Rust and Node.js
installed. For detailed build instructions, please see the [Building from Source guide](docs/BUILDING.md).

## Contributing

We welcome contributions from the community! If you have any questions or suggestions, please open an
issue or submit a pull request. Please follow the established project structure and guidelines. For
more details, refer to the [CONTRIBUTING.md](CONTRIBUTING.md) file.

## License

MIT License - see [`LICENSE.md`](LICENSE.md). gcrdings is a public derivative of an MIT-licensed upstream codebase;
see [`PROVENANCE.md`](PROVENANCE.md) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for upstream
attribution and third-party licenses.

## Acknowledgments

- We borrowed some code from [Whisper.cpp](https://github.com/ggerganov/whisper.cpp).
- We borrowed some code from [Screenpipe](https://github.com/mediar-ai/screenpipe).
- We borrowed some code from [transcribe-rs](https://crates.io/crates/transcribe-rs).
- Thanks to **NVIDIA** for developing the **Parakeet** model.
- Thanks to [istupakov](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx) for providing the
  **ONNX conversion** of the Parakeet model.
