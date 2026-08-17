# Bug Assessment: Long import exceeds the release memory limit

- **Slug**: long-import-rss
- **Created**: 2026-08-16
- **Source**: measured during T028 target-Mac qualification
- **Verdict**: valid
- **Severity**: high

## Report

The exact packaged candidate `b1da6a2cfbd31d525ae00e11750b38d1e0c9e688` imported a 46:27 local audio file. Resident memory rose from approximately 1003 MiB after processing to approximately 2036 MiB during transcription. The release contract permits at most 512 MiB growth and 1970 MB peak import memory.

## Symptom

Long local imports complete, but transcription temporarily retains more than 1 GiB above the steady app footprint. The expected result is bounded processing below both fixed release limits.

## Reproduction

1. Launch the packaged target-Mac candidate after the local model is loaded.
2. Import the preserved 46:27 system-audio track through the native file picker.
3. Sample app RSS every five seconds while transcription runs.
4. Observe an approximately 2036 MiB peak and more than 1 GiB growth.

## Suspected Code Paths

- `frontend/src-tauri/src/audio/retranscription.rs:process_audio_origin()` decodes the complete origin before VAD.
- `frontend/src-tauri/src/audio/decoder.rs:DecodedAudio::to_whisper_format()` borrows and clones a complete mono buffer.
- `frontend/src-tauri/src/audio/decoder.rs:chunked_resample_with_progress()` retains all resampled chunks before merging them.

## Root Cause Hypothesis

Confidence: high. A long 48 kHz origin is first materialized as decoded `f32` samples. Mono conversion clones the complete buffer, and parallel resampling retains all output chunks before the final merge. The loaded transcription model remains resident at the same time. These overlapping allocations explain the measured transient peak.

## Proposed Remediation

**Preferred**: Add one transcription-specific decoder path. Use the already verified bundled FFmpeg to stream the source into a private temporary 16 kHz mono PCM WAV. Decode that bounded intermediate, then consume `DecodedAudio` so an already compatible mono buffer moves into VAD without cloning. Keep the preserved source and playback file unchanged. Sanitize the touched decoder diagnostics so the source path is not emitted.

**Alternative**:

- Implement streaming decode, resampling, and VAD in Rust. This is larger and unnecessary while the verified bundled FFmpeg already provides the required bounded conversion.

**Files likely to change**:

- `frontend/src-tauri/src/audio/decoder.rs`
- `frontend/src-tauri/src/audio/retranscription.rs`

**Tests to add or update**:

- Prove compatible mono samples reuse their existing allocation.
- Prove the transcription decoder returns 16 kHz mono audio and preserves source bytes.
- Repeat the 46:27 native import with the fixed five-second sampler.

## Risks & Considerations

- Cancellation must still block persistence after conversion.
- The temporary file must be deleted on success, failure, or panic.
- Conversion must use only the verified bundled FFmpeg and must not expose a private source path in diagnostics.
- A new production candidate requires ordinary review, Security Review, Release Gate, and repeated T028 qualification.

## Open Questions

- None.
