import { describe, expect, test } from "bun:test";
import { validateReceipt } from "../../../qualification/validate-receipt";

const observedCodes: Record<string, string> = {
  save_markdown: "native_markdown_saved",
  save_json: "native_json_saved",
  share: "native_share_presented",
  cancellation: "native_save_cancelled",
  overwrite: "native_atomic_overwrite",
  unwritable_destination: "rust_write_failed",
  stale_prior_export: "native_current_snapshot_rebuilt",
  temporary_cleanup: "rust_temporary_cleanup",
  content_scan: "equivalent_content_clean",
};

function receipt() {
  return {
    schema_version: 1,
    receipt_type: "agent_handoff",
    candidate_commit: "611f1dd567c3b2694c665eb6b36d76f54588e587",
    release_identity: "gcrdings-0.4.0-aarch64-ad-hoc",
    target: { hardware: "Apple-Silicon", os: "macOS-26.5.2" },
    versions: { serializer: "agent-handoff-v1", app: "0.4.0" },
    action: { kind: "ui", id: "agent_handoff_qualification" },
    expected_code: "equivalent_explicit_handoff",
    observed_code: "agent_handoff_passed",
    outcome: "passed",
    metrics: { semantic_equivalence_rate: 1, forbidden_field_matches: 0, temporary_artifact_count: 0 },
    observations: { session_changed: false, network_requests: 0, safety_failures: [] },
    artifacts: [
      { id: "application", sha256: "7d5c18f0a709b1b4da559a7ef2b96dfcc2f38dfbe3faae12779c65d0bb1dd26a" },
      { id: "installer", sha256: "29469315ed411655420b63c529155f0a553a76a67281ada33950bdb94c027355" },
      { id: "exported_markdown", sha256: "bc02fd3479547bdf5074e2f3e9880c71512e126a0d3b50c6ced2c1147a0c4e95" },
      { id: "exported_json", sha256: "5f003b45ada3c6f1c5dabb3af7881bfa4d702b21b3affad26906674ecb11019d" },
    ],
    thresholds: {
      source: "fixed_acceptance",
      values: {
        minimum_semantic_equivalence_rate: 1,
        maximum_forbidden_field_matches: 0,
        maximum_temporary_artifacts: 0,
      },
      failures: [],
    },
    cases: Object.entries(observedCodes).map(([id, observed_code]) => ({
      id,
      category: id,
      status: "passed",
      evidence: { observed_code, session_unchanged: true, temporary_artifacts: 0 },
    })),
  };
}

describe("Agent Handoff qualification receipt", () => {
  test("accepts only the closed native handoff contract", () => {
    expect(validateReceipt(receipt())).toEqual([]);
  });

  test("rejects forged identity, artifacts, observations, cases, and private data", () => {
    const identity = receipt();
    identity.candidate_commit = "0".repeat(40);
    expect(validateReceipt(identity)).toContain("agent_handoff candidate identity does not match");

    const target = receipt();
    target.target = { hardware: "Intel", os: "macOS-99.9" };
    expect(validateReceipt(target)).toContain("agent_handoff target does not match");

    const artifacts = receipt();
    artifacts.artifacts.push({ id: "application", sha256: "f".repeat(64) });
    expect(validateReceipt(artifacts)).toContain("agent_handoff requires exactly one hash per artifact");

    const substituted = receipt();
    substituted.artifacts[0].sha256 = "a".repeat(64);
    expect(validateReceipt(substituted)).toContain("agent_handoff artifact hash does not match: application");

    const unsafe = receipt();
    unsafe.observations.session_changed = true;
    unsafe.observations.network_requests = 1;
    expect(validateReceipt(unsafe)).toContain("agent_handoff requires unchanged Session and zero network requests");

    const missing = receipt();
    missing.cases.pop();
    expect(validateReceipt(missing)).toContain("agent_handoff is missing required case: content_scan");

    const forgedCase = receipt();
    forgedCase.cases[0].evidence.observed_code = "fabricated_pass";
    expect(validateReceipt(forgedCase)).toContain("agent_handoff case save_markdown has incompatible observed_code");

    expect(validateReceipt({ ...receipt(), transcript: "private words" })).toContain("Forbidden private field: transcript");
  });
});
