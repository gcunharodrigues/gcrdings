# Use FluidAudio only for speaker diarization

V1 keeps the Parakeet/Whisper transcription, existing voice activity detection, and local summary engines already validated on the target Mac. FluidAudio is added only for offline speaker diarization after transcription. Diarization failure must not discard a successful transcript; the Session remains usable without speaker labels and can be reprocessed later.

This supersedes ADR-0006. Replacing another proven engine requires a measured quality or reliability gain on the versioned Portuguese/English corpus, not architectural uniformity.

The initial Rust adapter delegated the first diarization-model installation to FluidAudio's upstream registry. That development-only path was accepted for the private target Mac, but it was not release-ready: release remained blocked until Settings provided an explicit installer, model artifacts were pinned to exact revisions and SHA-256 hashes, and ordinary processing used only the verified local installation.

**Implementation status (2026-08-14):** The explicit Settings installer now pins the model revision and validates every artifact by size and SHA-256 before atomic activation. Ordinary processing uses only the verified local installation. The development-only registry path is closed.
