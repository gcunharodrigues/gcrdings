import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import audioCorpusManifest from "./audio-corpus/manifest.json";

const FORBIDDEN_FIELDS = new Set([
  "api_key",
  "apikey",
  "access_token",
  "auth_token",
  "audio",
  "audio_base64",
  "credential",
  "credentials",
  "client_secret",
  "openai_api_key",
  "password",
  "participant_name",
  "participant_names",
  "payload",
  "private_recording",
  "prompt",
  "refresh_token",
  "secret",
  "session_content",
  "source_path",
  "transcript",
  "token",
]);

const ALLOWED_TOP_LEVEL = new Set([
  "schema_version",
  "receipt_type",
  "candidate_commit",
  "release_identity",
  "target",
  "versions",
  "action",
  "expected_code",
  "observed_code",
  "outcome",
  "metrics",
  "observations",
  "artifacts",
  "thresholds",
  "cases",
]);

const AUDIO_CORPUS_CASES = new Set([
  "portuguese_clear",
  "english_clear",
  "code_switch_clear",
  "portuguese_noise",
  "two_speaker_overlap",
  "three_speaker_sequence",
  "long_form_repeat",
  "silence_boundary",
  "missing_audio",
  "malformed_transcript",
  "model_failure",
  "interrupted_run",
]);
const AUDIO_CORPUS_SAMPLE_CASES = new Set([
  "portuguese_clear",
  "english_clear",
  "code_switch_clear",
  "portuguese_noise",
  "two_speaker_overlap",
  "three_speaker_sequence",
  "long_form_repeat",
  "silence_boundary",
]);
const AUDIO_CORPUS_VERSION_FIELDS = new Set(["corpus", "engine"]);
const CAPTURED_TIMELINE_CASES = new Set([
  "microphone_only",
  "system_only",
  "both_origins",
  "silence",
  "overlap",
  "ambiguous_duplicate",
  "source_loss",
  "retry",
  "timestamp_seek",
]);
const CAPTURED_TIMELINE_ORIGINS: Record<string, string[]> = {
  microphone_only: ["microphone"],
  system_only: ["system-audio"],
  both_origins: ["microphone", "system-audio"],
  silence: ["system-audio"],
  overlap: ["microphone", "system-audio"],
  ambiguous_duplicate: ["microphone", "system-audio"],
  source_loss: ["microphone", "system-audio"],
  retry: ["microphone", "system-audio"],
  timestamp_seek: ["microphone", "system-audio"],
};
const LOCAL_FINDINGS_CASES = new Set([
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
]);
const LOCAL_FINDINGS_GENERATED_CASES = new Set(["meeting", "interview", "content"]);
const LOCAL_FINDINGS_CANDIDATE = "b3b2f82919c3747b71b698451f41eb3f86b57fcf";
const LOCAL_FINDINGS_RELEASE = "gcrdings-0.4.0-aarch64-ad-hoc";
const LOCAL_FINDINGS_ARTIFACT_HASHES: Record<string, string> = {
  application: "045f2dfff039cf7cd384845f46159b04a93f64a62cc4e9a0aba4038f98b15076",
  foundation_helper: "16667dfdfa6a512b75ad27f256f7943fe2250ffcc47f3a1c1f0b33b3f9f22a6c",
  installer: "f11249f26dccee911d255bd4eae1a9877cd60f67e0275f1df09a10528d020253",
};
const LOCAL_FINDINGS_OBSERVED_CODES: Record<string, string> = {
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
const AGENT_HANDOFF_CANDIDATE = "611f1dd567c3b2694c665eb6b36d76f54588e587";
const AGENT_HANDOFF_RELEASE = "gcrdings-0.4.0-aarch64-ad-hoc";
const AGENT_HANDOFF_TARGET = { hardware: "Apple-Silicon", os: "macOS-26.5.2" } as const;
const AGENT_HANDOFF_ARTIFACT_HASHES: Record<string, string> = {
  application: "7d5c18f0a709b1b4da559a7ef2b96dfcc2f38dfbe3faae12779c65d0bb1dd26a",
  installer: "29469315ed411655420b63c529155f0a553a76a67281ada33950bdb94c027355",
  exported_markdown: "bc02fd3479547bdf5074e2f3e9880c71512e126a0d3b50c6ced2c1147a0c4e95",
  exported_json: "5f003b45ada3c6f1c5dabb3af7881bfa4d702b21b3affad26906674ecb11019d",
};
const AGENT_HANDOFF_OBSERVED_CODES: Record<string, string> = {
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
const PROVIDER_AUTHORIZATION_CANDIDATE = "069d1079294eca80b3659183f3e0aa421b1c51a6";
const PROVIDER_AUTHORIZATION_RELEASE = "gcrdings-0.4.0-aarch64-ad-hoc";
const PROVIDER_AUTHORIZATION_TARGET = {
  model: "Mac15,12",
  chip: "Apple M3",
  memory: "24 GiB",
  arch: "arm64",
  os: "macOS-26.5.2",
  build: "25F84",
} as const;
const PROVIDER_AUTHORIZATION_ARTIFACT_PATHS: Record<string, string> = {
  application: "release/bundle/macos/gcrdings.app/Contents/MacOS/gcrdings",
  installer: "release/bundle/dmg/gcrdings_0.4.0_aarch64.dmg",
};
const PROVIDER_AUTHORIZATION_ARTIFACT_HASHES: Record<string, string> = {
  application: "20f3262a62762551af1438a4eddba9ecd446cacd128f7574c4043e5b53449fc2",
  installer: "ee03bd28a770b82dbfdc71b1c0022e269829a592a8fd742d7e295b451ab466db",
};
const PROVIDER_AUTHORIZATION_OBSERVED_CODES: Record<string, string> = {
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

type ProviderAuthorizationEnvironment = {
  artifactRoot: string;
  candidateCommit: string;
  artifactHashes: Record<string, string>;
  target: Record<keyof typeof PROVIDER_AUTHORIZATION_TARGET, string>;
};

type ValidationOptions = {
  providerAuthorization?: ProviderAuthorizationEnvironment;
};

function command(command: string, args: string[]): string {
  return execFileSync(command, args, { encoding: "utf8" }).trim();
}

function providerArtifactRoot(): string {
  const candidates = [
    process.env.CARGO_TARGET_DIR,
    path.resolve("target"),
    path.resolve("../target"),
    path.resolve("../../target"),
  ].filter((candidate): candidate is string => Boolean(candidate));
  return candidates.find((candidate) => existsSync(path.join(candidate, PROVIDER_AUTHORIZATION_ARTIFACT_PATHS.application)))
    ?? candidates[0];
}

function providerEnvironment(): ProviderAuthorizationEnvironment {
  const memoryBytes = Number(command("sysctl", ["-n", "hw.memsize"]));
  return {
    artifactRoot: providerArtifactRoot(),
    candidateCommit: PROVIDER_AUTHORIZATION_CANDIDATE,
    artifactHashes: PROVIDER_AUTHORIZATION_ARTIFACT_HASHES,
    target: {
      model: command("sysctl", ["-n", "hw.model"]),
      chip: command("sysctl", ["-n", "machdep.cpu.brand_string"]),
      memory: `${Math.round(memoryBytes / 1024 ** 3)} GiB`,
      arch: command("uname", ["-m"]),
      os: `macOS-${command("sw_vers", ["-productVersion"])}`,
      build: command("sw_vers", ["-buildVersion"]),
    },
  };
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isFiniteNonNegative(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}

function containsExactly(value: unknown, expected?: string[]) {
  return Array.isArray(expected)
    && Array.isArray(value)
    && value.length === expected.length
    && new Set(value).size === expected.length
    && expected.every((item) => value.includes(item));
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

function scanPrivateData(value: unknown, location: string, errors: string[]) {
  if (Array.isArray(value)) {
    value.forEach((item, index) => scanPrivateData(item, `${location}[${index}]`, errors));
    return;
  }
  if (isObject(value)) {
    for (const [key, child] of Object.entries(value)) {
      const normalized = key
        .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
        .toLocaleLowerCase("en-US")
        .replaceAll("-", "_");
      if (FORBIDDEN_FIELDS.has(normalized)) errors.push(`Forbidden private field: ${key}`);
      scanPrivateData(child, location ? `${location}.${key}` : key, errors);
    }
    return;
  }
  if (typeof value !== "string") return;
  if (/^(?:\/Users\/|\/private\/|[A-Za-z]:\\)/.test(value)) {
    errors.push(`Potential private path in ${location}`);
  }
  if (/\bBearer\s+\S+|\bsk-[A-Za-z0-9_-]{8,}|-----BEGIN [A-Z ]+ PRIVATE KEY-----/i.test(value)) {
    errors.push(`Potential credential in ${location}`);
  }
  if (/\b[^\s@]+@[^\s@]+\.[^\s@]+\b/.test(value)) {
    errors.push(`Potential personal email in ${location}`);
  }
  if (value.length > 160 || /[\r\n]/.test(value)) {
    errors.push(`Receipt text must be a short redacted identifier in ${location}`);
  }
}

function validateLocalFindings(receipt: Record<string, unknown>, errors: string[]) {
  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (typeof receipt.candidate_commit !== "string" || !/^[a-f0-9]{40}$/i.test(receipt.candidate_commit)) {
    errors.push("candidate_commit must be a 40-character hexadecimal commit");
  }
  if (typeof receipt.release_identity !== "string" || receipt.release_identity.length === 0) {
    errors.push("release_identity is required");
  }
  if (receipt.candidate_commit !== LOCAL_FINDINGS_CANDIDATE
    || receipt.release_identity !== LOCAL_FINDINGS_RELEASE) {
    errors.push("local_findings candidate identity does not match");
  }

  if (!isObject(receipt.target)) {
    errors.push("target must name hardware and OS");
  } else {
    rejectUnknownFields(receipt.target, new Set(["hardware", "os"]), "target", errors);
    if (typeof receipt.target.hardware !== "string" || typeof receipt.target.os !== "string") {
      errors.push("target must name hardware and OS");
    }
  }

  const versionFields = new Set(["helper", "framework", "prompt_version"]);
  if (!isObject(receipt.versions)) {
    errors.push("local_findings versions are required");
  } else {
    rejectUnknownFields(receipt.versions, versionFields, "versions", errors);
    for (const key of versionFields) {
      if (typeof receipt.versions[key] !== "string" || receipt.versions[key].length === 0) {
        errors.push(`versions.${key} is required`);
      }
    }
  }

  if (!isObject(receipt.action)) {
    errors.push("local_findings requires the native qualification action");
  } else {
    rejectUnknownFields(receipt.action, new Set(["kind", "id"]), "action", errors);
    if (receipt.action.kind !== "ui" || receipt.action.id !== "local_findings_qualification") {
      errors.push("local_findings requires the native qualification action");
    }
  }
  if (receipt.expected_code !== "grounded_local_findings") {
    errors.push("local_findings has incompatible expected_code");
  }
  if (receipt.observed_code !== "local_findings_passed") {
    errors.push("local_findings has incompatible observed_code");
  }
  if (receipt.outcome !== "passed") errors.push("local_findings outcome must be passed");

  if (!isObject(receipt.metrics)) {
    errors.push("local_findings metrics are required");
  } else {
    rejectUnknownFields(
      receipt.metrics,
      new Set(["duration_ms", "peak_memory_mb", "evidence_resolution_rate"]),
      "metrics",
      errors,
    );
    if (!isFiniteNonNegative(receipt.metrics.duration_ms)
      || !isFiniteNonNegative(receipt.metrics.peak_memory_mb)) {
      errors.push("local_findings metrics must be finite and non-negative");
    }
    if (receipt.metrics.evidence_resolution_rate !== 1) {
      errors.push("local_findings requires complete evidence resolution");
    }
  }

  if (!isObject(receipt.observations)) {
    errors.push("local_findings safety observations are required");
  } else {
    rejectUnknownFields(
      receipt.observations,
      new Set(["network_requests", "diagnostic_private_content_matches", "safety_failures"]),
      "observations",
      errors,
    );
    if (receipt.observations.network_requests !== 0
      || receipt.observations.diagnostic_private_content_matches !== 0
      || !Array.isArray(receipt.observations.safety_failures)
      || receipt.observations.safety_failures.length !== 0) {
      errors.push("local_findings requires zero safety observations");
    }
  }

  const artifactIds: string[] = [];
  if (Array.isArray(receipt.artifacts)) {
    receipt.artifacts.forEach((artifact, index) => {
      if (!isObject(artifact)
        || typeof artifact.id !== "string"
        || typeof artifact.sha256 !== "string"
        || !/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
        errors.push(`artifacts[${index}] must contain an identifier and SHA-256`);
        return;
      }
      rejectUnknownFields(artifact, new Set(["id", "sha256"]), `artifacts[${index}]`, errors);
      artifactIds.push(artifact.id);
      if (artifact.sha256 !== LOCAL_FINDINGS_ARTIFACT_HASHES[artifact.id]) {
        errors.push(`local_findings artifact hash does not match: ${artifact.id}`);
      }
    });
  }
  if (!containsExactly(artifactIds, ["application", "foundation_helper", "installer"])) {
    errors.push("local_findings requires exactly one hash per artifact");
  }

  if (!isObject(receipt.thresholds)) {
    errors.push("local_findings fixed evidence threshold is required");
  } else {
    rejectUnknownFields(receipt.thresholds, new Set(["source", "values", "failures"]), "thresholds", errors);
    if (receipt.thresholds.source !== "fixed_acceptance"
      || !Array.isArray(receipt.thresholds.failures)
      || receipt.thresholds.failures.length !== 0
      || !isObject(receipt.thresholds.values)
      || receipt.thresholds.values.minimum_evidence_resolution_rate !== 1) {
      errors.push("local_findings fixed evidence threshold is required");
    } else {
      rejectUnknownFields(
        receipt.thresholds.values,
        new Set(["minimum_evidence_resolution_rate"]),
        "thresholds.values",
        errors,
      );
    }
  }

  const caseIds = new Set<string>();
  if (!Array.isArray(receipt.cases)) {
    errors.push("local_findings cases are required");
  } else {
    receipt.cases.forEach((item, index) => {
      if (!isObject(item)
        || typeof item.id !== "string"
        || item.category !== item.id
        || item.status !== "passed"
        || !isObject(item.evidence)) {
        errors.push(`cases[${index}] must contain local findings evidence`);
        return;
      }
      rejectUnknownFields(item, new Set(["id", "category", "status", "evidence"]), `cases[${index}]`, errors);
      if (caseIds.has(item.id)) errors.push(`cases contains duplicate id: ${item.id}`);
      caseIds.add(item.id);
      rejectUnknownFields(
        item.evidence,
        new Set(["observed_code", "principal_unchanged", "network_requests", "evidence_resolution_rate"]),
        `cases[${index}].evidence`,
        errors,
      );
      if (typeof item.evidence.observed_code !== "string"
        || !/^[a-z][a-z0-9_]{2,95}$/.test(item.evidence.observed_code)
        || item.evidence.principal_unchanged !== true
        || item.evidence.network_requests !== 0) {
        errors.push(`local_findings case ${item.id} has invalid safety evidence`);
      }
      if (item.evidence.observed_code !== LOCAL_FINDINGS_OBSERVED_CODES[item.id]) {
        errors.push(`local_findings case ${item.id} has incompatible observed_code`);
      }
      if (LOCAL_FINDINGS_GENERATED_CASES.has(item.id)
        && item.evidence.evidence_resolution_rate !== 1) {
        errors.push(`local_findings case ${item.id} has unresolved evidence`);
      }
    });
  }
  for (const id of LOCAL_FINDINGS_CASES) {
    if (!caseIds.has(id)) errors.push(`local_findings is missing required case: ${id}`);
  }
  if (caseIds.size !== LOCAL_FINDINGS_CASES.size) {
    errors.push("local_findings passed receipt must contain only required cases");
  }
}

function validateCapturedTimeline(receipt: Record<string, unknown>, errors: string[]) {
  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (typeof receipt.candidate_commit !== "string" || !/^[a-f0-9]{40}$/i.test(receipt.candidate_commit)) {
    errors.push("candidate_commit must be a 40-character hexadecimal commit");
  }
  if (typeof receipt.release_identity !== "string" || receipt.release_identity.length === 0) {
    errors.push("release_identity is required");
  }

  if (!isObject(receipt.target)) {
    errors.push("target must name hardware and OS");
  } else {
    rejectUnknownFields(receipt.target, new Set(["hardware", "os"]), "target", errors);
    if (typeof receipt.target.hardware !== "string" || typeof receipt.target.os !== "string") {
      errors.push("target must name hardware and OS");
    }
  }

  const versionFields = new Set(["capture", "transcription", "diarization"]);
  if (!isObject(receipt.versions)) {
    errors.push("captured_timeline versions are required");
  } else {
    rejectUnknownFields(receipt.versions, versionFields, "versions", errors);
    for (const key of versionFields) {
      if (typeof receipt.versions[key] !== "string" || receipt.versions[key].length === 0) {
        errors.push(`versions.${key} is required`);
      }
    }
  }

  if (!isObject(receipt.action)) {
    errors.push("captured_timeline passed receipt requires the qualification command");
  } else {
    rejectUnknownFields(receipt.action, new Set(["kind", "id"]), "action", errors);
    if (receipt.action.kind !== "command" || receipt.action.id !== "captured_timeline_qualification") {
      errors.push("captured_timeline passed receipt requires the qualification command");
    }
  }
  if (receipt.expected_code !== "captured_origins_preserve_timeline") {
    errors.push("captured_timeline passed receipt has incompatible expected_code");
  }
  if (receipt.observed_code !== "captured_timeline_passed") {
    errors.push("captured_timeline passed receipt has incompatible observed_code");
  }
  if (receipt.outcome !== "passed") errors.push("captured_timeline outcome must be passed");

  if (!isObject(receipt.metrics)) {
    errors.push("captured_timeline metrics are required");
  } else {
    rejectUnknownFields(
      receipt.metrics,
      new Set(["duration_ms", "peak_memory_mb", "maximum_seek_error_ms"]),
      "metrics",
      errors,
    );
    if (!isFiniteNonNegative(receipt.metrics.duration_ms)
      || !isFiniteNonNegative(receipt.metrics.peak_memory_mb)
      || !isFiniteNonNegative(receipt.metrics.maximum_seek_error_ms)) {
      errors.push("captured_timeline metrics must be finite and non-negative");
    } else if (receipt.metrics.maximum_seek_error_ms > 100) {
      errors.push("captured_timeline seek error exceeds 100 ms");
    }
  }

  if (!isObject(receipt.observations)) {
    errors.push("captured_timeline safety observations are required");
  } else {
    rejectUnknownFields(
      receipt.observations,
      new Set(["network_requests", "diagnostic_private_content_matches", "safety_failures"]),
      "observations",
      errors,
    );
    if (receipt.observations.network_requests !== 0
      || receipt.observations.diagnostic_private_content_matches !== 0
      || !Array.isArray(receipt.observations.safety_failures)
      || receipt.observations.safety_failures.length !== 0) {
      errors.push("captured_timeline passed receipt requires zero safety observations");
    }
  }

  const artifactIds = new Set<string>();
  if (Array.isArray(receipt.artifacts)) {
    receipt.artifacts.forEach((artifact, index) => {
      if (!isObject(artifact)
        || typeof artifact.id !== "string"
        || typeof artifact.sha256 !== "string"
        || !/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
        errors.push(`artifacts[${index}] must contain an identifier and SHA-256`);
        return;
      }
      rejectUnknownFields(artifact, new Set(["id", "sha256"]), `artifacts[${index}]`, errors);
      artifactIds.add(artifact.id);
    });
  }
  if (artifactIds.size !== 3
    || !artifactIds.has("microphone_source")
    || !artifactIds.has("system_source")
    || !artifactIds.has("mixed_source")) {
    errors.push("captured_timeline requires microphone, system, and mixed source hashes");
  }

  if (!isObject(receipt.thresholds)) {
    errors.push("captured_timeline fixed seek threshold is required");
  } else {
    rejectUnknownFields(receipt.thresholds, new Set(["source", "values", "failures"]), "thresholds", errors);
    const values = receipt.thresholds.values;
    if (receipt.thresholds.source !== "fixed_acceptance"
      || !Array.isArray(receipt.thresholds.failures)
      || receipt.thresholds.failures.length !== 0
      || !isObject(values)
      || values.maximum_seek_error_ms !== 100) {
      errors.push("captured_timeline fixed seek threshold is required");
    } else {
      rejectUnknownFields(values, new Set(["maximum_seek_error_ms"]), "thresholds.values", errors);
    }
  }

  const caseIds = new Set<string>();
  let maximumCaseSeekError = 0;
  if (!Array.isArray(receipt.cases)) {
    errors.push("captured_timeline cases are required");
  } else {
    receipt.cases.forEach((item, index) => {
      if (!isObject(item)
        || typeof item.id !== "string"
        || item.category !== item.id
        || item.status !== "passed"
        || !isObject(item.evidence)) {
        errors.push(`cases[${index}] must contain captured timeline evidence`);
        return;
      }
      rejectUnknownFields(item, new Set(["id", "category", "status", "evidence"]), `cases[${index}]`, errors);
      if (caseIds.has(item.id)) errors.push(`cases contains duplicate id: ${item.id}`);
      caseIds.add(item.id);

      const evidence = item.evidence;
      rejectUnknownFields(
        evidence,
        new Set([
          "source_origins",
          "passage_count",
          "source_hash_unchanged",
          "retry_safe",
          "seek_error_ms",
          "ambiguity_preserved",
          "failed_origins",
        ]),
        `cases[${index}].evidence`,
        errors,
      );
      if (!containsExactly(evidence.source_origins, CAPTURED_TIMELINE_ORIGINS[item.id])) {
        errors.push(`captured_timeline case ${item.id} has invalid source origins`);
      }
      if (!Number.isInteger(evidence.passage_count)
        || Number(evidence.passage_count) < 0
        || evidence.source_hash_unchanged !== true
        || evidence.retry_safe !== true) {
        errors.push(`captured_timeline case ${item.id} has invalid provenance evidence`);
      }
      if (item.id === "silence" && evidence.passage_count !== 0) {
        errors.push("captured_timeline silence case must have zero passages");
      }
      if (item.id !== "silence" && Number(evidence.passage_count) < 1) {
        errors.push(`captured_timeline case ${item.id} must preserve a passage`);
      }
      if (item.id === "timestamp_seek"
        && (!isFiniteNonNegative(evidence.seek_error_ms) || evidence.seek_error_ms > 100)) {
        errors.push("captured_timeline timestamp seek case exceeds 100 ms");
      } else if (isFiniteNonNegative(evidence.seek_error_ms)) {
        maximumCaseSeekError = Math.max(maximumCaseSeekError, evidence.seek_error_ms);
      }
      if ((item.id === "overlap" || item.id === "ambiguous_duplicate")
        && evidence.ambiguity_preserved !== true) {
        errors.push(`captured_timeline case ${item.id} must preserve ambiguity`);
      }
      if (item.id === "source_loss") {
        const failedOrigins = evidence.failed_origins;
        const sourceOrigins = Array.isArray(evidence.source_origins) ? evidence.source_origins : [];
        if (!Array.isArray(failedOrigins)
          || failedOrigins.length !== 1
          || new Set(failedOrigins).size !== 1
          || !failedOrigins.every((origin) => sourceOrigins.includes(origin))) {
          errors.push("captured_timeline source loss has invalid failed origins");
        }
      }
    });
  }
  if (isObject(receipt.metrics)
    && isFiniteNonNegative(receipt.metrics.maximum_seek_error_ms)
    && receipt.metrics.maximum_seek_error_ms !== maximumCaseSeekError) {
    errors.push("captured_timeline maximum seek metric must equal case maximum");
  }
  for (const id of CAPTURED_TIMELINE_CASES) {
    if (!caseIds.has(id)) errors.push(`captured_timeline is missing required case: ${id}`);
  }
  if (caseIds.size !== CAPTURED_TIMELINE_CASES.size) {
    errors.push("captured_timeline passed receipt must contain only required cases");
  }
}

function validateAgentHandoff(receipt: Record<string, unknown>, errors: string[]) {
  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (receipt.candidate_commit !== AGENT_HANDOFF_CANDIDATE
    || receipt.release_identity !== AGENT_HANDOFF_RELEASE) {
    errors.push("agent_handoff candidate identity does not match");
  }

  if (!isObject(receipt.target)) {
    errors.push("target must name hardware and OS");
  } else {
    rejectUnknownFields(receipt.target, new Set(["hardware", "os"]), "target", errors);
    if (typeof receipt.target.hardware !== "string" || typeof receipt.target.os !== "string") {
      errors.push("target must name hardware and OS");
    } else if (receipt.target.hardware !== AGENT_HANDOFF_TARGET.hardware
      || receipt.target.os !== AGENT_HANDOFF_TARGET.os) {
      errors.push("agent_handoff target does not match");
    }
  }

  if (!isObject(receipt.versions)) {
    errors.push("agent_handoff versions are required");
  } else {
    rejectUnknownFields(receipt.versions, new Set(["serializer", "app"]), "versions", errors);
    if (receipt.versions.serializer !== "agent-handoff-v1" || receipt.versions.app !== "0.4.0") {
      errors.push("agent_handoff versions do not match");
    }
  }

  if (!isObject(receipt.action)
    || receipt.action.kind !== "ui"
    || receipt.action.id !== "agent_handoff_qualification") {
    errors.push("agent_handoff requires the native qualification action");
  } else {
    rejectUnknownFields(receipt.action, new Set(["kind", "id"]), "action", errors);
  }
  if (receipt.expected_code !== "equivalent_explicit_handoff"
    || receipt.observed_code !== "agent_handoff_passed"
    || receipt.outcome !== "passed") {
    errors.push("agent_handoff outcome is incompatible");
  }

  if (!isObject(receipt.metrics)) {
    errors.push("agent_handoff metrics are required");
  } else {
    rejectUnknownFields(receipt.metrics, new Set([
      "semantic_equivalence_rate", "forbidden_field_matches", "temporary_artifact_count",
    ]), "metrics", errors);
    if (receipt.metrics.semantic_equivalence_rate !== 1
      || receipt.metrics.forbidden_field_matches !== 0
      || receipt.metrics.temporary_artifact_count !== 0) {
      errors.push("agent_handoff requires equivalent clean outputs without temporary artifacts");
    }
  }

  if (!isObject(receipt.observations)) {
    errors.push("agent_handoff observations are required");
  } else {
    rejectUnknownFields(receipt.observations, new Set([
      "session_changed", "network_requests", "safety_failures",
    ]), "observations", errors);
    if (receipt.observations.session_changed !== false
      || receipt.observations.network_requests !== 0
      || !Array.isArray(receipt.observations.safety_failures)
      || receipt.observations.safety_failures.length !== 0) {
      errors.push("agent_handoff requires unchanged Session and zero network requests");
    }
  }

  const artifactIds: string[] = [];
  if (Array.isArray(receipt.artifacts)) {
    receipt.artifacts.forEach((artifact, index) => {
      if (!isObject(artifact)
        || typeof artifact.id !== "string"
        || typeof artifact.sha256 !== "string"
        || !/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
        errors.push(`artifacts[${index}] must contain an identifier and SHA-256`);
        return;
      }
      rejectUnknownFields(artifact, new Set(["id", "sha256"]), `artifacts[${index}]`, errors);
      artifactIds.push(artifact.id);
      if (artifact.sha256 !== AGENT_HANDOFF_ARTIFACT_HASHES[artifact.id]) {
        errors.push(`agent_handoff artifact hash does not match: ${artifact.id}`);
      }
    });
  }
  if (!containsExactly(artifactIds, Object.keys(AGENT_HANDOFF_ARTIFACT_HASHES))) {
    errors.push("agent_handoff requires exactly one hash per artifact");
  }

  if (!isObject(receipt.thresholds)) {
    errors.push("agent_handoff fixed thresholds are required");
  } else {
    rejectUnknownFields(receipt.thresholds, new Set(["source", "values", "failures"]), "thresholds", errors);
    const values = receipt.thresholds.values;
    if (receipt.thresholds.source !== "fixed_acceptance"
      || !Array.isArray(receipt.thresholds.failures)
      || receipt.thresholds.failures.length !== 0
      || !isObject(values)
      || values.minimum_semantic_equivalence_rate !== 1
      || values.maximum_forbidden_field_matches !== 0
      || values.maximum_temporary_artifacts !== 0) {
      errors.push("agent_handoff fixed thresholds are required");
    } else {
      rejectUnknownFields(values, new Set([
        "minimum_semantic_equivalence_rate", "maximum_forbidden_field_matches", "maximum_temporary_artifacts",
      ]), "thresholds.values", errors);
    }
  }

  const caseIds = new Set<string>();
  if (!Array.isArray(receipt.cases)) {
    errors.push("agent_handoff cases are required");
  } else {
    receipt.cases.forEach((item, index) => {
      if (!isObject(item)
        || typeof item.id !== "string"
        || item.category !== item.id
        || item.status !== "passed"
        || !isObject(item.evidence)) {
        errors.push(`cases[${index}] must contain Agent Handoff evidence`);
        return;
      }
      rejectUnknownFields(item, new Set(["id", "category", "status", "evidence"]), `cases[${index}]`, errors);
      if (caseIds.has(item.id)) errors.push(`cases contains duplicate id: ${item.id}`);
      caseIds.add(item.id);
      rejectUnknownFields(item.evidence, new Set([
        "observed_code", "session_unchanged", "temporary_artifacts",
      ]), `cases[${index}].evidence`, errors);
      if (item.evidence.observed_code !== AGENT_HANDOFF_OBSERVED_CODES[item.id]) {
        errors.push(`agent_handoff case ${item.id} has incompatible observed_code`);
      }
      if (item.evidence.session_unchanged !== true || item.evidence.temporary_artifacts !== 0) {
        errors.push(`agent_handoff case ${item.id} has unsafe evidence`);
      }
    });
  }
  for (const id of Object.keys(AGENT_HANDOFF_OBSERVED_CODES)) {
    if (!caseIds.has(id)) errors.push(`agent_handoff is missing required case: ${id}`);
  }
  if (caseIds.size !== Object.keys(AGENT_HANDOFF_OBSERVED_CODES).length) {
    errors.push("agent_handoff passed receipt must contain only required cases");
  }
}

function validateProviderAuthorization(
  receipt: Record<string, unknown>,
  errors: string[],
  environment: ProviderAuthorizationEnvironment,
) {
  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (receipt.candidate_commit !== environment.candidateCommit
    || receipt.release_identity !== PROVIDER_AUTHORIZATION_RELEASE) {
    errors.push("provider_authorization candidate identity does not match");
  }

  if (!isObject(receipt.target)) {
    errors.push("target must name hardware and OS");
  } else {
    const targetFields = new Set(Object.keys(PROVIDER_AUTHORIZATION_TARGET));
    rejectUnknownFields(receipt.target, targetFields, "target", errors);
    if (Object.entries(environment.target).some(([key, value]) => receipt.target[key] !== value)
      || Object.keys(receipt.target).length !== targetFields.size) {
      errors.push("provider_authorization target does not match");
    }
  }

  if (!isObject(receipt.versions)) {
    errors.push("provider_authorization versions are required");
  } else {
    rejectUnknownFields(receipt.versions, new Set(["keychain", "transfer", "app"]), "versions", errors);
    if (receipt.versions.keychain !== "macos-security-framework"
      || receipt.versions.transfer !== "provider-transfer-v1"
      || receipt.versions.app !== "0.4.0") {
      errors.push("provider_authorization versions do not match");
    }
  }

  if (!isObject(receipt.action)
    || receipt.action.kind !== "ui"
    || receipt.action.id !== "provider_authorization_qualification") {
    errors.push("provider_authorization requires the native qualification action");
  } else {
    rejectUnknownFields(receipt.action, new Set(["kind", "id"]), "action", errors);
  }
  if (receipt.expected_code !== "explicit_keychain_provider_transfer"
    || receipt.observed_code !== "provider_authorization_passed"
    || receipt.outcome !== "passed") {
    errors.push("provider_authorization outcome is incompatible");
  }

  if (!isObject(receipt.metrics)) {
    errors.push("provider_authorization metrics are required");
  } else {
    rejectUnknownFields(receipt.metrics, new Set([
      "maximum_requests_per_confirmation",
      "cancellation_requests",
      "automatic_retries",
      "payload_video_fields",
      "secret_matches",
    ]), "metrics", errors);
    if (receipt.metrics.maximum_requests_per_confirmation !== 1
      || receipt.metrics.cancellation_requests !== 0
      || receipt.metrics.automatic_retries !== 0
      || receipt.metrics.payload_video_fields !== 0
      || receipt.metrics.secret_matches !== 0) {
      errors.push("provider_authorization requires clean one-use transfers");
    }
  }

  if (!isObject(receipt.observations)) {
    errors.push("provider_authorization observations are required");
  } else {
    rejectUnknownFields(receipt.observations, new Set([
      "fallback_requests",
      "substitution_requests",
      "diagnostic_private_content_matches",
      "safety_failures",
    ]), "observations", errors);
    if (receipt.observations.fallback_requests !== 0
      || receipt.observations.substitution_requests !== 0
      || receipt.observations.diagnostic_private_content_matches !== 0
      || !Array.isArray(receipt.observations.safety_failures)
      || receipt.observations.safety_failures.length !== 0) {
      errors.push("provider_authorization requires zero fallback and safety observations");
    }
  }

  const artifactIds: string[] = [];
  if (Array.isArray(receipt.artifacts)) {
    receipt.artifacts.forEach((artifact, index) => {
      if (!isObject(artifact)
        || typeof artifact.id !== "string"
        || typeof artifact.path !== "string"
        || typeof artifact.sha256 !== "string"
        || !/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
        errors.push(`artifacts[${index}] must contain an identifier and SHA-256`);
        return;
      }
      rejectUnknownFields(artifact, new Set(["id", "path", "sha256"]), `artifacts[${index}]`, errors);
      artifactIds.push(artifact.id);
      if (artifact.path !== PROVIDER_AUTHORIZATION_ARTIFACT_PATHS[artifact.id]
        || artifact.sha256 !== environment.artifactHashes[artifact.id]) {
        errors.push(`provider_authorization artifact hash does not match: ${artifact.id}`);
      }
      const root = path.resolve(environment.artifactRoot);
      const absolute = path.resolve(root, artifact.path);
      if (!absolute.startsWith(`${root}${path.sep}`)) {
        errors.push(`provider_authorization artifact path escapes package root: ${artifact.id}`);
        return;
      }
      try {
        const bytes = readFileSync(absolute);
        const actual = createHash("sha256").update(bytes).digest("hex");
        if (actual !== artifact.sha256) {
          errors.push(`provider_authorization on-disk artifact hash does not match: ${artifact.id}`);
        }
        if (artifact.id === "application"
          && !bytes.includes(Buffer.from(`gcrdings-build-commit:${environment.candidateCommit}`))) {
          errors.push("provider_authorization application does not contain the candidate build marker");
        }
      } catch {
        errors.push(`provider_authorization on-disk artifact is unavailable: ${artifact.id}`);
      }
    });
  }
  if (!containsExactly(artifactIds, Object.keys(PROVIDER_AUTHORIZATION_ARTIFACT_HASHES))) {
    errors.push("provider_authorization requires exactly one hash per artifact");
  }

  if (!isObject(receipt.thresholds)) {
    errors.push("provider_authorization fixed thresholds are required");
  } else {
    rejectUnknownFields(receipt.thresholds, new Set(["source", "values", "failures"]), "thresholds", errors);
    const values = receipt.thresholds.values;
    if (receipt.thresholds.source !== "fixed_acceptance"
      || !Array.isArray(receipt.thresholds.failures)
      || receipt.thresholds.failures.length !== 0
      || !isObject(values)
      || values.maximum_requests_per_confirmation !== 1
      || values.maximum_cancellation_requests !== 0
      || values.maximum_automatic_retries !== 0
      || values.maximum_payload_video_fields !== 0
      || values.maximum_secret_matches !== 0) {
      errors.push("provider_authorization fixed thresholds are required");
    } else {
      rejectUnknownFields(values, new Set([
        "maximum_requests_per_confirmation",
        "maximum_cancellation_requests",
        "maximum_automatic_retries",
        "maximum_payload_video_fields",
        "maximum_secret_matches",
      ]), "thresholds.values", errors);
    }
  }

  const caseIds = new Set<string>();
  if (!Array.isArray(receipt.cases)) {
    errors.push("provider_authorization cases are required");
  } else {
    receipt.cases.forEach((item, index) => {
      if (!isObject(item)
        || typeof item.id !== "string"
        || item.category !== item.id
        || item.status !== "passed"
        || !isObject(item.evidence)) {
        errors.push(`cases[${index}] must contain provider authorization evidence`);
        return;
      }
      rejectUnknownFields(item, new Set(["id", "category", "status", "evidence"]), `cases[${index}]`, errors);
      if (caseIds.has(item.id)) errors.push(`cases contains duplicate id: ${item.id}`);
      caseIds.add(item.id);
      rejectUnknownFields(item.evidence, new Set([
        "observed_code", "requests", "secret_matches", "fallback_requests", "video_fields",
      ]), `cases[${index}].evidence`, errors);
      const expectedRequests = ["provider_failure", "explicit_retry", "outcome_unknown", "exact_payload"].includes(item.id) ? 1 : 0;
      if (!Number.isInteger(item.evidence.requests) || Number(item.evidence.requests) > 1) {
        errors.push(`provider_authorization case ${item.id} exceeds one request`);
      } else if (item.evidence.requests !== expectedRequests) {
        errors.push(`provider_authorization case ${item.id} has incompatible request count`);
      }
      if (item.evidence.secret_matches !== 0
        || item.evidence.fallback_requests !== 0
        || item.evidence.video_fields !== 0) {
        errors.push(`provider_authorization case ${item.id} has unsafe evidence`);
      }
      if (item.evidence.observed_code !== PROVIDER_AUTHORIZATION_OBSERVED_CODES[item.id]) {
        errors.push(`provider_authorization case ${item.id} has incompatible observed_code`);
      }
    });
  }
  for (const id of Object.keys(PROVIDER_AUTHORIZATION_OBSERVED_CODES)) {
    if (!caseIds.has(id)) errors.push(`provider_authorization is missing required case: ${id}`);
  }
  if (caseIds.size !== Object.keys(PROVIDER_AUTHORIZATION_OBSERVED_CODES).length) {
    errors.push("provider_authorization passed receipt must contain only required cases");
  }
}

export function validateReceipt(receipt: unknown, options: ValidationOptions = {}): string[] {
  const errors: string[] = [];
  if (!isObject(receipt)) return ["Receipt must be a JSON object"];

  scanPrivateData(receipt, "", errors);
  for (const key of Object.keys(receipt)) {
    if (!ALLOWED_TOP_LEVEL.has(key) && !FORBIDDEN_FIELDS.has(key)) {
      errors.push(`Unknown receipt field: ${key}`);
    }
  }
  if (receipt.receipt_type === "captured_timeline") {
    validateCapturedTimeline(receipt, errors);
    return [...new Set(errors)];
  }
  if (receipt.receipt_type === "local_findings") {
    validateLocalFindings(receipt, errors);
    return [...new Set(errors)];
  }
  if (receipt.receipt_type === "agent_handoff") {
    validateAgentHandoff(receipt, errors);
    return [...new Set(errors)];
  }
  if (receipt.receipt_type === "provider_authorization") {
    validateProviderAuthorization(receipt, errors, options.providerAuthorization ?? providerEnvironment());
    return [...new Set(errors)];
  }

  if (receipt.schema_version !== 1) errors.push("schema_version must be 1");
  if (typeof receipt.receipt_type !== "string" || !/^[a-z][a-z0-9_]{2,63}$/.test(receipt.receipt_type)) {
    errors.push("receipt_type must be a stable lowercase identifier");
  } else if (receipt.receipt_type !== "audio_corpus") {
    errors.push(`Unsupported receipt_type: ${receipt.receipt_type}`);
  }
  if (typeof receipt.candidate_commit !== "string" || !/^[a-f0-9]{40}$/i.test(receipt.candidate_commit)) {
    errors.push("candidate_commit must be a 40-character hexadecimal commit");
  }
  if (typeof receipt.release_identity !== "string" || receipt.release_identity.length === 0) {
    errors.push("release_identity is required");
  }

  if (!isObject(receipt.target)) {
    errors.push("target must name hardware and OS");
  } else {
    rejectUnknownFields(receipt.target, new Set(["hardware", "os"]), "target", errors);
    if (typeof receipt.target.hardware !== "string" || receipt.target.hardware.length === 0) {
      errors.push("target.hardware is required");
    }
    if (typeof receipt.target.os !== "string" || receipt.target.os.length === 0) {
      errors.push("target.os is required");
    }
  }

  if (!isObject(receipt.versions) || Object.keys(receipt.versions).length === 0) {
    errors.push("versions must contain at least one version identifier");
  } else {
    rejectUnknownFields(receipt.versions, AUDIO_CORPUS_VERSION_FIELDS, "versions", errors);
    if (Object.keys(receipt.versions).some((key) => !/^[a-z][a-z0-9_]{1,63}$/.test(key))) {
      errors.push("versions keys must be stable lowercase identifiers");
    }
    if (Object.values(receipt.versions).some((value) => typeof value !== "string" || value.length === 0)) {
      errors.push("versions values must be non-empty strings");
    }
    for (const key of AUDIO_CORPUS_VERSION_FIELDS) {
      if (typeof receipt.versions[key] !== "string" || receipt.versions[key].length === 0) {
        errors.push(`versions.${key} is required`);
      }
    }
  }

  if (!isObject(receipt.action) || !["command", "ui"].includes(String(receipt.action.kind))
    || typeof receipt.action.id !== "string" || !/^[a-z][a-z0-9_]{2,63}$/.test(receipt.action.id)) {
    errors.push("action must contain a command or UI identifier");
  } else {
    rejectUnknownFields(receipt.action, new Set(["kind", "id"]), "action", errors);
  }
  for (const key of ["expected_code", "observed_code"] as const) {
    if (typeof receipt[key] !== "string" || !/^[a-z][a-z0-9_]{2,95}$/.test(receipt[key])) {
      errors.push(`${key} must be a stable lowercase identifier`);
    }
  }
  if (!new Set(["passed", "failed", "measured", "evaluated"]).has(String(receipt.outcome))) {
    errors.push("outcome must be passed, failed, measured, or evaluated");
  }

  if (!isObject(receipt.metrics)
    || !isFiniteNonNegative(receipt.metrics.duration_ms)
    || !isFiniteNonNegative(receipt.metrics.peak_memory_mb)) {
    errors.push("metrics must contain finite non-negative duration_ms and peak_memory_mb");
  } else {
    rejectUnknownFields(receipt.metrics, new Set(["duration_ms", "peak_memory_mb", "quality"]), "metrics", errors);
    if (!isObject(receipt.metrics.quality)
      || !isFiniteNonNegative(receipt.metrics.quality.word_error_rate)
      || !isFiniteNonNegative(receipt.metrics.quality.speaker_count_accuracy)
      || !isFiniteNonNegative(receipt.metrics.quality.realtime_factor)
      || !isFiniteNonNegative(receipt.metrics.quality.peak_memory_mb)) {
      errors.push("metrics.quality must contain finite non-negative qualification metrics");
    } else {
      rejectUnknownFields(
        receipt.metrics.quality,
        new Set(["word_error_rate", "speaker_count_accuracy", "realtime_factor", "peak_memory_mb"]),
        "metrics.quality",
        errors,
      );
    }
  }
  if (!isObject(receipt.observations)
    || !isFiniteNonNegative(receipt.observations.network_requests)
    || !isFiniteNonNegative(receipt.observations.diagnostic_private_content_matches)) {
    errors.push("observations must contain finite non-negative network and diagnostic counts");
  } else {
    rejectUnknownFields(
      receipt.observations,
      new Set(["network_requests", "diagnostic_private_content_matches", "safety_failures"]),
      "observations",
      errors,
    );
    if (receipt.observations.safety_failures !== undefined
      && (!Array.isArray(receipt.observations.safety_failures)
        || receipt.observations.safety_failures.some((failure) => typeof failure !== "string"))) {
      errors.push("observations.safety_failures must be an array of identifiers");
    }
  }

  if (!Array.isArray(receipt.artifacts)) {
    errors.push("artifacts must be an array");
  } else {
    receipt.artifacts.forEach((artifact, index) => {
      if (!isObject(artifact)
        || typeof artifact.id !== "string"
        || !/^[a-z][a-z0-9_]{2,63}$/.test(artifact.id)
        || typeof artifact.sha256 !== "string"
        || !/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
        errors.push(`artifacts[${index}] must contain an identifier and SHA-256`);
      } else {
        rejectUnknownFields(artifact, new Set(["id", "sha256"]), `artifacts[${index}]`, errors);
      }
    });
  }
  if (!Array.isArray(receipt.cases) || receipt.cases.length === 0) {
    errors.push("cases must not be empty");
  } else {
    const ids = new Set<string>();
    receipt.cases.forEach((item, index) => {
      if (!isObject(item)
        || typeof item.id !== "string"
        || !/^[a-z][a-z0-9_]{2,63}$/.test(item.id)
        || typeof item.category !== "string"
        || !/^[a-z][a-z0-9_]{2,63}$/.test(item.category)
        || !["completed", "passed", "failed"].includes(String(item.status))) {
        errors.push(`cases[${index}] must contain stable id, category, and status values`);
      } else if (ids.has(item.id)) {
        errors.push(`cases contains duplicate id: ${item.id}`);
      } else {
        rejectUnknownFields(item, new Set(["id", "category", "status", "measurements"]), `cases[${index}]`, errors);
        ids.add(item.id);
        if (isObject(item.measurements)) {
          rejectUnknownFields(
            item.measurements,
            new Set([
              "word_error_rate",
              "speaker_count_accuracy",
              "overlap_detected",
              "realtime_factor",
              "peak_memory_mb",
            ]),
            `cases[${index}].measurements`,
            errors,
          );
        }
        if (receipt.receipt_type === "audio_corpus"
          && receipt.outcome === "passed"
          && AUDIO_CORPUS_SAMPLE_CASES.has(item.id)) {
          if (item.status !== "completed"
            || !isObject(item.measurements)
            || item.measurements.speaker_count_accuracy !== 1
            || !isFiniteNonNegative(item.measurements.realtime_factor)
            || !isFiniteNonNegative(item.measurements.peak_memory_mb)) {
            errors.push(`audio_corpus case ${item.id} does not satisfy required measurements`);
          }
          if (item.id === "two_speaker_overlap"
            && isObject(item.measurements)
            && item.measurements.overlap_detected !== true) {
            errors.push("audio_corpus overlap case must record detected overlap");
          }
        } else if (receipt.receipt_type === "audio_corpus"
          && receipt.outcome === "passed"
          && AUDIO_CORPUS_CASES.has(item.id)
          && item.status !== "passed") {
          errors.push(`audio_corpus failure case ${item.id} must pass`);
        }
      }
    });
    if (receipt.receipt_type === "audio_corpus" && receipt.outcome === "passed") {
      for (const id of AUDIO_CORPUS_CASES) {
        if (!ids.has(id)) errors.push(`audio_corpus is missing required case: ${id}`);
      }
      if (ids.size !== AUDIO_CORPUS_CASES.size) {
        errors.push("audio_corpus passed receipt must contain only required cases");
      }
    }
  }

  if (isObject(receipt.thresholds)) {
    rejectUnknownFields(receipt.thresholds, new Set(["source", "values", "failures"]), "thresholds", errors);
    if (isObject(receipt.thresholds.values)) {
      rejectUnknownFields(
        receipt.thresholds.values,
        new Set([
          "maximum_word_error_rate",
          "minimum_speaker_count_accuracy",
          "maximum_realtime_factor",
          "maximum_peak_memory_mb",
        ]),
        "thresholds.values",
        errors,
      );
    }
  }

  if (receipt.outcome === "passed") {
    if (!isObject(receipt.thresholds)
      || receipt.thresholds.source !== "baseline_derived"
      || !Array.isArray(receipt.thresholds.failures)
      || receipt.thresholds.failures.length !== 0
      || !isObject(receipt.thresholds.values)
      || !isFiniteNonNegative(receipt.thresholds.values.maximum_word_error_rate)
      || !isFiniteNonNegative(receipt.thresholds.values.minimum_speaker_count_accuracy)
      || !isFiniteNonNegative(receipt.thresholds.values.maximum_realtime_factor)
      || !isFiniteNonNegative(receipt.thresholds.values.maximum_peak_memory_mb)) {
      errors.push("passed receipt requires derived thresholds with no failures");
    } else {
      const policy = audioCorpusManifest.threshold_policy;
      if (receipt.thresholds.values.maximum_word_error_rate
        > policy.maximum_word_error_rate.absolute_ceiling) {
        errors.push("thresholds.values.maximum_word_error_rate exceeds manifest ceiling");
      }
      if (receipt.thresholds.values.minimum_speaker_count_accuracy
        < policy.minimum_speaker_count_accuracy.absolute_floor) {
        errors.push("thresholds.values.minimum_speaker_count_accuracy is below manifest floor");
      }
      if (receipt.thresholds.values.maximum_realtime_factor
        > policy.maximum_realtime_factor.absolute_ceiling) {
        errors.push("thresholds.values.maximum_realtime_factor exceeds manifest ceiling");
      }
      if (receipt.thresholds.values.maximum_peak_memory_mb
        > policy.maximum_peak_memory_mb.absolute_ceiling) {
        errors.push("thresholds.values.maximum_peak_memory_mb exceeds manifest ceiling");
      }
      const quality = isObject(receipt.metrics) && isObject(receipt.metrics.quality)
        ? receipt.metrics.quality
        : undefined;
      if (quality) {
        if (Number(quality.word_error_rate) > receipt.thresholds.values.maximum_word_error_rate) {
          errors.push("metrics.quality.word_error_rate exceeds threshold");
        }
        if (Number(quality.speaker_count_accuracy) < receipt.thresholds.values.minimum_speaker_count_accuracy) {
          errors.push("metrics.quality.speaker_count_accuracy is below threshold");
        }
        if (Number(quality.realtime_factor) > receipt.thresholds.values.maximum_realtime_factor) {
          errors.push("metrics.quality.realtime_factor exceeds threshold");
        }
        if (Number(quality.peak_memory_mb) > receipt.thresholds.values.maximum_peak_memory_mb) {
          errors.push("metrics.quality.peak_memory_mb exceeds threshold");
        }
      }
    }
    if (isObject(receipt.observations)
      && (receipt.observations.network_requests !== 0
        || receipt.observations.diagnostic_private_content_matches !== 0)) {
      errors.push("passed receipt requires zero safety observations");
    }
    if (!isObject(receipt.observations)
      || !Array.isArray(receipt.observations.safety_failures)
      || receipt.observations.safety_failures.length !== 0) {
      errors.push("audio_corpus passed receipt requires zero safety failures");
    }
    const artifactIds = Array.isArray(receipt.artifacts)
      ? receipt.artifacts.flatMap((artifact) => isObject(artifact) && typeof artifact.id === "string"
        ? [artifact.id]
        : [])
      : [];
    if (artifactIds.length !== 2
      || new Set(artifactIds).size !== 2
      || !artifactIds.includes("corpus_manifest")
      || !artifactIds.includes("generated_manifest")) {
      errors.push("audio_corpus passed receipt requires corpus and generated manifest artifacts");
    }
    if (!isObject(receipt.action)
      || receipt.action.kind !== "command"
      || receipt.action.id !== "audio_corpus_qualification") {
      errors.push("audio_corpus passed receipt requires the qualification command");
    }
    if (receipt.expected_code !== "local_pipeline_meets_derived_thresholds") {
      errors.push("audio_corpus passed receipt has incompatible expected_code");
    }
    if (receipt.observed_code !== "derived_thresholds_passed") {
      errors.push("audio_corpus passed receipt has incompatible observed_code");
    }
  }

  return [...new Set(errors)];
}

if (import.meta.main) {
  const receiptPath = process.argv[2];
  if (!receiptPath || process.argv.length !== 3) {
    console.error("Usage: bun qualification/validate-receipt.ts RECEIPT.json");
    process.exit(2);
  }
  try {
    const receipt = JSON.parse(readFileSync(path.resolve(receiptPath), "utf8"));
    const errors = validateReceipt(receipt);
    if (errors.length > 0) {
      errors.forEach((error) => console.error(error));
      process.exit(1);
    }
    console.log(`Receipt valid: ${receipt.receipt_type}`);
  } catch {
    console.error("Receipt is not readable JSON");
    process.exit(1);
  }
}
