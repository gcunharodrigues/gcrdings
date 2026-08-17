import { afterAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import contract from "../../../qualification/v1/cases.json";
import {
  type V1ValidationEnvironment,
  validateV1Receipt,
} from "../../../qualification/v1/validate";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const TARGET_RECEIPT = path.join(REPO_ROOT, "qualification", "v1", "target-mac.json");
const HAS_LOCAL_RELEASE_QUALIFICATION = existsSync(TARGET_RECEIPT);
const temporaryDirectories: string[] = [];

afterAll(() => temporaryDirectories.forEach((directory) => rmSync(directory, { recursive: true, force: true })));

function sha256File(filePath: string) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function temporaryArtifactRoot(candidateCommit: string) {
  const root = mkdtempSync(path.join(tmpdir(), "gcrdings-v1-receipt-"));
  temporaryDirectories.push(root);
  for (const artifact of contract.artifacts) {
    const artifactPath = path.join(root, artifact.path);
    mkdirSync(path.dirname(artifactPath), { recursive: true });
    const content = artifact.id === "application"
      ? `synthetic-binary:${artifact.candidate_marker}${candidateCommit}`
      : "synthetic-local-adhoc-dmg";
    writeFileSync(artifactPath, content);
  }
  return root;
}

function temporaryEvidenceRoot(artifactRoot: string) {
  const root = mkdtempSync(path.join(tmpdir(), "gcrdings-v1-evidence-"));
  temporaryDirectories.push(root);
  for (const declaration of contract.evidence_files) {
    const destination = path.join(root, declaration.path);
    mkdirSync(path.dirname(destination), { recursive: true });
    copyFileSync(path.join(REPO_ROOT, declaration.path), destination);
  }
  const applicationSha = sha256File(path.join(artifactRoot, contract.artifacts[0].path));
  const installerSha = sha256File(path.join(artifactRoot, contract.artifacts[1].path));
  const packageDeclaration = contract.evidence_files.find((item) => item.kind === "package")!;
  const packagePath = path.join(root, packageDeclaration.path);
  const packageEvidence = JSON.parse(readFileSync(packagePath, "utf8"));
  packageEvidence.dmg_sha256 = installerSha;
  packageEvidence.application_entries.find((entry: { path: string }) => entry.path === "Contents/MacOS/gcrdings").sha256 = applicationSha;
  writeFileSync(packagePath, `${JSON.stringify(packageEvidence, null, 2)}\n`);
  const packageSha = sha256File(packagePath);
  for (const declaration of contract.evidence_files.filter((item) => item.kind === "install")) {
    const installPath = path.join(root, declaration.path);
    const installEvidence = JSON.parse(readFileSync(installPath, "utf8"));
    installEvidence.package.manifest_sha256 = packageSha;
    installEvidence.package.dmg_sha256 = installerSha;
    writeFileSync(installPath, `${JSON.stringify(installEvidence, null, 2)}\n`);
  }
  return root;
}

function completeReceipt() {
  const candidateCommit = contract.release_candidate;
  const artifactRoot = temporaryArtifactRoot(candidateCommit);
  const evidenceRoot = temporaryEvidenceRoot(artifactRoot);
  const hashRows = (rows: Array<{ id: string; path: string }>, root: string) => rows.map((row) => ({
    id: row.id,
    path: row.path,
    sha256: sha256File(path.join(root, row.path)),
  }));

  const receipt = {
    schema_version: 1,
    receipt_type: "v1_release",
    candidate_commit: candidateCommit,
    evidence_commit: "25911fc2d7d282ae142acc01e446dc41fb9c7812",
    release_identity: contract.release_identity,
    distribution_mode: contract.distribution.mode,
    public_notarized: contract.distribution.public_notarized,
    target: structuredClone(contract.target),
    versions: structuredClone(contract.versions),
    action: { kind: "command", id: "v1_release_qualification" },
    expected_code: "complete_v1_release_qualified",
    observed_code: "complete_v1_release_qualified",
    outcome: "passed",
    metrics: {
      capture_duration_seconds: 1800.011,
      import_duration_seconds: 1800.01,
      startup_ms: 1415.076,
      graceful_shutdown_ms: 249.935,
      render_10000_blocks_ms: 756.455,
      interaction_10000_blocks_p95_ms: 89.243,
      capture_rss_growth_mib: 0,
      import_rss_growth_mib: 487.156,
      import_peak_mb: 1618.28,
      maximum_seek_error_ms: 10,
      thermal_states: ["nominal"],
      energy: [
        { case_id: "capture_energy", sample_count: 360, mean_power: 0, p95_power: 0 },
        { case_id: "import_energy", sample_count: 360, mean_power: 0, p95_power: 0 },
      ],
    },
    observations: {
      unapproved_network_requests: 0,
      diagnostic_private_matches: 0,
      safety_failures: [],
    },
    artifacts: hashRows(contract.artifacts, artifactRoot),
    locks: hashRows(contract.locks, REPO_ROOT),
    prior_receipts: hashRows(contract.prior_receipts, REPO_ROOT),
    evidence_files: hashRows(contract.evidence_files, evidenceRoot),
    thresholds: structuredClone(contract.thresholds),
    energy_proxy: structuredClone(contract.energy_proxy),
    cases: contract.cases.map((item) => ({
      id: item.id,
      command: item.command,
      expected_code: item.expected_code,
      observed_code: item.expected_code,
      status: item.required_status,
      evidence: {
        unapproved_network_requests: 0,
        private_matches: 0,
        safety_failures: [],
      },
    })),
  };
  const environment: V1ValidationEnvironment = {
    repositoryRoot: REPO_ROOT,
    artifactRoot,
    evidenceRoot,
    target: contract.target,
  };
  return { receipt, environment };
}

function expectedReceipt() {
  const { receipt, environment } = completeReceipt();
  receipt.outcome = "expected";
  receipt.observed_code = "awaiting_t028";
  for (const item of receipt.cases) {
    if (item.status === "passed") {
      item.status = "expected";
      item.observed_code = "awaiting_t028";
    }
  }
  return { receipt, environment };
}

function localReleaseArtifactsExist(receipt: { artifacts?: unknown }) {
  if (!Array.isArray(receipt.artifacts)) return false;
  return receipt.artifacts.every((artifact) => {
    if (!artifact || typeof artifact !== "object" || Array.isArray(artifact)) return false;
    const relative = (artifact as { path?: unknown }).path;
    return typeof relative === "string" && !path.isAbsolute(relative)
      && existsSync(path.join(REPO_ROOT, "target", relative));
  });
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

describe("complete V1 qualification contract", () => {
  test("records all T028 rows passed while public notarization remains unsatisfied", () => {
    expect(contract.current_state).toEqual({
      required_cases: "passed",
      public_notarized: "unsatisfied",
    });
    expect(contract.cases.filter((item) => item.required_status === "passed")).toHaveLength(40);
    expect(contract.cases.filter((item) => item.required_status === "unsatisfied").map((item) => item.id))
      .toEqual(["public_notarized"]);
  });

  test.skipIf(!HAS_LOCAL_RELEASE_QUALIFICATION)("accepts the exact closed contract through the public validator", () => {
    const { receipt, environment } = completeReceipt();
    expect(validateV1Receipt(receipt, environment)).toEqual([]);
  });

  test.skipIf(!HAS_LOCAL_RELEASE_QUALIFICATION)("binds persisted T028 evidence by hash and semantics", () => {
    const missing = completeReceipt();
    const missingPath = path.join(missing.environment.evidenceRoot!, contract.evidence_files[0].path);
    rmSync(missingPath);
    expect(validateV1Receipt(missing.receipt, missing.environment)).toContain("Declared evidence file is missing: capture_30m");

    const forgedHash = completeReceipt();
    forgedHash.receipt.evidence_files[0].sha256 = "0".repeat(64);
    expect(validateV1Receipt(forgedHash.receipt, forgedHash.environment)).toContain("On-disk evidence file hash does not match: capture_30m");

    const workload = completeReceipt();
    const workloadPath = path.join(workload.environment.evidenceRoot!, contract.evidence_files[0].path);
    const workloadDocument = JSON.parse(readFileSync(workloadPath, "utf8"));
    workloadDocument.duration_seconds += 1;
    writeFileSync(workloadPath, `${JSON.stringify(workloadDocument, null, 2)}\n`);
    workload.receipt.evidence_files[0].sha256 = sha256File(workloadPath);
    expect(validateV1Receipt(workload.receipt, workload.environment)).toContain("Workload metrics do not match evidence: capture_30m");

    const rollback = completeReceipt();
    const rollbackDeclaration = contract.evidence_files.find((item) => item.id === "rollback")!;
    const rollbackPath = path.join(rollback.environment.evidenceRoot!, rollbackDeclaration.path);
    const rollbackDocument = JSON.parse(readFileSync(rollbackPath, "utf8"));
    rollbackDocument.restored.data_sha256 = "0".repeat(64);
    writeFileSync(rollbackPath, `${JSON.stringify(rollbackDocument, null, 2)}\n`);
    rollback.receipt.evidence_files.find((item) => item.id === "rollback")!.sha256 = sha256File(rollbackPath);
    expect(validateV1Receipt(rollback.receipt, rollback.environment)).toContain("Install restoration mismatch: rollback");
  });

  test.skipIf(!HAS_LOCAL_RELEASE_QUALIFICATION)("rejects unknown nested fields, duplicates, missing rows, and conflicting success", () => {
    const { receipt, environment } = completeReceipt();

    const unknown = clone(receipt);
    (unknown.cases[0].evidence as Record<string, unknown>).forged = true;
    expect(validateV1Receipt(unknown, environment)).toContain("Unknown case evidence field: forged");

    const duplicate = clone(receipt);
    duplicate.cases.push(clone(duplicate.cases[0]));
    expect(validateV1Receipt(duplicate, environment)).toContain("Duplicate case id: capture_30_min");

    const missing = clone(receipt);
    missing.cases = missing.cases.filter((item) => item.id !== "rollback_snapshot");
    expect(validateV1Receipt(missing, environment)).toContain("Missing required case: rollback_snapshot");

    const missingTarget = clone(receipt) as typeof receipt & { target?: typeof receipt.target };
    delete missingTarget.target;
    expect(validateV1Receipt(missingTarget, environment)).toContain("target is required");

    const conflicting = clone(receipt);
    conflicting.cases[0].status = "failed";
    expect(validateV1Receipt(conflicting, environment)).toContain("Passed receipt contains failed or unexecuted cases");

    const changedCommand = clone(receipt);
    changedCommand.cases[0].command = "native:forged-command";
    expect(validateV1Receipt(changedCommand, environment)).toContain("Case capture_30_min command does not match cases.json");

    const changedCode = clone(receipt);
    changedCode.cases[0].observed_code = "forged_success";
    expect(validateV1Receipt(changedCode, environment)).toContain("Case capture_30_min observed_code does not match");
  });

  test.skipIf(!HAS_LOCAL_RELEASE_QUALIFICATION)("rejects private data, unapproved network activity, and forged artifacts or evidence", () => {
    const { receipt, environment } = completeReceipt();

    const privateData = { ...clone(receipt), transcript: "private Session words" };
    expect(validateV1Receipt(privateData, environment)).toContain("Forbidden private field: transcript");

    const network = clone(receipt);
    network.observations.unapproved_network_requests = 1;
    expect(validateV1Receipt(network, environment)).toContain("Unapproved network requests must be zero");

    const forgedLock = clone(receipt);
    forgedLock.locks[0].sha256 = "0".repeat(64);
    expect(validateV1Receipt(forgedLock, environment)).toContain("On-disk lock hash does not match: cargo_lock");

    const forgedPrior = clone(receipt);
    forgedPrior.prior_receipts[0].sha256 = "0".repeat(64);
    expect(validateV1Receipt(forgedPrior, environment)).toContain("On-disk prior receipt hash does not match: imported_audio");

    const applicationPath = path.join(environment.artifactRoot, contract.artifacts[0].path);
    writeFileSync(applicationPath, "forged-binary");
    expect(validateV1Receipt(receipt, environment)).toContain("On-disk artifact hash does not match: application");

    const forgedMarker = clone(receipt);
    writeFileSync(applicationPath, "synthetic-binary-without-candidate-marker");
    forgedMarker.artifacts[0].sha256 = sha256File(applicationPath);
    expect(validateV1Receipt(forgedMarker, environment)).toContain("Application candidate marker does not match");
  });

  test.skipIf(!HAS_LOCAL_RELEASE_QUALIFICATION)("rejects wrong target, distribution, notarization, thresholds, and ancestry", () => {
    const { receipt, environment } = completeReceipt();

    const target = clone(receipt);
    target.target.model = "Mac15,13";
    expect(validateV1Receipt(target, environment)).toContain("Target does not match the fixed release Mac");

    const distribution = clone(receipt);
    distribution.distribution_mode = "public_notarized";
    expect(validateV1Receipt(distribution, environment)).toContain("distribution_mode must be local_adhoc");

    const notarized = clone(receipt);
    notarized.public_notarized = "passed";
    expect(validateV1Receipt(notarized, environment)).toContain("public_notarized must remain unsatisfied");

    const threshold = clone(receipt);
    threshold.thresholds.maximum_startup_ms = 15001;
    expect(validateV1Receipt(threshold, environment)).toContain("Threshold contract does not match cases.json");

    const ancestry = clone(receipt);
    ancestry.candidate_commit = "0".repeat(40);
    expect(validateV1Receipt(ancestry, environment)).toContain("candidate_commit does not resolve in Git");

    const reversedAncestry = clone(receipt);
    reversedAncestry.evidence_commit = contract.base_commit;
    expect(validateV1Receipt(reversedAncestry, environment)).toContain("candidate_commit is not an ancestor of evidence_commit");

    const energy = clone(receipt);
    energy.metrics.energy[0].sample_count = 341;
    expect(validateV1Receipt(energy, environment)).toContain("Energy sample count is below 342: capture_energy");
  });

  test("reports only the exact T028 rows that have not been executed", () => {
    if (!HAS_LOCAL_RELEASE_QUALIFICATION) {
      expect(existsSync(TARGET_RECEIPT)).toBe(false);
      return;
    }
    if (existsSync(TARGET_RECEIPT)) {
      const receipt = JSON.parse(readFileSync(TARGET_RECEIPT, "utf8"));
      if (localReleaseArtifactsExist(receipt)) expect(validateV1Receipt(receipt)).toEqual([]);
      else expect(receipt.outcome).toBe("passed");
      return;
    }

    const { receipt, environment } = expectedReceipt();
    const errors = validateV1Receipt(receipt, environment);
    expect(errors.length).toBe(contract.cases.filter((item) => item.required_status === "passed").length);
    expect(errors.every((error) => error.endsWith("has not been executed"))).toBe(true);
  });

  test("T028 execution remains blocked until every required row passes", () => {
    if (!HAS_LOCAL_RELEASE_QUALIFICATION) {
      expect(existsSync(TARGET_RECEIPT)).toBe(false);
      return;
    }
    if (existsSync(TARGET_RECEIPT)) {
      const receipt = JSON.parse(readFileSync(TARGET_RECEIPT, "utf8"));
      if (localReleaseArtifactsExist(receipt)) expect(validateV1Receipt(receipt)).toEqual([]);
      else expect(receipt.public_notarized).toBe("unsatisfied");
      return;
    }
    const { receipt, environment } = expectedReceipt();
    const errors = validateV1Receipt(receipt, environment);
    expect(errors).toEqual(
      contract.cases
        .filter((item) => item.required_status === "passed")
        .map((item) => `Case ${item.id} has not been executed`),
    );
  });
});
