# Use FluidAudio for all V1 audio intelligence

**Status:** Superseded by [ADR-0013](0013-use-fluidaudio-only-for-diarization.md).

V1 will use FluidAudio, through its Rust/Tauri integration, as the single engine for voice activity detection, batch transcription, speaker diarization, and overlap metadata. Meetily's existing capture path remains responsible for acquiring audio, while Apple Foundation Models remains responsible for summaries and structured findings. FluidAudio adoption is gated by a versioned corpus covering Brazilian Portuguese, English, code-switching, long recordings, noise, and overlapping speakers.
