import { describe, expect, test } from "bun:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { consumePendingAgentHandoff } from "../../src/components/MeetingDetails/AgentHandoffMenu";
import { agentHandoffRequest } from "../../src/types/verifiable-record";

const ROOT = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

describe("Agent Handoff renderer contract", () => {
  test("starts one pending native action only after a closed render is committed", () => {
    const starts: string[] = [];
    const pending = { format: "markdown", action: "save" } as const;

    let remaining = consumePendingAgentHandoff(true, pending, (action) => {
      starts.push(`${action.format}:${action.action}`);
    });
    expect(remaining).toEqual(pending);
    expect(starts).toEqual([]);

    remaining = consumePendingAgentHandoff(false, remaining, (action) => {
      starts.push(`${action.format}:${action.action}`);
    });
    expect(remaining).toBeNull();
    expect(starts).toEqual(["markdown:save"]);

    remaining = consumePendingAgentHandoff(false, remaining, (action) => {
      starts.push(`${action.format}:${action.action}`);
    });
    expect(remaining).toBeNull();
    expect(starts).toEqual(["markdown:save"]);
  });

  test("supplies only Session identity, format, and explicit native action", () => {
    const request = agentHandoffRequest("session-1", "markdown", "save");
    expect(request).toEqual({ meetingId: "session-1", format: "markdown", action: "save" });
    expect(Object.keys(request).sort()).toEqual(["action", "format", "meetingId"]);
    expect(request).not.toHaveProperty("content");
    expect(request).not.toHaveProperty("path");
  });

  test("exposes an accessible Session-level save and share menu", () => {
    const menuPath = path.join(ROOT, "frontend/src/components/MeetingDetails/AgentHandoffMenu.tsx");
    expect(fs.existsSync(menuPath)).toBe(true);
    const menu = fs.readFileSync(menuPath, "utf8");
    const workspace = fs.readFileSync(path.join(ROOT, "frontend/src/app/meeting-details/page-content.tsx"), "utf8");
    expect(menu).toContain("Agent Handoff");
    expect(menu).toContain("Save Markdown");
    expect(menu).toContain("Save JSON");
    expect(menu).toContain("Share Markdown");
    expect(menu).toContain("Share JSON");
    expect(menu).toContain("<DropdownMenu open={open} onOpenChange={setOpen}>");
    expect(menu).toContain("useEffect(() =>");
    expect(menu).toContain("consumePendingAgentHandoff(open, pending");
    expect(menu).toContain('aria-live="polite"');
    expect(menu).toContain('invoke<AgentHandoffOutcome>("api_export_agent_handoff"');
    expect(workspace).toContain("<AgentHandoffMenu");
    expect(workspace).toContain("meetingId={meeting.id}");
    // Unsaved corrections are saved before exporting rather than disabling the
    // menu, so the export always matches the principal record.
    expect(workspace).toContain("onSaveTranscript={savePrincipal}");
    expect(menu).toContain("await onSaveTranscript()");
  });
});
