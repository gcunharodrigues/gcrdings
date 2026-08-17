#!/bin/sh
set -eu

usage() {
  echo "Usage: $0 --output DIRECTORY"
}

output=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      [ "$#" -ge 2 ] || { usage >&2; exit 2; }
      output=$2
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

[ -n "$output" ] || { usage >&2; exit 2; }
[ -x /usr/bin/say ] || { echo "Corpus generation requires macOS /usr/bin/say" >&2; exit 1; }
command -v bun >/dev/null 2>&1 || { echo "Corpus generation requires Bun" >&2; exit 1; }

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
manifest="$script_dir/../manifest.json"

bun run - "$manifest" "$output" <<'BUN'
import { createHash } from "node:crypto";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const manifestPath = path.resolve(process.argv[2]);
const outputDirectory = path.resolve(process.argv[3]);
const manifestBytes = readFileSync(manifestPath);
const manifest = JSON.parse(manifestBytes.toString("utf8"));
const sampleRate = manifest.generation.sample_rate_hz;
const speechRate = manifest.generation.speech_rate_words_per_minute;

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

function decodeWav(file) {
  const bytes = readFileSync(file);
  if (bytes.toString("ascii", 0, 4) !== "RIFF" || bytes.toString("ascii", 8, 12) !== "WAVE") {
    throw new Error("Synthesizer returned invalid WAV audio");
  }

  let offset = 12;
  let format;
  let samples;
  while (offset + 8 <= bytes.length) {
    const id = bytes.toString("ascii", offset, offset + 4);
    const size = bytes.readUInt32LE(offset + 4);
    const dataStart = offset + 8;
    if (id === "fmt ") {
      format = {
        encoding: bytes.readUInt16LE(dataStart),
        channels: bytes.readUInt16LE(dataStart + 2),
        rate: bytes.readUInt32LE(dataStart + 4),
        bits: bytes.readUInt16LE(dataStart + 14),
      };
    } else if (id === "data") {
      samples = new Int16Array(size / 2);
      for (let index = 0; index < samples.length; index += 1) {
        samples[index] = bytes.readInt16LE(dataStart + index * 2);
      }
    }
    offset = dataStart + size + (size % 2);
  }

  if (!format || format.encoding !== 1 || format.channels !== 1 || format.rate !== sampleRate || format.bits !== 16 || !samples) {
    throw new Error(`Synthesizer must return mono ${sampleRate} Hz signed 16-bit PCM`);
  }
  return samples;
}

function encodeWav(samples) {
  const dataLength = samples.length * 2;
  const bytes = Buffer.alloc(44 + dataLength);
  bytes.write("RIFF", 0, "ascii");
  bytes.writeUInt32LE(36 + dataLength, 4);
  bytes.write("WAVEfmt ", 8, "ascii");
  bytes.writeUInt32LE(16, 16);
  bytes.writeUInt16LE(1, 20);
  bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(sampleRate, 24);
  bytes.writeUInt32LE(sampleRate * 2, 28);
  bytes.writeUInt16LE(2, 32);
  bytes.writeUInt16LE(16, 34);
  bytes.write("data", 36, "ascii");
  bytes.writeUInt32LE(dataLength, 40);
  for (let index = 0; index < samples.length; index += 1) {
    bytes.writeInt16LE(samples[index], 44 + index * 2);
  }
  return bytes;
}

function noiseGenerator(id) {
  let state = Number.parseInt(sha256(id).slice(0, 8), 16) >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 0x1_0000_0000;
  };
}

function synthesize(sample, temporaryDirectory, sourceCache) {
  const sources = sample.sources.map((source) => {
    const voice = manifest.generation.voices[source.voice];
    if (!voice) throw new Error(`Unknown voice key for ${sample.id}`);
    const cacheKey = `${voice}\0${source.text}`;
    let samples = sourceCache.get(cacheKey);
    if (!samples) {
      const file = path.join(temporaryDirectory, `${sha256(cacheKey)}.wav`);
      const result = Bun.spawnSync({
        cmd: [
          "/usr/bin/say",
          "-v", voice,
          "-r", String(speechRate),
          "--file-format=WAVE",
          `--data-format=LEI16@${sampleRate}`,
          "-o", file,
          source.text,
        ],
        stdout: "ignore",
        stderr: "pipe",
      });
      if (result.exitCode !== 0) throw new Error(`Required local voice is unavailable: ${source.voice}`);
      samples = decodeWav(file);
      sourceCache.set(cacheKey, samples);
    }
    return { ...source, samples };
  });

  let length = Math.round((sample.duration_seconds ?? 0) * sampleRate);
  for (const source of sources) {
    length = Math.max(length, Math.round(source.start_ms * sampleRate / 1000) + source.samples.length);
  }
  if (sample.repeat_to_seconds) length = Math.round(sample.repeat_to_seconds * sampleRate);

  const mixed = new Int16Array(length);
  for (const source of sources) {
    const start = Math.round(source.start_ms * sampleRate / 1000);
    const available = Math.max(0, length - start);
    const repeatedLength = sample.repeat_to_seconds ? available : Math.min(available, source.samples.length);
    for (let index = 0; index < repeatedLength; index += 1) {
      const sourceSample = source.samples[index % source.samples.length];
      const value = mixed[start + index] + Math.round(sourceSample * source.gain);
      mixed[start + index] = Math.max(-32768, Math.min(32767, value));
    }
  }

  if (sample.noise_amplitude) {
    const random = noiseGenerator(sample.id);
    for (let index = 0; index < mixed.length; index += 1) {
      const noise = Math.round((random() * 2 - 1) * sample.noise_amplitude);
      mixed[index] = Math.max(-32768, Math.min(32767, mixed[index] + noise));
    }
  }
  return mixed;
}

mkdirSync(outputDirectory, { recursive: true });
const temporaryDirectory = mkdtempSync(path.join(tmpdir(), "gcrdings-audio-corpus-"));
try {
  const generatedSamples = [];
  const sourceCache = new Map();
  for (const sample of manifest.samples) {
    const audio = encodeWav(synthesize(sample, temporaryDirectory, sourceCache));
    const file = `${sample.id}.wav`;
    const destination = path.join(outputDirectory, file);
    const temporaryDestination = `${destination}.tmp`;
    writeFileSync(temporaryDestination, audio);
    renameSync(temporaryDestination, destination);
    generatedSamples.push({
      id: sample.id,
      category: sample.category,
      file,
      sha256: sha256(audio),
      bytes: audio.length,
      duration_ms: Math.round((audio.length - 44) / 2 / sampleRate * 1000),
    });
  }

  const generated = {
    schema_version: 1,
    corpus_id: manifest.corpus_id,
    corpus_manifest_sha256: sha256(manifestBytes),
    samples: generatedSamples,
  };
  const destination = path.join(outputDirectory, "generated-manifest.json");
  writeFileSync(`${destination}.tmp`, `${JSON.stringify(generated, null, 2)}\n`);
  renameSync(`${destination}.tmp`, destination);
} finally {
  rmSync(temporaryDirectory, { recursive: true, force: true });
}
BUN
