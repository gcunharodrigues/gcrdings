import { describe, expect, test } from "bun:test";
import { validateReceipt } from "../../../qualification/validate-receipt";

const caseIds = [
  "microphone_only",
  "system_only",
  "both_origins",
  "silence",
  "overlap",
  "ambiguous_duplicate",
  "source_loss",
  "retry",
  "timestamp_seek",
];

function sourceOrigins(id: string) {
  if (id === "microphone_only") return ["microphone"];
  if (id === "system_only" || id === "silence") return ["system-audio"];
  return ["microphone", "system-audio"];
}

function receipt() {
  return {
    schema_version: 1,
    receipt_type: "captured_timeline",
    candidate_commit: "a".repeat(40),
    release_identity: "gcrdings-v1-wave3",
    target: { hardware: "Apple-Silicon", os: "macOS-26.5" },
    versions: {
      capture: "native-audio-v1",
      transcription: "parakeet-pinned",
      diarization: "fluidaudio-pinned",
    },
    action: { kind: "command", id: "captured_timeline_qualification" },
    expected_code: "captured_origins_preserve_timeline",
    observed_code: "captured_timeline_passed",
    outcome: "passed",
    metrics: { duration_ms: 1_000, peak_memory_mb: 512, maximum_seek_error_ms: 80 },
    observations: {
      network_requests: 0,
      diagnostic_private_content_matches: 0,
      safety_failures: [],
    },
    artifacts: [
      { id: "microphone_source", sha256: "b".repeat(64) },
      { id: "system_source", sha256: "c".repeat(64) },
      { id: "mixed_source", sha256: "d".repeat(64) },
    ],
    thresholds: {
      source: "fixed_acceptance",
      values: { maximum_seek_error_ms: 100 },
      failures: [],
    },
    cases: caseIds.map((id) => ({
      id,
      category: id,
      status: "passed",
      evidence: {
        source_origins: sourceOrigins(id),
        passage_count: id === "silence" ? 0 : 1,
        source_hash_unchanged: true,
        retry_safe: true,
        ...(id === "timestamp_seek" ? { seek_error_ms: 80 } : {}),
        ...(id === "overlap" || id === "ambiguous_duplicate" ? { ambiguity_preserved: true } : {}),
        ...(id === "source_loss" ? { failed_origins: ["system-audio"] } : {}),
      },
    })),
  };
}

describe("captured timeline qualification receipt", () => {
  test("accepts the closed captured-origin contract", () => {
    expect(validateReceipt(receipt())).toEqual([]);
  });

  test("rejects missing cases, excessive seek error, provenance gaps, and private fields", () => {
    const candidate = receipt();
    candidate.cases.pop();
    expect(validateReceipt(candidate)).toContain("captured_timeline is missing required case: timestamp_seek");

    const excessiveSeek = receipt();
    excessiveSeek.metrics.maximum_seek_error_ms = 101;
    expect(validateReceipt(excessiveSeek)).toContain("captured_timeline seek error exceeds 100 ms");

    expect(validateReceipt({ ...receipt(), artifacts: [] })).toContain(
      "captured_timeline requires microphone, system, and mixed source hashes",
    );
    expect(validateReceipt({ ...receipt(), transcript: "private words" })).toContain(
      "Forbidden private field: transcript",
    );
  });

  test("binds each case to exact origins, failed origins, and aggregate seek", () => {
    const missingSystemOrigin = receipt();
    missingSystemOrigin.cases.find(({ id }) => id === "system_only")!.evidence.source_origins = [];
    expect(validateReceipt(missingSystemOrigin)).toContain(
      "captured_timeline case system_only has invalid source origins",
    );

    const malformedSourceLossOrigins = receipt();
    Object.assign(
      malformedSourceLossOrigins.cases.find(({ id }) => id === "source_loss")!.evidence,
      { source_origins: {} },
    );
    expect(validateReceipt(malformedSourceLossOrigins)).toContain(
      "captured_timeline case source_loss has invalid source origins",
    );

    const missingBothOrigin = receipt();
    missingBothOrigin.cases.find(({ id }) => id === "both_origins")!.evidence.source_origins = [
      "microphone",
    ];
    expect(validateReceipt(missingBothOrigin)).toContain(
      "captured_timeline case both_origins has invalid source origins",
    );

    const duplicateFailedOrigin = receipt();
    duplicateFailedOrigin.cases.find(({ id }) => id === "source_loss")!.evidence.failed_origins = [
      "microphone",
      "microphone",
    ];
    expect(validateReceipt(duplicateFailedOrigin)).toContain(
      "captured_timeline source loss has invalid failed origins",
    );

    const inconsistentSeek = receipt();
    inconsistentSeek.metrics.maximum_seek_error_ms = 0;
    expect(validateReceipt(inconsistentSeek)).toContain(
      "captured_timeline maximum seek metric must equal case maximum",
    );

    const unknownCase = receipt();
    const firstCase = unknownCase.cases[0];
    Object.assign(firstCase, { id: "unknown_case", category: "unknown_case" });
    firstCase.evidence.source_origins = [];
    expect(validateReceipt(unknownCase)).toContain(
      "captured_timeline case unknown_case has invalid source origins",
    );
  });
});
