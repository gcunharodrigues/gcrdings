import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import contract from "./cases.json";

const REPOSITORY_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const SHA256 = /^[a-f0-9]{64}$/;
const COMMIT = /^[a-f0-9]{40}$/;
const FORBIDDEN_FIELDS = new Set([
  "api_key",
  "apikey",
  "audio",
  "credential",
  "credentials",
  "participant_name",
  "participant_names",
  "password",
  "payload",
  "prompt",
  "secret",
  "session_content",
  "source_path",
  "token",
  "transcript",
]);
const TOP_LEVEL_FIELDS = new Set([
  "schema_version",
  "receipt_type",
  "candidate_commit",
  "evidence_commit",
  "release_identity",
  "distribution_mode",
  "public_notarized",
  "target",
  "versions",
  "action",
  "expected_code",
  "observed_code",
  "outcome",
  "metrics",
  "observations",
  "artifacts",
  "locks",
  "prior_receipts",
  "evidence_files",
  "thresholds",
  "energy_proxy",
  "cases",
]);
const TARGET_FIELDS = new Set(["model", "chip", "memory", "arch", "os", "build"]);
const VERSION_FIELDS = new Set(Object.keys(contract.versions));
const ACTION_FIELDS = new Set(["kind", "id"]);
const METRIC_FIELDS = new Set([
  "capture_duration_seconds",
  "import_duration_seconds",
  "startup_ms",
  "graceful_shutdown_ms",
  "render_10000_blocks_ms",
  "interaction_10000_blocks_p95_ms",
  "capture_rss_growth_mib",
  "import_rss_growth_mib",
  "import_peak_mb",
  "maximum_seek_error_ms",
  "thermal_states",
  "energy",
]);
const ENERGY_FIELDS = new Set(["case_id", "sample_count", "mean_power", "p95_power"]);
const OBSERVATION_FIELDS = new Set([
  "unapproved_network_requests",
  "diagnostic_private_matches",
  "safety_failures",
]);
const HASH_ROW_FIELDS = new Set(["id", "path", "sha256"]);
const CASE_FIELDS = new Set(["id", "command", "expected_code", "observed_code", "status", "evidence"]);
const CASE_EVIDENCE_FIELDS = new Set([
  "unapproved_network_requests",
  "private_matches",
  "safety_failures",
]);

export type V1ValidationEnvironment = {
  repositoryRoot: string;
  artifactRoot: string;
  evidenceRoot?: string;
  target: typeof contract.target;
};

type HashRow = { id: string; path: string; sha256: string };

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNonNegative(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function rejectUnknownFields(
  value: Record<string, unknown>,
  allowed: Set<string>,
  location: string,
  errors: string[],
) {
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) errors.push(`Unknown ${location} field: ${key}`);
  }
}

function scanPrivateData(value: unknown, errors: string[]) {
  if (Array.isArray(value)) {
    value.forEach((item) => scanPrivateData(item, errors));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key
        .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
        .toLocaleLowerCase("en-US")
        .replaceAll("-", "_");
      if (FORBIDDEN_FIELDS.has(normalized)) errors.push(`Forbidden private field: ${key}`);
      scanPrivateData(child, errors);
    }
    return;
  }
  if (typeof value !== "string") return;
  if (/^(?:\/Users\/|\/private\/|[A-Za-z]:\\)/.test(value)) errors.push("Potential private path in receipt");
  if (/\bBearer\s+\S+|\bsk-[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]+ PRIVATE KEY-----/i.test(value)) {
    errors.push("Potential credential in receipt");
  }
  if (/\b[^\s@]+@[^\s@]+\.[^\s@]+\b/.test(value)) errors.push("Potential personal email in receipt");
}

function canonical(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (isObject(value)) {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function hashFile(filePath: string): string {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

function resolveInside(root: string, relativePath: string): string | undefined {
  if (path.isAbsolute(relativePath)) return undefined;
  const resolvedRoot = path.resolve(root);
  const resolved = path.resolve(resolvedRoot, relativePath);
  const relation = path.relative(resolvedRoot, resolved);
  return relation !== "" && !relation.startsWith("..") && !path.isAbsolute(relation) ? resolved : undefined;
}

function command(commandName: string, args: string[]): string {
  return execFileSync(commandName, args, { encoding: "utf8" }).trim();
}

function machineTarget(): typeof contract.target {
  const memoryBytes = Number(command("sysctl", ["-n", "hw.memsize"]));
  return {
    model: command("sysctl", ["-n", "hw.model"]),
    chip: command("sysctl", ["-n", "machdep.cpu.brand_string"]),
    memory: `${Math.round(memoryBytes / 1024 ** 3)} GiB`,
    arch: command("uname", ["-m"]),
    os: `macOS-${command("sw_vers", ["-productVersion"])}`,
    build: command("sw_vers", ["-buildVersion"]),
  };
}

function defaultArtifactRoot(repositoryRoot: string): string {
  return process.env.CARGO_TARGET_DIR
    ? path.resolve(process.env.CARGO_TARGET_DIR)
    : path.join(repositoryRoot, "target");
}

function defaultEnvironment(): V1ValidationEnvironment {
  return {
    repositoryRoot: REPOSITORY_ROOT,
    artifactRoot: defaultArtifactRoot(REPOSITORY_ROOT),
    target: machineTarget(),
  };
}

function gitCommitExists(repositoryRoot: string, commit: string): boolean {
  if (!COMMIT.test(commit)) return false;
  try {
    execFileSync("git", ["-C", repositoryRoot, "cat-file", "-e", `${commit}^{commit}`], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function gitIsAncestor(repositoryRoot: string, ancestor: string, descendant: string): boolean {
  try {
    execFileSync("git", ["-C", repositoryRoot, "merge-base", "--is-ancestor", ancestor, descendant], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function validateIdentity(
  receipt: Record<string, unknown>,
  environment: V1ValidationEnvironment,
  errors: string[],
) {
  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (receipt.receipt_type !== "v1_release") errors.push("receipt_type must be v1_release");
  if (receipt.release_identity !== contract.release_identity) errors.push("release_identity does not match cases.json");
  if (receipt.distribution_mode !== contract.distribution.mode) errors.push("distribution_mode must be local_adhoc");
  if (receipt.public_notarized !== contract.distribution.public_notarized) {
    errors.push("public_notarized must remain unsatisfied");
  }

  const candidate = typeof receipt.candidate_commit === "string" ? receipt.candidate_commit : "";
  const evidence = typeof receipt.evidence_commit === "string" ? receipt.evidence_commit : "";
  if (!gitCommitExists(environment.repositoryRoot, candidate)) {
    errors.push("candidate_commit does not resolve in Git");
  }
  if (candidate !== contract.release_candidate) errors.push("candidate_commit does not match the fixed release candidate");
  if (!gitCommitExists(environment.repositoryRoot, evidence)) {
    errors.push("evidence_commit does not resolve in Git");
  }
  if (gitCommitExists(environment.repositoryRoot, candidate)
    && !gitIsAncestor(environment.repositoryRoot, contract.base_commit, candidate)) {
    errors.push("Wave 7 base is not an ancestor of candidate_commit");
  }
  if (gitCommitExists(environment.repositoryRoot, candidate)
    && gitCommitExists(environment.repositoryRoot, evidence)
    && !gitIsAncestor(environment.repositoryRoot, candidate, evidence)) {
    errors.push("candidate_commit is not an ancestor of evidence_commit");
  }

  if (!isObject(receipt.target)) {
    errors.push("target is required");
  } else {
    rejectUnknownFields(receipt.target, TARGET_FIELDS, "target", errors);
    if (canonical(receipt.target) !== canonical(contract.target)
      || canonical(receipt.target) !== canonical(environment.target)) {
      errors.push("Target does not match the fixed release Mac");
    }
  }
  if (!isObject(receipt.versions)) {
    errors.push("versions are required");
  } else {
    rejectUnknownFields(receipt.versions, VERSION_FIELDS, "versions", errors);
    if (canonical(receipt.versions) !== canonical(contract.versions)) errors.push("versions do not match cases.json");
  }
  if (!isObject(receipt.action)) {
    errors.push("action is required");
  } else {
    rejectUnknownFields(receipt.action, ACTION_FIELDS, "action", errors);
    if (receipt.action.kind !== "command" || receipt.action.id !== "v1_release_qualification") {
      errors.push("action must be command:v1_release_qualification");
    }
  }
  if (receipt.expected_code !== "complete_v1_release_qualified") {
    errors.push("expected_code does not match the V1 contract");
  }
}

function parseHashRows(value: unknown, location: string, errors: string[]): HashRow[] {
  if (!Array.isArray(value)) {
    errors.push(`${location} must be an array`);
    return [];
  }
  const result: HashRow[] = [];
  const seen = new Set<string>();
  for (const row of value) {
    if (!isObject(row)) {
      errors.push(`${location} entries must be objects`);
      continue;
    }
    rejectUnknownFields(row, HASH_ROW_FIELDS, `${location} entry`, errors);
    if (typeof row.id !== "string" || typeof row.path !== "string" || typeof row.sha256 !== "string") {
      errors.push(`${location} entries require id, path, and sha256`);
      continue;
    }
    if (seen.has(row.id)) errors.push(`Duplicate ${location} id: ${row.id}`);
    seen.add(row.id);
    if (!SHA256.test(row.sha256)) errors.push(`${location} sha256 must be lowercase hexadecimal: ${row.id}`);
    result.push({ id: row.id, path: row.path, sha256: row.sha256 });
  }
  return result;
}

function validateHashRows(
  rows: HashRow[],
  expected: Array<{ id: string; path: string }>,
  root: string,
  label: string,
  errors: string[],
) {
  const expectedById = new Map(expected.map((item) => [item.id, item]));
  if (rows.length !== expected.length) errors.push(`${label} requires exactly one hash per declared file`);
  for (const row of rows) {
    const declaration = expectedById.get(row.id);
    if (!declaration) {
      errors.push(`Unknown ${label} id: ${row.id}`);
      continue;
    }
    if (row.path !== declaration.path) errors.push(`${label} path does not match: ${row.id}`);
    const resolved = resolveInside(root, declaration.path);
    if (!resolved || !existsSync(resolved)) {
      errors.push(`Declared ${label} is missing: ${row.id}`);
      continue;
    }
    if (SHA256.test(row.sha256) && hashFile(resolved) !== row.sha256) {
      errors.push(`On-disk ${label} hash does not match: ${row.id}`);
    }
  }
  for (const declaration of expected) {
    if (!rows.some((row) => row.id === declaration.id)) errors.push(`Missing ${label}: ${declaration.id}`);
  }
}

function validateBoundFiles(
  receipt: Record<string, unknown>,
  environment: V1ValidationEnvironment,
  errors: string[],
) {
  const artifacts = parseHashRows(receipt.artifacts, "artifact", errors);
  validateHashRows(artifacts, contract.artifacts, environment.artifactRoot, "artifact", errors);
  const locks = parseHashRows(receipt.locks, "lock", errors);
  validateHashRows(locks, contract.locks, environment.repositoryRoot, "lock", errors);
  const priorReceipts = parseHashRows(receipt.prior_receipts, "prior receipt", errors);
  validateHashRows(priorReceipts, contract.prior_receipts, environment.repositoryRoot, "prior receipt", errors);
  const evidenceFiles = parseHashRows(receipt.evidence_files, "evidence file", errors);
  const evidenceRoot = environment.evidenceRoot ?? environment.repositoryRoot;
  validateHashRows(evidenceFiles, contract.evidence_files, evidenceRoot, "evidence file", errors);

  const candidate = typeof receipt.candidate_commit === "string" ? receipt.candidate_commit : "";
  const application = contract.artifacts.find((artifact) => artifact.id === "application");
  const applicationRow = artifacts.find((artifact) => artifact.id === "application");
  if (application?.candidate_marker && applicationRow?.path === application.path && COMMIT.test(candidate)) {
    const applicationPath = resolveInside(environment.artifactRoot, application.path);
    if (applicationPath && existsSync(applicationPath)) {
      const marker = Buffer.from(`${application.candidate_marker}${candidate}`);
      if (!readFileSync(applicationPath).includes(marker)) errors.push("Application candidate marker does not match");
    }
  }

  if (gitCommitExists(environment.repositoryRoot, candidate)) {
    for (const declaration of contract.prior_receipts) {
      const priorPath = resolveInside(environment.repositoryRoot, declaration.path);
      if (!priorPath || !existsSync(priorPath)) continue;
      try {
        const prior = JSON.parse(readFileSync(priorPath, "utf8"));
        const priorCandidate = prior.candidate_commit;
        if (typeof priorCandidate !== "string"
          || !gitCommitExists(environment.repositoryRoot, priorCandidate)
          || !gitIsAncestor(environment.repositoryRoot, priorCandidate, candidate)) {
          errors.push(`Prior receipt candidate is not an ancestor: ${declaration.id}`);
        }
      } catch {
        errors.push(`Prior receipt is not valid JSON: ${declaration.id}`);
      }
    }
  }
  validateT028Evidence(receipt, artifacts, evidenceFiles, environment, errors);
}

function readJsonFile(filePath: string, label: string, errors: string[]): Record<string, unknown> | undefined {
  try {
    const value: unknown = JSON.parse(readFileSync(filePath, "utf8"));
    if (!isObject(value)) throw new Error("not an object");
    scanPrivateData(value, errors);
    return value;
  } catch {
    errors.push(`${label} is not valid JSON`);
    return undefined;
  }
}

function hashGitPath(repositoryRoot: string, commit: string, relativePath: string): string | undefined {
  try {
    const bytes = execFileSync("git", ["-C", repositoryRoot, "show", `${commit}:${relativePath}`]);
    return createHash("sha256").update(bytes).digest("hex");
  } catch {
    return undefined;
  }
}

function validateWorkloadEvidence(
  receipt: Record<string, unknown>,
  declaration: (typeof contract.evidence_files)[number],
  evidence: Record<string, unknown>,
  errors: string[],
) {
  if (declaration.kind !== "workload") return;
  const metrics = isObject(receipt.metrics) ? receipt.metrics : {};
  const mode = declaration.mode;
  if (evidence.schema_version !== 1 || evidence.mode !== mode) errors.push(`Workload evidence identity mismatch: ${declaration.id}`);
  if (!Array.isArray(evidence.samples) || evidence.samples.length !== evidence.sample_count) {
    errors.push(`Workload sample count mismatch: ${declaration.id}`);
  }
  const durationKey = mode === "capture" ? "capture_duration_seconds" : "import_duration_seconds";
  const rssKey = mode === "capture" ? "capture_rss_growth_mib" : "import_rss_growth_mib";
  if (metrics[durationKey] !== evidence.duration_seconds || metrics[rssKey] !== evidence.rss_growth_mib) {
    errors.push(`Workload metrics do not match evidence: ${declaration.id}`);
  }
  if (mode === "import" && metrics.import_peak_mb !== evidence.rss_peak_mb) {
    errors.push("Import peak does not match evidence");
  }
  if (evidence.maximum_socket_count !== 0 || canonical(metrics.thermal_states) !== canonical(evidence.thermal_states)) {
    errors.push(`Workload safety observations do not match evidence: ${declaration.id}`);
  }
  const energy = Array.isArray(metrics.energy)
    ? metrics.energy.find((row) => isObject(row) && row.case_id === `${mode}_energy`)
    : undefined;
  if (!isObject(energy)
    || energy.sample_count !== evidence.power_sample_count
    || energy.mean_power !== evidence.mean_power
    || energy.p95_power !== evidence.p95_power) {
    errors.push(`Workload energy does not match evidence: ${declaration.id}`);
  }
}

function validateInstallEvidence(
  receipt: Record<string, unknown>,
  declaration: (typeof contract.evidence_files)[number],
  evidence: Record<string, unknown>,
  packageEvidence: Record<string, unknown>,
  packageEvidenceSha: string,
  artifacts: HashRow[],
  errors: string[],
) {
  if (declaration.kind !== "install") return;
  const expectedCodes: Record<string, string> = {
    "install-clean": "clean_install_launched",
    "install-existing": "existing_data_preserved",
    rollback: "snapshot_restored_exactly",
    "rollback-interrupted": "rollback_retry_completed",
  };
  if (evidence.schema_version !== 1
    || evidence.distribution_mode !== contract.distribution.mode
    || evidence.candidate_commit !== receipt.candidate_commit
    || evidence.candidate_commit !== declaration.candidate_commit
    || evidence.case !== declaration.case
    || evidence.status !== "passed"
    || evidence.code !== expectedCodes[String(declaration.case)]) {
    errors.push(`Install evidence identity mismatch: ${declaration.id}`);
  }
  const packageValue = isObject(evidence.package) ? evidence.package : {};
  const installer = artifacts.find((row) => row.id === "installer");
  if (packageValue.manifest_sha256 !== packageEvidenceSha
    || packageValue.application_manifest_sha256 !== packageEvidence.application_tree_sha256
    || packageValue.dmg_sha256 !== packageEvidence.dmg_sha256
    || packageValue.dmg_sha256 !== installer?.sha256) {
    errors.push(`Install package identity mismatch: ${declaration.id}`);
  }
  const snapshot = isObject(evidence.snapshot) ? evidence.snapshot : {};
  const restored = isObject(evidence.restored) ? evidence.restored : {};
  for (const key of ["application_sha256", "data_sha256", "keychain_configuration_sha256"]) {
    if (snapshot[key] !== restored[key]) errors.push(`Install restoration mismatch: ${declaration.id}`);
  }
  if (snapshot.database_integrity !== "ok" || restored.database_integrity !== "ok") {
    errors.push(`Install database integrity mismatch: ${declaration.id}`);
  }
  const cleanup = isObject(evidence.cleanup) ? evidence.cleanup : {};
  for (const key of [
    "mounted_images", "candidate_processes", "helper_processes", "listeners",
    "temporary_keychains", "temporary_artifacts", "pending_journals",
  ]) {
    const count = isObject(cleanup[key]) ? cleanup[key] : {};
    if (count.before !== count.after) errors.push(`Install cleanup mismatch: ${declaration.id}:${key}`);
  }
  const recovery = isObject(evidence.recovery) ? evidence.recovery : {};
  const interrupted = declaration.case === "rollback-interrupted";
  if (recovery.occurred !== interrupted || (interrupted && recovery.candidate_commit !== receipt.candidate_commit)) {
    errors.push(`Install recovery mismatch: ${declaration.id}`);
  }
}

function validateT028Evidence(
  receipt: Record<string, unknown>,
  artifacts: HashRow[],
  evidenceRows: HashRow[],
  environment: V1ValidationEnvironment,
  errors: string[],
) {
  const evidenceRoot = environment.evidenceRoot ?? environment.repositoryRoot;
  const evidenceCommit = typeof receipt.evidence_commit === "string" ? receipt.evidence_commit : "";
  const documents = new Map<string, Record<string, unknown>>();
  for (const declaration of contract.evidence_files) {
    const row = evidenceRows.find((item) => item.id === declaration.id);
    const filePath = resolveInside(evidenceRoot, declaration.path);
    if (!row || !filePath || !existsSync(filePath)) continue;
    if (environment.evidenceRoot === undefined
      && hashGitPath(environment.repositoryRoot, evidenceCommit, declaration.path) !== row.sha256) {
      errors.push(`Evidence commit does not bind file: ${declaration.id}`);
    }
    const document = readJsonFile(filePath, `Evidence file ${declaration.id}`, errors);
    if (document) documents.set(declaration.id, document);
  }
  const packageDeclaration = contract.evidence_files.find((item) => item.kind === "package");
  const packageEvidence = packageDeclaration ? documents.get(packageDeclaration.id) : undefined;
  const packageRow = packageDeclaration ? evidenceRows.find((item) => item.id === packageDeclaration.id) : undefined;
  if (!packageDeclaration || !packageEvidence || !packageRow) return;
  const application = artifacts.find((row) => row.id === "application");
  const installer = artifacts.find((row) => row.id === "installer");
  const executable = Array.isArray(packageEvidence.application_entries)
    ? packageEvidence.application_entries.find((entry) => isObject(entry) && entry.path === "Contents/MacOS/gcrdings")
    : undefined;
  if (packageEvidence.candidate_commit !== receipt.candidate_commit
    || !isObject(executable)
    || executable.sha256 !== application?.sha256
    || packageEvidence.dmg_sha256 !== installer?.sha256) {
    errors.push("Release package evidence does not match receipt artifacts");
  }
  const installPackages = contract.evidence_files
    .filter((item) => item.kind === "install")
    .map((item) => documents.get(item.id)?.package)
    .filter(isObject);
  if (installPackages.length !== contract.evidence_files.filter((item) => item.kind === "install").length
    || installPackages.some((item) => canonical(item) !== canonical(installPackages[0]))) {
    errors.push("Install evidence package identities disagree");
  }
  for (const declaration of contract.evidence_files) {
    const document = documents.get(declaration.id);
    if (!document) continue;
    if (declaration.kind === "workload") validateWorkloadEvidence(receipt, declaration, document, errors);
    if (declaration.kind === "install") {
      validateInstallEvidence(receipt, declaration, document, packageEvidence, packageRow.sha256, artifacts, errors);
    }
  }
  if (contract.evidence_files.some((item) => item.kind === "workload"
    && item.candidate_commit !== contract.runtime_candidate)) {
    errors.push("Workload evidence candidate does not match the runtime candidate");
  }
  if (!gitIsAncestor(environment.repositoryRoot, contract.runtime_candidate, contract.release_candidate)) {
    errors.push("Runtime candidate is not an ancestor of the release candidate");
  }
}

function validateMetrics(receipt: Record<string, unknown>, errors: string[]) {
  if (canonical(receipt.thresholds) !== canonical(contract.thresholds)) {
    errors.push("Threshold contract does not match cases.json");
  }
  if (canonical(receipt.energy_proxy) !== canonical(contract.energy_proxy)) {
    errors.push("Energy proxy contract does not match cases.json");
  }
  if (!isObject(receipt.metrics)) {
    errors.push("metrics are required");
    return;
  }
  rejectUnknownFields(receipt.metrics, METRIC_FIELDS, "metrics", errors);
  const metrics = receipt.metrics;
  const numericFields = [...METRIC_FIELDS].filter((field) => !["thermal_states", "energy"].includes(field));
  for (const field of numericFields) {
    if (!isFiniteNonNegative(metrics[field])) errors.push(`metrics.${field} must be finite and non-negative`);
  }
  if (typeof metrics.capture_duration_seconds === "number"
    && metrics.capture_duration_seconds < contract.thresholds.capture_duration_seconds) {
    errors.push("Capture duration is below 30 minutes");
  }
  if (typeof metrics.import_duration_seconds === "number"
    && metrics.import_duration_seconds < contract.thresholds.import_duration_seconds) {
    errors.push("Import duration is below 30 minutes");
  }
  const maxima: Array<[string, number, string]> = [
    ["startup_ms", contract.thresholds.maximum_startup_ms, "Startup exceeds 15000 ms"],
    ["graceful_shutdown_ms", contract.thresholds.maximum_graceful_shutdown_ms, "Graceful shutdown exceeds 5000 ms"],
    ["render_10000_blocks_ms", contract.thresholds.maximum_10000_block_render_ms, "10,000-block render exceeds 5000 ms"],
    ["interaction_10000_blocks_p95_ms", contract.thresholds.maximum_10000_block_interaction_p95_ms, "10,000-block interaction p95 exceeds 200 ms"],
    ["capture_rss_growth_mib", contract.thresholds.maximum_rss_growth_mib, "Capture RSS growth exceeds 512 MiB"],
    ["import_rss_growth_mib", contract.thresholds.maximum_rss_growth_mib, "Import RSS growth exceeds 512 MiB"],
    ["import_peak_mb", contract.thresholds.maximum_import_peak_mb, "Import peak exceeds 1970 MB"],
    ["maximum_seek_error_ms", contract.thresholds.maximum_seek_error_ms, "Seek error exceeds 100 ms"],
  ];
  for (const [field, maximum, message] of maxima) {
    if (typeof metrics[field] === "number" && metrics[field] > maximum) errors.push(message);
  }
  if (!Array.isArray(metrics.thermal_states) || metrics.thermal_states.length === 0
    || metrics.thermal_states.some((state) => typeof state !== "string" || !["nominal", "fair"].includes(state))) {
    errors.push("Thermal state must never be serious or critical");
  }
  if (!Array.isArray(metrics.energy)) {
    errors.push("Energy measurements are required");
    return;
  }
  const expectedEnergyCases = new Set(["capture_energy", "import_energy"]);
  const seen = new Set<string>();
  for (const row of metrics.energy) {
    if (!isObject(row)) {
      errors.push("Energy measurements must be objects");
      continue;
    }
    rejectUnknownFields(row, ENERGY_FIELDS, "energy measurement", errors);
    const caseId = typeof row.case_id === "string" ? row.case_id : "";
    if (seen.has(caseId)) errors.push(`Duplicate energy measurement: ${caseId}`);
    seen.add(caseId);
    if (!expectedEnergyCases.has(caseId)) errors.push(`Unknown energy measurement: ${caseId}`);
    if (!isFiniteNonNegative(row.sample_count) || row.sample_count < contract.energy_proxy.minimum_samples) {
      errors.push(`Energy sample count is below ${contract.energy_proxy.minimum_samples}: ${caseId}`);
    }
    if (!isFiniteNonNegative(row.mean_power) || row.mean_power > contract.energy_proxy.maximum_mean_power) {
      errors.push(`Mean POWER exceeds fixed ceiling: ${caseId}`);
    }
    if (!isFiniteNonNegative(row.p95_power) || row.p95_power > contract.energy_proxy.maximum_p95_power) {
      errors.push(`p95 POWER exceeds fixed ceiling: ${caseId}`);
    }
  }
  for (const caseId of expectedEnergyCases) {
    if (!seen.has(caseId)) errors.push(`Missing energy measurement: ${caseId}`);
  }
}

function validateObservations(receipt: Record<string, unknown>, errors: string[]) {
  if (!isObject(receipt.observations)) {
    errors.push("observations are required");
    return;
  }
  rejectUnknownFields(receipt.observations, OBSERVATION_FIELDS, "observations", errors);
  if (receipt.observations.unapproved_network_requests !== 0) {
    errors.push("Unapproved network requests must be zero");
  }
  if (receipt.observations.diagnostic_private_matches !== 0) {
    errors.push("Diagnostic private matches must be zero");
  }
  if (!Array.isArray(receipt.observations.safety_failures)
    || receipt.observations.safety_failures.length !== 0) {
    errors.push("Safety failures must be empty");
  }
}

function validateCases(receipt: Record<string, unknown>, errors: string[]) {
  if (!Array.isArray(receipt.cases)) {
    errors.push("cases must be an array");
    return;
  }
  const expectedById = new Map(contract.cases.map((item) => [item.id, item]));
  const seen = new Set<string>();
  let hasIncomplete = false;
  for (const value of receipt.cases) {
    if (!isObject(value)) {
      errors.push("case entries must be objects");
      hasIncomplete = true;
      continue;
    }
    rejectUnknownFields(value, CASE_FIELDS, "case", errors);
    const id = typeof value.id === "string" ? value.id : "";
    if (seen.has(id)) errors.push(`Duplicate case id: ${id}`);
    seen.add(id);
    const expected = expectedById.get(id);
    if (!expected) {
      errors.push(`Unknown case id: ${id}`);
      hasIncomplete = true;
      continue;
    }
    if (value.command !== expected.command) errors.push(`Case ${id} command does not match cases.json`);
    if (value.expected_code !== expected.expected_code) errors.push(`Case ${id} expected_code does not match cases.json`);
    if (!isObject(value.evidence)) {
      errors.push(`Case ${id} evidence is required`);
    } else {
      rejectUnknownFields(value.evidence, CASE_EVIDENCE_FIELDS, "case evidence", errors);
    }

    if (expected.required_status === "unsatisfied") {
      if (value.status !== "unsatisfied" || value.observed_code !== expected.expected_code) {
        errors.push(`Case ${id} must remain unsatisfied`);
      }
      continue;
    }
    if (value.status === "expected") {
      errors.push(`Case ${id} has not been executed`);
      hasIncomplete = true;
      continue;
    }
    if (value.status !== "passed") {
      errors.push(`Case ${id} did not pass`);
      hasIncomplete = true;
      continue;
    }
    if (value.observed_code !== expected.expected_code) errors.push(`Case ${id} observed_code does not match`);
    if (isObject(value.evidence)) {
      if (value.evidence.unapproved_network_requests !== 0) {
        errors.push(`Case ${id} has unapproved network requests`);
      }
      if (value.evidence.private_matches !== 0) errors.push(`Case ${id} has private matches`);
      if (!Array.isArray(value.evidence.safety_failures) || value.evidence.safety_failures.length !== 0) {
        errors.push(`Case ${id} has safety failures`);
      }
    }
  }
  for (const expected of contract.cases) {
    if (!seen.has(expected.id)) {
      errors.push(`Missing required case: ${expected.id}`);
      if (expected.required_status === "passed") hasIncomplete = true;
    }
  }
  if (receipt.outcome === "passed") {
    if (hasIncomplete) errors.push("Passed receipt contains failed or unexecuted cases");
    if (receipt.observed_code !== "complete_v1_release_qualified") {
      errors.push("Passed receipt has incompatible observed_code");
    }
  } else if (receipt.outcome === "expected") {
    if (receipt.observed_code !== "awaiting_t028") errors.push("Expected receipt must await T028");
  } else {
    errors.push("outcome must be passed or expected");
  }
}

export function validateV1Receipt(
  value: unknown,
  suppliedEnvironment?: V1ValidationEnvironment,
): string[] {
  const errors: string[] = [];
  if (!isObject(value)) return ["Receipt must be an object"];
  const environment = suppliedEnvironment ?? defaultEnvironment();
  rejectUnknownFields(value, TOP_LEVEL_FIELDS, "top-level", errors);
  scanPrivateData(value, errors);
  validateIdentity(value, environment, errors);
  validateBoundFiles(value, environment, errors);
  validateMetrics(value, errors);
  validateObservations(value, errors);
  validateCases(value, errors);
  return [...new Set(errors)];
}

function main() {
  const receiptPath = process.argv[2];
  if (!receiptPath) {
    console.error("Usage: bun qualification/v1/validate.ts <receipt.json>");
    process.exit(2);
  }
  let receipt: unknown;
  try {
    receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
  } catch {
    console.error("V1 release receipt is not readable JSON");
    process.exit(1);
  }
  const errors = validateV1Receipt(receipt);
  if (errors.length > 0) {
    errors.forEach((error) => console.error(error));
    process.exit(1);
  }
  console.log("V1 release receipt valid");
}

if (import.meta.main) main();
