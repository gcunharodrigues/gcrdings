import { describe, expect, test } from "bun:test";
import {
  createReviewRecordState,
  deriveReviewRecordPatch,
  reviewRecordErrorMessage,
  reviewRecordReducer,
  type ReviewRecordAction,
  type ReviewRecordState,
} from "../../src/lib/review-record";

describe("reviewRecordErrorMessage", () => {
  test("renders structured backend failures without object coercion", () => {
    expect(reviewRecordErrorMessage({ code: "revision_conflict" })).toBe(
      "This Session changed elsewhere. Reload the latest record before saving.",
    );
    expect(reviewRecordErrorMessage({ code: "storage" })).toBe(
      "The Session could not be saved. Check storage access and try again.",
    );
    expect(reviewRecordErrorMessage({ message: "Native failure" })).toBe(
      "Native failure",
    );
    expect(reviewRecordErrorMessage({ unexpected: true })).toBe(
      "Unexpected Session record error.",
    );
  });
});

function initialState(): ReviewRecordState {
  return createReviewRecordState({
    meetingId: "meeting-1",
    recordVersion: 4,
    participants: [
      { id: "participant-a", displayName: "Speaker 1" },
      { id: "participant-b", displayName: "Speaker 2" },
    ],
    transcripts: [
      { id: "block-1", text: "First", participantId: "participant-a" },
      { id: "block-2", text: "Second", participantId: "participant-a" },
    ],
  });
}

function reduce(
  state: ReviewRecordState,
  ...actions: ReviewRecordAction[]
): ReviewRecordState {
  return actions.reduce(reviewRecordReducer, state);
}

describe("reviewRecordReducer", () => {
  test("renames one participant identity used by every assigned block", () => {
    const state = reduce(initialState(), {
      type: "participant-renamed",
      participantId: "participant-a",
      displayName: "Alice",
    });

    expect(state.present.participants["participant-a"].displayName).toBe("Alice");
    expect(
      state.present.transcriptOrder.map(
        (id) => state.present.participants[state.present.transcripts[id].participantId].displayName,
      ),
    ).toEqual(["Alice", "Alice"]);
  });

  test("edits exact transcript text", () => {
    const state = reduce(initialState(), {
      type: "transcript-text-edited",
      transcriptId: "block-1",
      text: "Corrected exact text",
    });

    expect(state.present.transcripts["block-1"].text).toBe("Corrected exact text");
    expect(state.present.transcripts["block-2"].text).toBe("Second");
  });

  test("assigns a transcript to another participant", () => {
    const state = reduce(initialState(), {
      type: "transcript-assigned",
      transcriptId: "block-1",
      participantId: "participant-b",
    });

    expect(state.present.transcripts["block-1"].participantId).toBe("participant-b");
  });

  test("merges a paginated block without overwriting an existing draft", () => {
    const draft = reduce(initialState(), {
      type: "transcript-text-edited",
      transcriptId: "block-1",
      text: "Unsaved draft",
    });
    const state = reduce(draft, {
      type: "transcripts-merged",
      transcripts: [
        { id: "block-1", text: "Stale server text", participantId: "participant-a" },
        { id: "block-3", text: "Third", participantId: "participant-b" },
      ],
    });

    expect(state.present.transcripts["block-1"].text).toBe("Unsaved draft");
    expect(state.present.transcripts["block-3"]).toEqual({
      id: "block-3",
      text: "Third",
      participantId: "participant-b",
    });
    expect(state.present.transcriptOrder).toEqual(["block-1", "block-2", "block-3"]);
  });

  test("tracks dirty state and undo/redo boundaries in edit order", () => {
    const edited = reduce(
      initialState(),
      {
        type: "participant-renamed",
        participantId: "participant-a",
        displayName: "Alice",
      },
      {
        type: "transcript-text-edited",
        transcriptId: "block-1",
        text: "Corrected",
      },
      {
        type: "transcript-assigned",
        transcriptId: "block-1",
        participantId: "participant-b",
      },
    );

    expect(edited.dirty).toBe(true);
    expect(edited.past).toHaveLength(3);
    expect(edited.future).toHaveLength(0);

    const undone = reduce(edited, { type: "undo" }, { type: "undo" }, { type: "undo" });
    expect(undone.present).toEqual(undone.baseline);
    expect(undone.dirty).toBe(false);
    expect(undone.past).toHaveLength(0);
    expect(reviewRecordReducer(undone, { type: "undo" })).toBe(undone);

    const redone = reduce(undone, { type: "redo" }, { type: "redo" }, { type: "redo" });
    expect(redone.present).toEqual(edited.present);
    expect(redone.future).toHaveLength(0);
    expect(reviewRecordReducer(redone, { type: "redo" })).toBe(redone);

    const oneUndo = reviewRecordReducer(redone, { type: "undo" });
    const divergentEdit = reviewRecordReducer(oneUndo, {
      type: "transcript-text-edited",
      transcriptId: "block-2",
      text: "New path",
    });
    expect(divergentEdit.future).toHaveLength(0);
  });

  test("establishes a new clean baseline after save success", () => {
    const edited = reduce(initialState(), {
      type: "transcript-text-edited",
      transcriptId: "block-1",
      text: "Saved text",
    });
    const saving = reviewRecordReducer(edited, { type: "save-started" });
    const saved = reviewRecordReducer(saving, {
      type: "save-succeeded",
      recordVersion: 5,
    });

    expect(saving.saveStatus).toEqual({ type: "saving" });
    expect(saved.recordVersion).toBe(5);
    expect(saved.baseline).toEqual(saved.present);
    expect(saved.dirty).toBe(false);
    expect(saved.past).toHaveLength(0);
    expect(saved.future).toHaveLength(0);
    expect(saved.saveStatus).toEqual({ type: "idle" });
  });

  test("keeps edits and history available after save failure", () => {
    const edited = reduce(initialState(), {
      type: "participant-renamed",
      participantId: "participant-a",
      displayName: "Alice",
    });
    const failed = reduce(
      edited,
      { type: "save-started" },
      { type: "save-failed", error: "Version conflict" },
    );

    expect(failed.present).toEqual(edited.present);
    expect(failed.baseline).toEqual(edited.baseline);
    expect(failed.past).toEqual(edited.past);
    expect(failed.dirty).toBe(true);
    expect(failed.saveStatus).toEqual({ type: "failed", error: "Version conflict" });
  });

  test("allows duplicate participant display names", () => {
    const state = reduce(initialState(), {
      type: "participant-renamed",
      participantId: "participant-b",
      displayName: "Speaker 1",
    });

    expect(state.present.participants["participant-a"].displayName).toBe("Speaker 1");
    expect(state.present.participants["participant-b"].displayName).toBe("Speaker 1");
    expect(state.dirty).toBe(true);
  });

  test("rejects blank participant names without adding history", () => {
    const initial = initialState();
    const state = reduce(initial, {
      type: "participant-renamed",
      participantId: "participant-a",
      displayName: "   ",
    });

    expect(state).toBe(initial);
    expect(state.present.participants["participant-a"].displayName).toBe("Speaker 1");
    expect(state.past).toHaveLength(0);
  });

  test("allows explicit empty transcript text", () => {
    const state = reduce(initialState(), {
      type: "transcript-text-edited",
      transcriptId: "block-1",
      text: "",
    });

    expect(state.present.transcripts["block-1"].text).toBe("");
    expect(state.dirty).toBe(true);
  });

  test("derives only changed participant and transcript fields", () => {
    const state = reduce(
      initialState(),
      {
        type: "participant-renamed",
        participantId: "participant-a",
        displayName: "Alice",
      },
      {
        type: "transcript-text-edited",
        transcriptId: "block-1",
        text: "Corrected",
      },
      {
        type: "transcript-assigned",
        transcriptId: "block-2",
        participantId: "participant-b",
      },
    );

    expect(deriveReviewRecordPatch(state)).toEqual({
      meetingId: "meeting-1",
      expectedVersion: 4,
      participantChanges: [{ id: "participant-a", displayName: "Alice" }],
      transcriptChanges: [
        { id: "block-1", text: "Corrected" },
        { id: "block-2", participantId: "participant-b" },
      ],
    });
  });

  test("derives an empty patch when edits return to the baseline", () => {
    const state = reduce(
      initialState(),
      {
        type: "transcript-text-edited",
        transcriptId: "block-1",
        text: "Draft",
      },
      { type: "undo" },
    );

    expect(deriveReviewRecordPatch(state)).toEqual({
      meetingId: "meeting-1",
      expectedVersion: 4,
      participantChanges: [],
      transcriptChanges: [],
    });
  });
});
