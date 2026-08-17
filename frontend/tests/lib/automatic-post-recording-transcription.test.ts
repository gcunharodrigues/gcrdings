import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const STOP_HOOK = path.join(REPO_ROOT, "frontend/src/hooks/useRecordingStop.ts");
const IMPORT_BACKEND = path.join(REPO_ROOT, "frontend/src-tauri/src/audio/import.rs");
const TRANSCRIPT_TYPES = path.join(REPO_ROOT, "frontend/src/types/index.ts");
const HOME = path.join(REPO_ROOT, "frontend/src/app/page.tsx");

describe("post-recording transcription", () => {
  test("starts batch transcription after saving the recording and before refreshing it", () => {
    const source = fs.readFileSync(STOP_HOOK, "utf8");
    const save = source.indexOf("storageService.saveMeeting(");
    const transcribe = source.indexOf("start_retranscription_command", save);
    const refresh = source.indexOf("refetchMeetings()", save);

    expect(save).toBeGreaterThan(-1);
    expect(transcribe).toBeGreaterThan(save);
    expect(refresh).toBeGreaterThan(transcribe);
  });

  test("starts batch transcription after importing media and before reporting completion", () => {
    const source = fs.readFileSync(IMPORT_BACKEND, "utf8");
    const importMedia = source.indexOf("run_import(");
    const transcribe = source.indexOf("start_retranscription(", importMedia);
    const complete = source.indexOf('app.emit("import-complete"', transcribe);

    expect(importMedia).toBeGreaterThan(-1);
    expect(transcribe).toBeGreaterThan(importMedia);
    expect(complete).toBeGreaterThan(transcribe);
  });

  test("registers captured-origin completion listeners before starting the job", () => {
    const source = fs.readFileSync(STOP_HOOK, "utf8");
    const completeListener = source.indexOf("'retranscription-complete'");
    const errorListener = source.indexOf("'retranscription-error'");
    const start = source.indexOf("start_retranscription_command");

    expect(completeListener).toBeGreaterThan(-1);
    expect(errorListener).toBeGreaterThan(completeListener);
    expect(start).toBeGreaterThan(errorListener);
    expect(source).toContain("failed_origins");
  });

  test("exposes captured-origin provenance to the review timeline", () => {
    const source = fs.readFileSync(TRANSCRIPT_TYPES, "utf8");

    for (const field of [
      "source_origin?: string",
      "speaker_cluster_id?: string",
      "ambiguity_group_id?: string",
      "alignment_decision?: string",
      "audio_start_time?: number",
    ]) {
      expect(source).toContain(field);
    }
  });

  test("keeps capture controls available when only system audio is usable", () => {
    const source = fs.readFileSync(HOME, "utf8");

    expect(source).toContain("hasMicrophone, hasSystemAudio");
    expect(source).toContain("hasMicrophone || hasSystemAudio || isRecording");
  });
});
