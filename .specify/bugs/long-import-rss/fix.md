# Bug Fix: Bound long-import transcription memory

- **Slug**: long-import-rss
- **Fixed**: 2026-08-16
- **Assessment**: ./assessment.md
- **Status**: applied

## Summary

Long-audio transcription converts the source through the verified bundled FFmpeg into a private temporary 16 kHz mono PCM file. Before speaker diarization starts, batch processing unloads the transcription model and asks the macOS allocator to release unused pages. Parakeet and FluidAudio no longer overlap in memory.

## Changes

| File | Change | Notes |
|------|--------|-------|
| `frontend/src-tauri/src/audio/decoder.rs` | modified | Adds the bounded transcription decoder, consumes compatible samples, sanitizes touched diagnostics, and adds regressions. |
| `frontend/src-tauri/src/audio/common.rs` | modified | Exposes the existing guarded transcription-engine unload at the earlier lifecycle boundary. |
| `frontend/src-tauri/src/audio/retranscription.rs` | modified | Routes every origin through the bounded decoder and unloads transcription before FluidAudio diarization. |

## Tests Added or Updated

- `audio::decoder::tests::compatible_mono_audio_reuses_its_allocation` — proves 16 kHz mono samples move without a new allocation.
- `audio::decoder::tests::transcription_decode_is_16khz_mono_and_preserves_source` — proves bounded format conversion and immutable source bytes.

## Local Verification

- `cargo fmt --all --check` → passed.
- `cargo test -p gcrdings audio::decoder::tests --lib --locked` → 18 passed in 27.33 seconds after the focused compile; the pre-fix test failed because the bounded decoder did not exist.
- `cargo test -p gcrdings audio::retranscription --lib --locked` → 35 passed in 23.95 seconds.
- The first packaged 46:27 rerun exposed a second overlap: 2049 MiB peak while Parakeet and FluidAudio were resident together.
- The 45-second target-Mac reproduction loop now passes on the same 46:27 file: 1610.8 MB peak and 491.0 MiB end-to-start RSS growth.

## Deviations from Assessment

The initial assessment correctly found complete-buffer costs but did not identify the later Parakeet/FluidAudio overlap. Target-Mac `vmmap` evidence showed both model families resident together. The final fix moves the existing engine unload before diarization.

## Follow-ups

- Rebuild the immutable package and repeat the 46:27 import with the five-second 30-minute target-Mac sampler before marking the fix verified.
