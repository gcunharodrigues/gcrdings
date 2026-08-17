import { afterAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  calculateWordErrorRate,
  deriveThresholds,
  evaluateThresholds,
  measureQualification,
} from "../../../qualification/audio-corpus/scripts/measure";
import { validateReceipt } from "../../../qualification/validate-receipt";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const CORPUS_ROOT = path.join(REPO_ROOT, "qualification", "audio-corpus");
const MANIFEST_PATH = path.join(CORPUS_ROOT, "manifest.json");
const GENERATE_PATH = path.join(CORPUS_ROOT, "scripts", "generate.sh");
const QUALIFY_PATH = path.join(CORPUS_ROOT, "scripts", "qualify.sh");
const QUALIFIED_RECEIPT_PATH = path.join(CORPUS_ROOT, "results", "target-mac.json");
const HAS_LOCAL_QUALIFICATION_RECEIPT = existsSync(QUALIFIED_RECEIPT_PATH);
const temporaryDirectories: string[] = [];

function createTemporaryDirectory(prefix: string): string {
  const directory = mkdtempSync(path.join(tmpdir(), prefix));
  temporaryDirectories.push(directory);
  return directory;
}

afterAll(() => {
  temporaryDirectories.forEach((directory) => rmSync(directory, { recursive: true, force: true }));
});

function loadManifest() {
  return JSON.parse(readFileSync(MANIFEST_PATH, "utf8"));
}

describe("private audio qualification corpus", () => {
  test("declares every accepted language, acoustic, speaker, and duration category", () => {
    const manifest = loadManifest();
    const categories = new Set(manifest.samples.map((sample: { category: string }) => sample.category));

    expect(manifest.schema_version).toBe(1);
    expect(manifest.privacy.classification).toBe("synthetic_private_safe");
    expect(categories).toEqual(new Set([
      "portuguese",
      "english",
      "code_switch",
      "noise",
      "overlap",
      "multi_speaker",
      "long_form",
      "silence_boundary",
    ]));
    expect(manifest.failure_cases.map((item: { id: string }) => item.id)).toEqual([
      "missing_audio",
      "malformed_transcript",
      "model_failure",
      "interrupted_run",
    ]);
    expect(manifest.samples.find((sample: { category: string }) => sample.category === "long_form"))
      .toMatchObject({ measure_word_error_rate: false, repeat_to_seconds: 120 });
  });

  test("regenerates byte-identical audio and hashes without committed recordings", () => {
    const first = createTemporaryDirectory("gcrdings-corpus-first-");
    const second = createTemporaryDirectory("gcrdings-corpus-second-");

    for (const output of [first, second]) {
      const run = Bun.spawnSync({ cmd: [GENERATE_PATH, "--output", output], cwd: REPO_ROOT });
      expect(run.exitCode, run.stderr.toString()).toBe(0);
    }

    const firstManifest = readFileSync(path.join(first, "generated-manifest.json"), "utf8");
    const secondManifest = readFileSync(path.join(second, "generated-manifest.json"), "utf8");
    expect(firstManifest).toBe(secondManifest);

    const generated = JSON.parse(firstManifest);
    expect(generated.corpus_manifest_sha256).toMatch(/^[a-f0-9]{64}$/);
    expect(generated.samples).toHaveLength(loadManifest().samples.length);
    for (const sample of generated.samples) {
      expect(sample.sha256).toMatch(/^[a-f0-9]{64}$/);
      expect(existsSync(path.join(first, sample.file))).toBe(true);
      expect(createHash("sha256").update(readFileSync(path.join(first, sample.file))).digest("hex"))
        .toBe(sample.sha256);
      expect(readFileSync(path.join(first, sample.file))).toEqual(
        readFileSync(path.join(second, sample.file)),
      );
    }
    expect(existsSync(path.join(CORPUS_ROOT, "generated"))).toBe(false);
  }, 90_000);

  test("calculates word errors and derives bounded thresholds from measured baselines", () => {
    expect(calculateWordErrorRate("one two three four", "one two five four")).toBe(0.25);
    expect(calculateWordErrorRate("", "")).toBe(0);
    expect(calculateWordErrorRate("", "unexpected" )).toBe(1);

    const thresholds = deriveThresholds([
      {
        word_error_rate: 0.1,
        speaker_count_accuracy: 0.95,
        realtime_factor: 0.8,
        peak_memory_mb: 1000,
      },
      {
        word_error_rate: 0.2,
        speaker_count_accuracy: 0.9,
        realtime_factor: 1,
        peak_memory_mb: 1200,
      },
    ], loadManifest().threshold_policy);

    expect(thresholds).toEqual({
      maximum_word_error_rate: 0.25,
      minimum_speaker_count_accuracy: 0.85,
      maximum_realtime_factor: 1.25,
      maximum_peak_memory_mb: 1500,
    });
    expect(evaluateThresholds({
      word_error_rate: 0.2,
      speaker_count_accuracy: 0.9,
      realtime_factor: 1,
      peak_memory_mb: 1200,
    }, thresholds)).toEqual([]);
    expect(evaluateThresholds({
      word_error_rate: 0.3,
      speaker_count_accuracy: 0.8,
      realtime_factor: 1.5,
      peak_memory_mb: 1600,
    }, thresholds)).toEqual([
      "maximum_word_error_rate",
      "minimum_speaker_count_accuracy",
      "maximum_realtime_factor",
      "maximum_peak_memory_mb",
    ]);
  });

  test("rejects a required multi-speaker case hidden by aggregate accuracy", () => {
    const manifest = loadManifest();
    const generated = {
      corpus_manifest_sha256: "a".repeat(64),
      samples: manifest.samples.map((sample: any) => ({ id: sample.id, duration_ms: 1_000 })),
    };
    const observations = {
      candidate_commit: "b".repeat(40),
      release_identity: "gcrdings-v1-wave2",
      target: { hardware: "Apple-Silicon", os: "macOS-26.5" },
      versions: { corpus: manifest.corpus_id, engine: "unit-test-no-model" },
      generated_manifest_sha256: "c".repeat(64),
      samples: manifest.samples.map((sample: any) => ({
        id: sample.id,
        status: "completed",
        transcript: sample.expected_text,
        speaker_count: sample.id === "two_speaker_overlap" ? 0 : sample.expected_speaker_count,
        processing_ms: 100,
        peak_memory_mb: 128,
        network_requests: 0,
        diagnostic_private_content_matches: 0,
      })),
      failure_cases: manifest.failure_cases.map((failure: any) => ({
        id: failure.id,
        error_code: failure.expected_error,
        retry_safe: failure.retry_safe,
      })),
    };

    const receipt = measureQualification(manifest, generated, observations, [{
      word_error_rate: 0,
      speaker_count_accuracy: 1,
      realtime_factor: 0.1,
      peak_memory_mb: 128,
    }]);

    expect(receipt.outcome).toBe("failed");
    expect((receipt.thresholds as { failures: string[] }).failures)
      .toContain("two_speaker_overlap.speaker_count_accuracy");
  });

  test("accepts aggregate receipts and rejects private or secret-bearing fields", () => {
    const receipt = {
      schema_version: 1,
      receipt_type: "audio_corpus",
      candidate_commit: "a".repeat(40),
      release_identity: "gcrdings-v1-wave2",
      target: { hardware: "Apple-Silicon", os: "macOS-26.5" },
      versions: { corpus: "gcrdings-private-audio-v1", engine: "parakeet-pinned" },
      action: { kind: "command", id: "audio_corpus_qualification" },
      expected_code: "local_pipeline_meets_derived_thresholds",
      observed_code: "baseline_measurement_recorded",
      outcome: "measured",
      metrics: {
        duration_ms: 1000,
        peak_memory_mb: 512,
        quality: {
          word_error_rate: 0.1,
          speaker_count_accuracy: 1,
          realtime_factor: 0.5,
          peak_memory_mb: 512,
        },
      },
      observations: { network_requests: 0, diagnostic_private_content_matches: 0 },
      artifacts: [{ id: "corpus_manifest", sha256: "b".repeat(64) }],
      thresholds: { source: "pending_baseline" },
      cases: [{ id: "portuguese_clear", category: "portuguese", status: "completed" }],
    };

    expect(validateReceipt(receipt)).toEqual([]);
    expect(validateReceipt({ ...receipt, candidate_commit: "short" })).toContain(
      "candidate_commit must be a 40-character hexadecimal commit",
    );
    expect(validateReceipt({ ...receipt, transcript: "private words" })).toContain(
      "Forbidden private field: transcript",
    );
    expect(validateReceipt({ ...receipt, release_identity: "/Users/operator/session" })).toContain(
      "Potential private path in release_identity",
    );
    expect(validateReceipt({ ...receipt, release_identity: "Bearer test-token" })).toContain(
      "Potential credential in release_identity",
    );
    expect(validateReceipt({ ...receipt, versions: { apiKey: "plain-test-token" } })).toContain(
      "Forbidden private field: apiKey",
    );
    expect(validateReceipt({ ...receipt, versions: { token: "plain-test-token" } }).length).toBeGreaterThan(0);
    expect(validateReceipt({ ...receipt, receipt_type: "other_receipt" }).length).toBeGreaterThan(0);
    expect(validateReceipt({ ...receipt, cases: [{}] })).toContain(
      "cases[0] must contain stable id, category, and status values",
    );
  });

  test.skipIf(!HAS_LOCAL_QUALIFICATION_RECEIPT)("rejects a passed receipt whose aggregate metrics violate its thresholds", () => {
    const receipt = JSON.parse(readFileSync(QUALIFIED_RECEIPT_PATH, "utf8"));
    receipt.metrics.quality = {
      word_error_rate: 1,
      speaker_count_accuracy: 0,
      realtime_factor: 99,
      peak_memory_mb: 5_000,
    };

    expect(validateReceipt(receipt)).toEqual(expect.arrayContaining([
      "metrics.quality.word_error_rate exceeds threshold",
      "metrics.quality.speaker_count_accuracy is below threshold",
      "metrics.quality.realtime_factor exceeds threshold",
      "metrics.quality.peak_memory_mb exceeds threshold",
    ]));

    receipt.thresholds.values = {
      maximum_word_error_rate: 1,
      minimum_speaker_count_accuracy: 0,
      maximum_realtime_factor: 99,
      maximum_peak_memory_mb: 5_000,
    };
    expect(validateReceipt(receipt)).toEqual(expect.arrayContaining([
      "thresholds.values.maximum_word_error_rate exceeds manifest ceiling",
      "thresholds.values.minimum_speaker_count_accuracy is below manifest floor",
      "thresholds.values.maximum_realtime_factor exceeds manifest ceiling",
      "thresholds.values.maximum_peak_memory_mb exceeds manifest ceiling",
    ]));
  });

  test.skipIf(!HAS_LOCAL_QUALIFICATION_RECEIPT)("rejects passed receipts without fixed provenance and safety evidence", () => {
    const receipt = JSON.parse(readFileSync(QUALIFIED_RECEIPT_PATH, "utf8"));

    expect(validateReceipt({ ...receipt, artifacts: [] })).toContain(
      "audio_corpus passed receipt requires corpus and generated manifest artifacts",
    );
    expect(validateReceipt({
      ...receipt,
      observations: { ...receipt.observations, safety_failures: ["network_observed"] },
    })).toContain("audio_corpus passed receipt requires zero safety failures");
    expect(validateReceipt({
      ...receipt,
      action: { kind: "ui", id: "anything" },
      expected_code: "anything",
      observed_code: "anything",
    })).toEqual(expect.arrayContaining([
      "audio_corpus passed receipt requires the qualification command",
      "audio_corpus passed receipt has incompatible expected_code",
      "audio_corpus passed receipt has incompatible observed_code",
    ]));
  });

  test("runs generation, aggregate measurement, and receipt validation without a model or network", () => {
    const manifest = loadManifest();
    const working = createTemporaryDirectory("gcrdings-qualification-");
    const observationsPath = path.join(working, "observations.json");
    const receiptPath = path.join(working, "receipt.json");
    const corpusPath = path.join(working, "corpus");
    writeFileSync(observationsPath, JSON.stringify({
      schema_version: 1,
      candidate_commit: "c".repeat(40),
      release_identity: "gcrdings-v1-wave2",
      target: { hardware: "Apple-Silicon", os: "macOS-26.5" },
      versions: { corpus: manifest.corpus_id, engine: "unit-test-no-model" },
      samples: manifest.samples.map((sample: any) => ({
        id: sample.id,
        status: "completed",
        transcript: sample.expected_text,
        speaker_count: sample.expected_speaker_count,
        ...(sample.expected_overlap === true ? { overlap_detected: true } : {}),
        processing_ms: 100,
        peak_memory_mb: 128,
        network_requests: 0,
        diagnostic_private_content_matches: 0,
      })),
      failure_cases: manifest.failure_cases.map((failure: any) => ({
        id: failure.id,
        error_code: failure.expected_error,
        retry_safe: failure.retry_safe,
      })),
    }));

    const run = Bun.spawnSync({
      cmd: [
        QUALIFY_PATH,
        "--observations", observationsPath,
        "--output", receiptPath,
        "--corpus-dir", corpusPath,
      ],
      cwd: REPO_ROOT,
    });
    expect(run.exitCode, run.stderr.toString()).toBe(0);

    const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
    expect(validateReceipt(receipt)).toEqual([]);
    expect(receipt.metrics.quality.word_error_rate).toBe(0);
    expect(receipt.metrics.quality.speaker_count_accuracy).toBe(1);
    expect(receipt.metrics.quality.realtime_factor).toBeGreaterThan(0);
    expect(receipt.metrics.quality.realtime_factor).toBeLessThan(1);
    expect(receipt.metrics.quality.peak_memory_mb).toBe(128);
    expect(receipt.thresholds.source).toBe("pending_baseline");
    expect(receipt.observations).toEqual({
      network_requests: 0,
      diagnostic_private_content_matches: 0,
      safety_failures: [],
    });

    const unsafeObservations = JSON.parse(readFileSync(observationsPath, "utf8"));
    unsafeObservations.samples[0].network_requests = 1;
    writeFileSync(observationsPath, JSON.stringify(unsafeObservations));
    const failedReceiptPath = path.join(working, "failed-receipt.json");
    const failedRun = Bun.spawnSync({
      cmd: [
        QUALIFY_PATH,
        "--observations", observationsPath,
        "--output", failedReceiptPath,
        "--corpus-dir", corpusPath,
      ],
      cwd: REPO_ROOT,
    });
    expect(failedRun.exitCode).toBe(1);
    const failedReceipt = JSON.parse(readFileSync(failedReceiptPath, "utf8"));
    expect(validateReceipt(failedReceipt)).toEqual([]);
    expect(failedReceipt).toMatchObject({
      observed_code: "safety_observation_failed",
      outcome: "failed",
      observations: {
        network_requests: 1,
        safety_failures: ["network_requests_observed"],
      },
    });
  }, 60_000);
});
