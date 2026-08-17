import { describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { validateReceipt } from "../../../qualification/validate-receipt";

const candidate = "a".repeat(40);
const applicationPath = "release/bundle/macos/gcrdings.app/Contents/MacOS/gcrdings";
const installerPath = "release/bundle/dmg/gcrdings_0.4.0_aarch64.dmg";
const artifactRoot = mkdtempSync(path.join(os.tmpdir(), "gcrdings-provider-receipt-"));
const application = Buffer.from(`binary:gcrdings-build-commit:${candidate}`);
const installer = Buffer.from("synthetic-dmg");
for (const [relative, content] of [[applicationPath, application], [installerPath, installer]] as const) {
  const absolute = path.join(artifactRoot, relative);
  mkdirSync(path.dirname(absolute), { recursive: true });
  writeFileSync(absolute, content);
}
const sha256 = (value: Buffer) => createHash("sha256").update(value).digest("hex");
const environment = {
  artifactRoot,
  candidateCommit: candidate,
  artifactHashes: { application: sha256(application), installer: sha256(installer) },
  target: {
    model: "Mac15,12",
    chip: "Apple M3",
    memory: "24 GiB",
    arch: "arm64",
    os: "macOS-26.5.2",
    build: "25F84",
  },
};
const validate = (value: unknown) => validateReceipt(value, { providerAuthorization: environment });

const observedCodes: Record<string, string> = {
  keychain_save: "native_keychain_saved",
  keychain_test: "native_keychain_tested",
  keychain_replace: "native_keychain_replaced",
  keychain_remove: "native_keychain_removed",
  legacy_migration: "native_legacy_migrated",
  same_source_retry: "native_same_source_retry_completed",
  different_source_rejection: "rust_different_source_rejected",
  fresh_retained_rejection: "rust_fresh_retained_rejected",
  fresh_symlink_rejection: "rust_fresh_symlink_rejected",
  task_enable_disable: "native_task_authorization_toggled",
  settings_persistence: "native_settings_rehydrated",
  cancellation: "native_cancelled_zero_send",
  changed_snapshot: "rust_stale_preview_rejected",
  provider_failure: "native_typed_provider_failure",
  explicit_retry: "native_explicit_retry_completed",
  outcome_unknown: "native_outcome_unknown_blocked",
  exact_payload: "native_exact_payload_sent",
  no_fallback: "rust_no_fallback",
  secret_scan: "native_secret_scan_clean",
};

function receipt() {
  return {
    schema_version: 1,
    receipt_type: "provider_authorization",
    candidate_commit: candidate,
    release_identity: "gcrdings-0.4.0-aarch64-ad-hoc",
    target: environment.target,
    versions: { keychain: "macos-security-framework", transfer: "provider-transfer-v1", app: "0.4.0" },
    action: { kind: "ui", id: "provider_authorization_qualification" },
    expected_code: "explicit_keychain_provider_transfer",
    observed_code: "provider_authorization_passed",
    outcome: "passed",
    metrics: {
      maximum_requests_per_confirmation: 1,
      cancellation_requests: 0,
      automatic_retries: 0,
      payload_video_fields: 0,
      secret_matches: 0,
    },
    observations: {
      fallback_requests: 0,
      substitution_requests: 0,
      diagnostic_private_content_matches: 0,
      safety_failures: [],
    },
    artifacts: [
      { id: "application", path: applicationPath, sha256: environment.artifactHashes.application },
      { id: "installer", path: installerPath, sha256: environment.artifactHashes.installer },
    ],
    thresholds: {
      source: "fixed_acceptance",
      values: {
        maximum_requests_per_confirmation: 1,
        maximum_cancellation_requests: 0,
        maximum_automatic_retries: 0,
        maximum_payload_video_fields: 0,
        maximum_secret_matches: 0,
      },
      failures: [],
    },
    cases: Object.entries(observedCodes).map(([id, observed_code]) => ({
      id,
      category: id,
      status: "passed",
      evidence: {
        observed_code,
        requests: ["provider_failure", "explicit_retry", "outcome_unknown", "exact_payload"].includes(id) ? 1 : 0,
        secret_matches: 0,
        fallback_requests: 0,
        video_fields: 0,
      },
    })),
  };
}

describe("provider authorization qualification receipt", () => {
  test("accepts only the closed native provider contract", () => {
    expect(validate(receipt())).toEqual([]);
  });

  test("rejects forged identity, target, requests, payload, secrets, cases, and private data", () => {
    const identity = receipt();
    identity.candidate_commit = "0".repeat(40);
    expect(validate(identity)).toContain("provider_authorization candidate identity does not match");

    const target = receipt();
    target.target = { ...environment.target, model: "Mac15,13" };
    expect(validate(target)).toContain("provider_authorization target does not match");

    const extraRequest = receipt();
    extraRequest.cases.at(-2)!.evidence.requests = 2;
    expect(validate(extraRequest)).toContain("provider_authorization case no_fallback exceeds one request");

    const unsafePayload = receipt();
    unsafePayload.metrics.payload_video_fields = 1;
    expect(validate(unsafePayload)).toContain("provider_authorization requires clean one-use transfers");

    const secret = receipt();
    secret.metrics.secret_matches = 1;
    expect(validate(secret)).toContain("provider_authorization requires clean one-use transfers");

    const missing = receipt();
    missing.cases.pop();
    expect(validate(missing)).toContain("provider_authorization is missing required case: secret_scan");

    const forgedCase = receipt();
    forgedCase.cases[0].evidence.observed_code = "fabricated_pass";
    expect(validate(forgedCase)).toContain("provider_authorization case keychain_save has incompatible observed_code");

    expect(validate({ ...receipt(), credential: "synthetic-private-value" }))
      .toContain("Forbidden private field: credential");
  });

  test("rejects forged on-disk artifacts and the same-family wrong Mac target", () => {
    const applicationFile = path.join(artifactRoot, applicationPath);
    writeFileSync(applicationFile, "forged-binary");
    expect(validate(receipt())).toContain("provider_authorization on-disk artifact hash does not match: application");
    writeFileSync(applicationFile, application);

    const sameFamily = receipt();
    sameFamily.target = { ...environment.target, model: "Mac15,13" };
    expect(validate(sameFamily)).toContain("provider_authorization target does not match");
  });
});
