import { describe, expect, test } from "bun:test";
import { validateReceipt } from "../../../qualification/validate-receipt";

const caseIds = [
  "meeting",
  "interview",
  "content",
  "stale_revision",
  "invalid_evidence",
  "model_unavailable",
  "apple_intelligence_disabled",
  "unsupported_locale",
  "context_limit",
  "cancellation",
  "helper_death",
];
const caseObservedCodes: Record<string, string> = {
  meeting: "native_completed",
  interview: "native_completed",
  content: "native_completed",
  stale_revision: "native_stale_state",
  invalid_evidence: "rust_rejected_invalid_evidence",
  model_unavailable: "typed_unavailable_state",
  apple_intelligence_disabled: "framework_disabled_mapping",
  unsupported_locale: "native_typed_unavailable",
  context_limit: "native_typed_context_size",
  cancellation: "rust_typed_cancelled",
  helper_death: "typed_helper_failed",
};

function receipt() {
  return {
    schema_version: 1,
    receipt_type: "local_findings",
    candidate_commit: "b3b2f82919c3747b71b698451f41eb3f86b57fcf",
    release_identity: "gcrdings-0.4.0-aarch64-ad-hoc",
    target: { hardware: "Apple-Silicon", os: "macOS-26.5" },
    versions: { helper: "foundation-helper-v1", framework: "FoundationModels", prompt_version: "v1" },
    action: { kind: "ui", id: "local_findings_qualification" },
    expected_code: "grounded_local_findings",
    observed_code: "local_findings_passed",
    outcome: "passed",
    metrics: { duration_ms: 10_000, peak_memory_mb: 21, evidence_resolution_rate: 1 },
    observations: { network_requests: 0, diagnostic_private_content_matches: 0, safety_failures: [] },
    artifacts: [
      { id: "application", sha256: "045f2dfff039cf7cd384845f46159b04a93f64a62cc4e9a0aba4038f98b15076" },
      { id: "foundation_helper", sha256: "16667dfdfa6a512b75ad27f256f7943fe2250ffcc47f3a1c1f0b33b3f9f22a6c" },
      { id: "installer", sha256: "f11249f26dccee911d255bd4eae1a9877cd60f67e0275f1df09a10528d020253" },
    ],
    thresholds: {
      source: "fixed_acceptance",
      values: { minimum_evidence_resolution_rate: 1 },
      failures: [],
    },
    cases: caseIds.map((id) => ({
      id,
      category: id,
      status: "passed",
      evidence: {
        observed_code: caseObservedCodes[id],
        principal_unchanged: true,
        network_requests: 0,
        ...(new Set(["meeting", "interview", "content"]).has(id)
          ? { evidence_resolution_rate: 1 }
          : {}),
      },
    })),
  };
}

describe("local findings qualification receipt", () => {
  test("accepts only the closed local findings contract", () => {
    expect(validateReceipt(receipt())).toEqual([]);
  });

  test("rejects missing cases, unresolved evidence, network, and private data", () => {
    const missing = receipt();
    missing.cases.pop();
    expect(validateReceipt(missing)).toContain("local_findings is missing required case: helper_death");

    const unresolved = receipt();
    unresolved.metrics.evidence_resolution_rate = 0;
    expect(validateReceipt(unresolved)).toContain("local_findings requires complete evidence resolution");

    const network = receipt();
    network.observations.network_requests = 1;
    expect(validateReceipt(network)).toContain("local_findings requires zero safety observations");

    expect(validateReceipt({ ...receipt(), transcript: "private words" })).toContain(
      "Forbidden private field: transcript",
    );
  });

  test("binds candidate, release, artifact identities, and case observations", () => {
    const forgedIdentity = receipt();
    forgedIdentity.candidate_commit = "0".repeat(40);
    forgedIdentity.release_identity = "fabricated-release";
    expect(validateReceipt(forgedIdentity)).toContain("local_findings candidate identity does not match");

    const duplicateArtifact = receipt();
    duplicateArtifact.artifacts.push({ id: "application", sha256: "e".repeat(64) });
    expect(validateReceipt(duplicateArtifact)).toContain("local_findings requires exactly one hash per artifact");

    const substitutedHashes = receipt();
    substitutedHashes.artifacts.forEach((artifact, index) => { artifact.sha256 = `${index + 1}`.repeat(64); });
    expect(validateReceipt(substitutedHashes)).toContain("local_findings artifact hash does not match: application");

    const forgedCases = receipt();
    forgedCases.cases.forEach((item) => { item.evidence.observed_code = "fabricated_pass"; });
    expect(validateReceipt(forgedCases)).toContain("local_findings case meeting has incompatible observed_code");
  });
});
