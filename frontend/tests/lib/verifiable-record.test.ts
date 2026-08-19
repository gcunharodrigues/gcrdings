import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { generationMessage } from "../../src/types/verifiable-record";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("verifiable record UI contract", () => {
  test("maps typed unavailable and stale states without provider fallback", () => {
    expect(generationMessage("unavailable", "apple_intelligence_disabled")).toBe(
      "Apple Intelligence is disabled. The Session remains available; local findings are pending.",
    );
    expect(generationMessage("stale")).toBe(
      "The principal transcript changed. Generate current findings again.",
    );
  });

  test("maps timeout and cancellation without exposing private input", () => {
    expect(generationMessage("failed", "timeout")).toBe(
      "Local generation timed out. Try again when ready.",
    );
    expect(generationMessage("pending", "cancelled")).toBe(
      "Local generation was cancelled. No Session content changed.",
    );
  });

  test("ships only the Apple helper and exposes no llama or Ollama findings control", () => {
    const cargo = fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8");
    const tauri = fs.readFileSync(path.join(ROOT, "frontend/src-tauri/tauri.conf.json"), "utf8");
    const controls = [
      "frontend/src/components/MeetingDetails/SummaryPanel.tsx",
      "frontend/src/components/MeetingDetails/shapes/FindingViews.tsx",
      "frontend/src/components/MeetingDetails/shapes/ChartView.tsx",
      "frontend/src/components/SummaryModelSettings.tsx",
      "frontend/src/app/meeting-details/page-content.tsx",
    ].map((file) => fs.readFileSync(path.join(ROOT, file), "utf8")).join("\n");
    expect(cargo).not.toContain("llama-helper");
    expect(tauri).toContain("binaries/foundation-helper");
    expect(tauri).not.toContain("binaries/llama-helper");
    expect(controls.toLowerCase()).not.toMatch(/llama|ollama/);
    expect(controls).toContain("Generate local findings");
    expect(controls).toContain("Play evidence at");
  });
});
