import { describe, expect, test } from "bun:test";
import type { Transcript } from "../../src/types";
import { mergePaginatedTranscripts } from "../../src/hooks/usePaginatedTranscripts";

const source = async (path: string) => Bun.file(new URL(path, import.meta.url)).text();

describe("editable Evidence desk contract", () => {
  test("keeps existing paginated passages when a duplicate page arrives", () => {
    const draft = { id: "a", text: "unsaved exact text", timestamp: "00:00" } satisfies Transcript;
    const stale = { id: "a", text: "server text", timestamp: "00:00" } satisfies Transcript;
    const next = { id: "b", text: "next", timestamp: "00:01", audio_start_time: 1 } satisfies Transcript;

    expect(mergePaginatedTranscripts([draft], [stale, next])).toEqual([draft, next]);
  });

  test("renders the accepted columns and current grounded evidence", async () => {
    const [page, summary, findings] = await Promise.all([
      source("../../src/app/meeting-details/page-content.tsx"),
      source("../../src/components/MeetingDetails/SummaryPanel.tsx"),
      source("../../src/types/verifiable-record.ts"),
    ]);

    expect(page.match(/data-review-column=/g)).toHaveLength(2);
    // Evidence lives inside the findings it belongs to; a separate column only
    // restated the same items with the same seek action.
    expect(page).not.toContain("<EvidenceStatusPanel");
    expect(summary).toContain("generatedFindings");
    expect(summary).toContain("Play evidence at");
    expect(summary).toContain("resolves to a playable principal-transcript passage");
    for (const state of ["unavailable", "failed", "stale"]) expect(findings).toContain(`status === "${state}"`);
    expect(findings).toContain("Local findings are pending.");
  });

  test("uses exact editable text, participant identity, and no live cleaner", async () => {
    const transcript = await source("../../src/components/MeetingDetails/TranscriptPanel.tsx");

    expect(transcript).toContain("value={transcript.text}");
    expect(transcript).toContain('type: "transcript-assigned"');
    expect(transcript).not.toContain("VirtualizedTranscriptView");
    expect(transcript).not.toContain("TranscriptView");
    expect(transcript).not.toMatch(/stop.?word|cleanTranscript/i);
  });

  test("routes internal navigation through the shared dirty guard", async () => {
    const [provider, sidebar, recordingStop] = await Promise.all([
      source("../../src/components/Sidebar/SidebarProvider.tsx"),
      source("../../src/components/Sidebar/index.tsx"),
      source("../../src/hooks/useRecordingStop.ts"),
    ]);

    expect(provider).toContain("hasUnsavedReviewChanges");
    expect(provider).toContain("hasUnsavedReviewChangesRef.current");
    // The guard is answered in-app; nothing may fall back to the OS confirm sheet.
    expect(provider).not.toContain("window.confirm");
    expect(provider).toContain("<ConfirmationModal");
    expect(provider).toContain("setPendingNavigation(path)");
    expect(sidebar).not.toContain("router.push(");
    expect(sidebar).toContain("navigate(basePath)");
    expect(recordingStop).not.toContain("router.push(");
    expect(recordingStop).toContain("navigate(`/meeting-details");
  });

  test("guards transcript replacement and disables edits while saving", async () => {
    const [page, transcript, participants, toolbar] = await Promise.all([
      source("../../src/app/meeting-details/page-content.tsx"),
      source("../../src/components/MeetingDetails/TranscriptPanel.tsx"),
      source("../../src/components/MeetingDetails/ParticipantsPanel.tsx"),
      source("../../src/components/MeetingDetails/TranscriptButtonGroup.tsx"),
    ]);

    expect(page).toContain("confirmDestructiveOperation");
    expect(page).toContain("hasUnsavedTranscript");
    expect(page).toContain("savePrincipal");
    expect(toolbar).toContain("await confirmDestructiveOperation()");
    expect(transcript).toContain("disabled={saving}");
    expect(participants).toContain("disabled={saving}");
    expect(participants).toContain('role="alert"');
  });

  test("offers conflict recovery and guards import navigation", async () => {
    const [transcript, importDialog, audio] = await Promise.all([
      source("../../src/components/MeetingDetails/TranscriptPanel.tsx"),
      source("../../src/components/ImportAudio/ImportAudioDialog.tsx"),
      source("../../src/hooks/useAudioPlayer.ts"),
    ]);

    expect(transcript).toContain("Reload latest");
    expect(transcript).toContain("Unknown participant");
    expect(importDialog).not.toContain("router.push(");
    expect(importDialog).toContain("navigate(`/meeting-details");
    expect(audio).toContain("catch (reason)");
    expect(audio).toContain("Failed to play Session audio.");
  });
});
