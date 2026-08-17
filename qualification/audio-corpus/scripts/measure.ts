import { createHash } from "node:crypto";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";

export interface AggregateMetrics {
  word_error_rate: number;
  speaker_count_accuracy: number;
  realtime_factor: number;
  peak_memory_mb: number;
}

interface ThresholdPolicy {
  minimum_baseline_runs: number;
  maximum_word_error_rate: { margin: number; absolute_ceiling: number };
  minimum_speaker_count_accuracy: { margin: number; absolute_floor: number };
  maximum_realtime_factor: { multiplier: number; absolute_ceiling: number };
  maximum_peak_memory_mb: { multiplier: number; absolute_ceiling: number };
}

export interface DerivedThresholds {
  maximum_word_error_rate: number;
  minimum_speaker_count_accuracy: number;
  maximum_realtime_factor: number;
  maximum_peak_memory_mb: number;
}

function round(value: number): number {
  return Number(value.toFixed(6));
}

function normalizeWords(text: string): string[] {
  return text
    .normalize("NFKD")
    .replace(/\p{Mark}/gu, "")
    .toLocaleLowerCase("en-US")
    .replace(/[^\p{Letter}\p{Number}]+/gu, " ")
    .trim()
    .split(/\s+/)
    .filter(Boolean);
}

export function calculateWordErrorRate(reference: string, hypothesis: string): number {
  const expected = normalizeWords(reference);
  const observed = normalizeWords(hypothesis);
  if (expected.length === 0) return observed.length === 0 ? 0 : 1;

  let previous = Array.from({ length: observed.length + 1 }, (_, index) => index);
  for (let row = 1; row <= expected.length; row += 1) {
    const current = [row];
    for (let column = 1; column <= observed.length; column += 1) {
      current[column] = Math.min(
        current[column - 1] + 1,
        previous[column] + 1,
        previous[column - 1] + (expected[row - 1] === observed[column - 1] ? 0 : 1),
      );
    }
    previous = current;
  }
  return round(previous[observed.length] / expected.length);
}

export function deriveThresholds(baselines: AggregateMetrics[], policy: ThresholdPolicy): DerivedThresholds {
  if (baselines.length < policy.minimum_baseline_runs) {
    throw new Error(`At least ${policy.minimum_baseline_runs} baseline run(s) are required`);
  }
  const maximum = (key: keyof AggregateMetrics) => Math.max(...baselines.map((run) => run[key]));
  const minimum = (key: keyof AggregateMetrics) => Math.min(...baselines.map((run) => run[key]));

  return {
    maximum_word_error_rate: round(Math.min(
      policy.maximum_word_error_rate.absolute_ceiling,
      maximum("word_error_rate") + policy.maximum_word_error_rate.margin,
    )),
    minimum_speaker_count_accuracy: round(Math.max(
      policy.minimum_speaker_count_accuracy.absolute_floor,
      minimum("speaker_count_accuracy") - policy.minimum_speaker_count_accuracy.margin,
    )),
    maximum_realtime_factor: round(Math.min(
      policy.maximum_realtime_factor.absolute_ceiling,
      maximum("realtime_factor") * policy.maximum_realtime_factor.multiplier,
    )),
    maximum_peak_memory_mb: round(Math.min(
      policy.maximum_peak_memory_mb.absolute_ceiling,
      maximum("peak_memory_mb") * policy.maximum_peak_memory_mb.multiplier,
    )),
  };
}

export function evaluateThresholds(metrics: AggregateMetrics, thresholds: DerivedThresholds): string[] {
  const failures: string[] = [];
  if (metrics.word_error_rate > thresholds.maximum_word_error_rate) {
    failures.push("maximum_word_error_rate");
  }
  if (metrics.speaker_count_accuracy < thresholds.minimum_speaker_count_accuracy) {
    failures.push("minimum_speaker_count_accuracy");
  }
  if (metrics.realtime_factor > thresholds.maximum_realtime_factor) {
    failures.push("maximum_realtime_factor");
  }
  if (metrics.peak_memory_mb > thresholds.maximum_peak_memory_mb) {
    failures.push("maximum_peak_memory_mb");
  }
  return failures;
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function requireFinite(value: unknown, field: string): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw new Error(`${field} must be a finite non-negative number`);
  }
  return value;
}

export function measureQualification(manifest: any, generated: any, observations: any, baselines: AggregateMetrics[] = []) {
  const generatedById = new Map(generated.samples.map((sample: any) => [sample.id, sample]));
  const observedById = new Map(observations.samples.map((sample: any) => [sample.id, sample]));
  const cases = [];
  const requiredCaseFailures: string[] = [];
  let wordErrorTotal = 0;
  let wordErrorSampleCount = 0;
  let speakerAccuracyTotal = 0;
  let processingTotal = 0;
  let durationTotal = 0;
  let peakMemory = 0;
  let networkRequests = 0;
  let diagnosticMatches = 0;

  for (const sample of manifest.samples) {
    const generatedSample: any = generatedById.get(sample.id);
    const observed: any = observedById.get(sample.id);
    if (!generatedSample || !observed) throw new Error(`Missing observation for sample ${sample.id}`);
    if (observed.status !== "completed") throw new Error(`Sample ${sample.id} did not complete`);
    if (typeof observed.transcript !== "string") throw new Error(`Sample ${sample.id} has no transcript observation`);

    const measuresWordErrorRate = sample.measure_word_error_rate !== false;
    const wordErrorRate = measuresWordErrorRate
      ? calculateWordErrorRate(sample.expected_text, observed.transcript)
      : null;
    const expectedSpeakers = requireFinite(sample.expected_speaker_count, `${sample.id}.expected_speaker_count`);
    const observedSpeakers = requireFinite(observed.speaker_count, `${sample.id}.speaker_count`);
    const speakerAccuracy = expectedSpeakers === 0
      ? (observedSpeakers === 0 ? 1 : 0)
      : Math.max(0, 1 - Math.abs(expectedSpeakers - observedSpeakers) / expectedSpeakers);
    if (expectedSpeakers > 0 && speakerAccuracy !== 1) {
      requiredCaseFailures.push(`${sample.id}.speaker_count_accuracy`);
    }
    const overlapDetected = sample.expected_overlap === true
      ? observed.overlap_detected === true
      : undefined;
    if (sample.expected_overlap === true && !overlapDetected) {
      requiredCaseFailures.push(`${sample.id}.overlap_state`);
    }
    const processingMs = requireFinite(observed.processing_ms, `${sample.id}.processing_ms`);
    const durationMs = requireFinite(generatedSample.duration_ms, `${sample.id}.duration_ms`);
    const memoryMb = requireFinite(observed.peak_memory_mb, `${sample.id}.peak_memory_mb`);
    const sampleNetwork = requireFinite(observed.network_requests, `${sample.id}.network_requests`);
    const sampleDiagnosticMatches = requireFinite(
      observed.diagnostic_private_content_matches,
      `${sample.id}.diagnostic_private_content_matches`,
    );

    if (wordErrorRate !== null) {
      wordErrorTotal += wordErrorRate;
      wordErrorSampleCount += 1;
    }
    speakerAccuracyTotal += speakerAccuracy;
    processingTotal += processingMs;
    durationTotal += durationMs;
    peakMemory = Math.max(peakMemory, memoryMb);
    networkRequests += sampleNetwork;
    diagnosticMatches += sampleDiagnosticMatches;
    cases.push({
      id: sample.id,
      category: sample.category,
      status: "completed",
      measurements: {
        ...(wordErrorRate === null ? {} : { word_error_rate: wordErrorRate }),
        speaker_count_accuracy: round(speakerAccuracy),
        ...(sample.expected_overlap === true ? { overlap_detected: overlapDetected } : {}),
        realtime_factor: durationMs === 0 ? 0 : round(processingMs / durationMs),
        peak_memory_mb: memoryMb,
      },
    });
  }

  for (const failure of manifest.failure_cases) {
    const observed = observations.failure_cases?.find((item: any) => item.id === failure.id);
    if (!observed || observed.error_code !== failure.expected_error || observed.retry_safe !== failure.retry_safe) {
      throw new Error(`Failure observation does not satisfy ${failure.id}`);
    }
    cases.push({ id: failure.id, category: "failure", status: "passed" });
  }

  const metrics: AggregateMetrics = {
    word_error_rate: wordErrorSampleCount === 0 ? 0 : round(wordErrorTotal / wordErrorSampleCount),
    speaker_count_accuracy: round(speakerAccuracyTotal / manifest.samples.length),
    realtime_factor: durationTotal === 0 ? 0 : round(processingTotal / durationTotal),
    peak_memory_mb: round(peakMemory),
  };
  const derivedThresholds = baselines.length > 0
    ? deriveThresholds(baselines, manifest.threshold_policy)
    : null;
  const thresholdFailures = derivedThresholds
    ? [...evaluateThresholds(metrics, derivedThresholds), ...requiredCaseFailures]
    : requiredCaseFailures;
  const safetyFailures = [
    ...(networkRequests === 0 ? [] : ["network_requests_observed"]),
    ...(diagnosticMatches === 0 ? [] : ["private_content_observed_in_diagnostics"]),
  ];
  const thresholds = derivedThresholds
    ? { source: "baseline_derived", values: derivedThresholds, failures: thresholdFailures }
    : { source: "pending_baseline" };

  return {
    schema_version: 1,
    receipt_type: "audio_corpus",
    candidate_commit: observations.candidate_commit,
    release_identity: observations.release_identity,
    target: observations.target,
    versions: observations.versions,
    action: { kind: "command", id: "audio_corpus_qualification" },
    expected_code: "local_pipeline_meets_derived_thresholds",
    observed_code: safetyFailures.length > 0
      ? "safety_observation_failed"
      : baselines.length === 0
      ? "baseline_measurement_recorded"
      : thresholdFailures.length === 0
        ? "derived_thresholds_passed"
        : "derived_thresholds_failed",
    outcome: safetyFailures.length > 0
      ? "failed"
      : baselines.length === 0
        ? "measured"
        : thresholdFailures.length === 0 ? "passed" : "failed",
    metrics: {
      duration_ms: round(processingTotal),
      peak_memory_mb: metrics.peak_memory_mb,
      quality: metrics,
    },
    observations: {
      network_requests: networkRequests,
      diagnostic_private_content_matches: diagnosticMatches,
      safety_failures: safetyFailures,
    },
    artifacts: [
      { id: "corpus_manifest", sha256: generated.corpus_manifest_sha256 },
      { id: "generated_manifest", sha256: observations.generated_manifest_sha256 },
    ],
    thresholds,
    cases,
  };
}

function parseArguments(args: string[]) {
  const parsed: Record<string, string | string[]> = {};
  for (let index = 0; index < args.length; index += 1) {
    const key = args[index];
    if (!["--manifest", "--generated", "--observations", "--output", "--baseline"].includes(key)) {
      throw new Error(`Unknown argument: ${key}`);
    }
    const value = args[index + 1];
    if (!value) throw new Error(`Missing value for ${key}`);
    index += 1;
    if (key === "--baseline") {
      parsed[key] = [...(parsed[key] as string[] | undefined ?? []), value];
    } else {
      parsed[key] = value;
    }
  }
  for (const key of ["--manifest", "--generated", "--observations", "--output"]) {
    if (typeof parsed[key] !== "string") throw new Error(`Missing required argument: ${key}`);
  }
  return parsed;
}

if (import.meta.main) {
  try {
    const args = parseArguments(process.argv.slice(2));
    const readJson = (file: string) => JSON.parse(readFileSync(path.resolve(file), "utf8"));
    const manifest = readJson(args["--manifest"] as string);
    const generatedPath = path.resolve(args["--generated"] as string);
    const generated = readJson(generatedPath);
    const observations = readJson(args["--observations"] as string);
    observations.generated_manifest_sha256 = sha256(readFileSync(generatedPath));
    const baselines = (args["--baseline"] as string[] | undefined ?? [])
      .map(readJson)
      .map((receipt) => receipt.metrics.quality as AggregateMetrics);
    const receipt = measureQualification(manifest, generated, observations, baselines);
    const output = path.resolve(args["--output"] as string);
    writeFileSync(`${output}.tmp`, `${JSON.stringify(receipt, null, 2)}\n`);
    renameSync(`${output}.tmp`, output);
  } catch (error) {
    console.error(error instanceof Error ? error.message : "Qualification measurement failed");
    process.exit(1);
  }
}
