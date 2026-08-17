# Private Audio Qualification Corpus

This corpus tests the local imported-audio pipeline. It contains only deterministic synthetic speech and
silence. Git stores the source manifest and scripts. Git does not store generated audio, private Recordings,
transcripts, participant names, prompts, credentials, or Session content.

## Requirements

- Apple Silicon macOS target used for V1 qualification
- `/usr/bin/say` with the `Luciana`, `Joana`, and `Samantha` system voices
- the repository-pinned Bun runtime

The manifest records the voice keys, speech rate, sample format, sample rate, expected synthetic text,
categories, failure cases, and threshold policy. The generated manifest records only sample identifiers,
durations, byte counts, and SHA-256 hashes. Regeneration on the same qualified macOS build must produce the
same generated manifest and audio hashes.

## Generate

Use a directory outside Git:

```bash
qualification/audio-corpus/scripts/generate.sh --output "$CORPUS_OUTPUT"
```

The output contains one WAV file per manifest sample and `generated-manifest.json`.

## Observation input

Native qualification records one observation per sample:

```json
{
  "schema_version": 1,
  "candidate_commit": "40 hexadecimal characters",
  "release_identity": "redacted-release-identifier",
  "target": { "hardware": "redacted-hardware-id", "os": "redacted-os-id" },
  "versions": { "corpus": "gcrdings-private-audio-v1", "engine": "pinned-engine-id" },
  "samples": [
    {
      "id": "portuguese_clear",
      "status": "completed",
      "transcript": "synthetic expected or observed text",
      "speaker_count": 1,
      "processing_ms": 1000,
      "peak_memory_mb": 512,
      "network_requests": 0,
      "diagnostic_private_content_matches": 0
    }
  ],
  "failure_cases": [
    { "id": "missing_audio", "error_code": "audio_missing", "retry_safe": true }
  ]
}
```

Observation files are temporary qualification inputs because they contain synthetic transcripts. Do not
commit them. The receipt contains aggregate metrics and case status only.

## Qualify and derive thresholds

Run the aggregate measurement and shared receipt validator:

```bash
qualification/audio-corpus/scripts/qualify.sh \
  --observations "$OBSERVATIONS" \
  --corpus-dir "$CORPUS_OUTPUT" \
  --output "$RECEIPT"
```

This first receipt has `thresholds.source` set to `pending_baseline`. Derive bounded thresholds from an
accepted baseline receipt:

```bash
qualification/audio-corpus/scripts/qualify.sh \
  --observations "$OBSERVATIONS" \
  --corpus-dir "$CORPUS_OUTPUT" \
  --baseline "$BASELINE_RECEIPT" \
  --output "$RECEIPT"
```

The calculator measures word error rate, speaker-count accuracy, real-time factor, peak memory, failure-case
behavior, network requests, and diagnostic private-content matches. Thresholds cannot exceed the safety
bounds in `manifest.json`.

The scripts perform no transcription, model installation, or network request. Native T008 qualification owns
pipeline execution and network observation. Unit tests use synthetic observations and never run a model.

## Qualified target-Mac result

The 2026-08-14 run used the pinned Silero, Parakeet, and FluidAudio configuration. The aggregate result
passed: 0.264444 word error rate, 1.0 speaker-count accuracy, 0.224177 real-time factor, and 1576 MB
peak memory. Processing observation found zero external requests and zero synthetic transcript matches in
operational diagnostics. The complete receipt remains in the private qualification workspace and is not
part of the public source tree.

Every speech case received the expected speaker count. The overlap case also produced an explicit overlap
assignment. Silence produced no principal transcript and left its Recording retryable, which is the accepted
boundary behavior.
