export interface ReviewParticipant {
  id: string;
  displayName: string;
}

export interface ReviewTranscript {
  id: string;
  text: string;
  participantId: string;
}

export interface ReviewRecordSnapshot {
  participants: Record<string, ReviewParticipant>;
  transcripts: Record<string, ReviewTranscript>;
  transcriptOrder: string[];
}

export interface ReviewRecordEdit {
  type: "participant-renamed" | "transcript-text-edited" | "transcript-assigned";
  id: string;
  before: string;
  after: string;
}

export type ReviewRecordSaveStatus =
  | { type: "idle" }
  | { type: "saving" }
  | { type: "failed"; error: string };

export interface ReviewRecordState {
  meetingId: string;
  recordVersion: number;
  baseline: ReviewRecordSnapshot;
  present: ReviewRecordSnapshot;
  past: ReviewRecordEdit[];
  future: ReviewRecordEdit[];
  dirty: boolean;
  saveStatus: ReviewRecordSaveStatus;
}

export interface ReviewRecordInput {
  meetingId: string;
  recordVersion: number;
  participants: ReviewParticipant[];
  transcripts: ReviewTranscript[];
}

export interface ReviewRecordPatch {
  meetingId: string;
  expectedVersion: number;
  participantChanges: Array<{ id: string; displayName: string }>;
  transcriptChanges: Array<{
    id: string;
    text?: string;
    participantId?: string;
  }>;
}

export type ReviewRecordAction =
  | { type: "participant-renamed"; participantId: string; displayName: string }
  | { type: "transcript-text-edited"; transcriptId: string; text: string }
  | { type: "transcript-assigned"; transcriptId: string; participantId: string }
  | { type: "transcripts-merged"; transcripts: ReviewTranscript[] }
  | { type: "undo" }
  | { type: "redo" }
  | { type: "save-started" }
  | {
      type: "save-succeeded";
      recordVersion: number;
      savedSnapshot?: ReviewRecordSnapshot;
    }
  | { type: "save-failed"; error: string };

const ERROR_MESSAGES: Record<string, string> = {
  revision_conflict:
    "This Session changed elsewhere. Reload the latest record before saving.",
  invalid_change: "One or more edits are invalid. Review them and try again.",
  not_found: "This Session no longer exists.",
  storage: "The Session could not be saved. Check storage access and try again.",
};

export function reviewRecordErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const { code, message } = error as { code?: unknown; message?: unknown };
    if (typeof message === "string" && message.trim()) return message;
    if (typeof code === "string" && ERROR_MESSAGES[code]) return ERROR_MESSAGES[code];
  }
  return "Unexpected Session record error.";
}

function snapshot(input: ReviewRecordInput): ReviewRecordSnapshot {
  return {
    participants: Object.fromEntries(
      input.participants.map((participant) => [participant.id, { ...participant }]),
    ),
    transcripts: Object.fromEntries(
      input.transcripts.map((transcript) => [transcript.id, { ...transcript }]),
    ),
    transcriptOrder: input.transcripts.map(({ id }) => id),
  };
}

export function createReviewRecordState(input: ReviewRecordInput): ReviewRecordState {
  return {
    meetingId: input.meetingId,
    recordVersion: input.recordVersion,
    baseline: snapshot(input),
    present: snapshot(input),
    past: [],
    future: [],
    dirty: false,
    saveStatus: { type: "idle" },
  };
}

export function reviewRecordReducer(
  state: ReviewRecordState,
  action: ReviewRecordAction,
): ReviewRecordState {
  switch (action.type) {
    case "participant-renamed": {
      const participant = state.present.participants[action.participantId];
      if (
        !participant ||
        action.displayName.trim() === "" ||
        participant.displayName === action.displayName
      ) {
        return state;
      }
      return edit(state, {
        type: action.type,
        id: action.participantId,
        before: participant.displayName,
        after: action.displayName,
      });
    }
    case "transcript-text-edited": {
      const transcript = state.present.transcripts[action.transcriptId];
      if (!transcript || transcript.text === action.text) return state;
      return edit(state, {
        type: action.type,
        id: action.transcriptId,
        before: transcript.text,
        after: action.text,
      });
    }
    case "transcript-assigned": {
      const transcript = state.present.transcripts[action.transcriptId];
      if (
        !transcript ||
        !state.present.participants[action.participantId] ||
        transcript.participantId === action.participantId
      ) {
        return state;
      }
      return edit(state, {
        type: action.type,
        id: action.transcriptId,
        before: transcript.participantId,
        after: action.participantId,
      });
    }
    case "transcripts-merged": {
      const additions = action.transcripts.filter(
        ({ id }) => !state.present.transcripts[id],
      );
      if (additions.length === 0) return state;

      const merged = Object.fromEntries(
        additions.map((transcript) => [transcript.id, { ...transcript }]),
      );
      const transcriptOrder = [
        ...state.present.transcriptOrder,
        ...additions.map(({ id }) => id),
      ];
      return {
        ...state,
        baseline: {
          ...state.baseline,
          transcripts: { ...state.baseline.transcripts, ...merged },
          transcriptOrder,
        },
        present: {
          ...state.present,
          transcripts: { ...state.present.transcripts, ...merged },
          transcriptOrder,
        },
      };
    }
    case "undo": {
      const command = state.past.at(-1);
      if (!command) return state;
      const present = applyEdit(state.present, command, "before");
      return {
        ...state,
        present,
        past: state.past.slice(0, -1),
        future: [command, ...state.future],
        dirty: snapshotsDiffer(state.baseline, present),
      };
    }
    case "redo": {
      const command = state.future[0];
      if (!command) return state;
      const present = applyEdit(state.present, command, "after");
      return {
        ...state,
        present,
        past: [...state.past, command],
        future: state.future.slice(1),
        dirty: snapshotsDiffer(state.baseline, present),
      };
    }
    case "save-started":
      return { ...state, saveStatus: { type: "saving" } };
    case "save-succeeded": {
      const baseline = action.savedSnapshot ?? state.present;
      const past = commandsBetween(baseline, state.present);
      return {
        ...state,
        recordVersion: action.recordVersion,
        baseline,
        past,
        future: [],
        dirty: past.length > 0,
        saveStatus: { type: "idle" },
      };
    }
    case "save-failed":
      return {
        ...state,
        saveStatus: { type: "failed", error: action.error },
      };
  }
}

function edit(state: ReviewRecordState, command: ReviewRecordEdit): ReviewRecordState {
  const present = applyEdit(state.present, command, "after");
  return {
    ...state,
    present,
    past: [...state.past, command],
    future: [],
    dirty: snapshotsDiffer(state.baseline, present),
  };
}

function applyEdit(
  value: ReviewRecordSnapshot,
  command: ReviewRecordEdit,
  side: "before" | "after",
): ReviewRecordSnapshot {
  if (command.type === "participant-renamed") {
    return {
      ...value,
      participants: {
        ...value.participants,
        [command.id]: {
          ...value.participants[command.id],
          displayName: command[side],
        },
      },
    };
  }

  const field =
    command.type === "transcript-text-edited" ? "text" : "participantId";
  return {
    ...value,
    transcripts: {
      ...value.transcripts,
      [command.id]: {
        ...value.transcripts[command.id],
        [field]: command[side],
      },
    },
  };
}

function snapshotsDiffer(
  baseline: ReviewRecordSnapshot,
  present: ReviewRecordSnapshot,
): boolean {
  return (
    Object.keys(present.participants).some(
      (id) =>
        baseline.participants[id]?.displayName !==
        present.participants[id].displayName,
    ) ||
    present.transcriptOrder.some((id) => {
      const before = baseline.transcripts[id];
      const after = present.transcripts[id];
      return (
        !before ||
        before.text !== after.text ||
        before.participantId !== after.participantId
      );
    })
  );
}

function commandsBetween(
  baseline: ReviewRecordSnapshot,
  present: ReviewRecordSnapshot,
): ReviewRecordEdit[] {
  const commands: ReviewRecordEdit[] = [];
  for (const [id, participant] of Object.entries(present.participants)) {
    const before = baseline.participants[id]?.displayName;
    if (before !== undefined && before !== participant.displayName) {
      commands.push({
        type: "participant-renamed",
        id,
        before,
        after: participant.displayName,
      });
    }
  }
  for (const id of present.transcriptOrder) {
    const before = baseline.transcripts[id];
    const after = present.transcripts[id];
    if (!before) continue;
    if (before.text !== after.text) {
      commands.push({
        type: "transcript-text-edited",
        id,
        before: before.text,
        after: after.text,
      });
    }
    if (before.participantId !== after.participantId) {
      commands.push({
        type: "transcript-assigned",
        id,
        before: before.participantId,
        after: after.participantId,
      });
    }
  }
  return commands;
}

export function deriveReviewRecordPatch(
  state: ReviewRecordState,
): ReviewRecordPatch {
  const participantChanges = Object.values(state.present.participants)
    .filter(
      (participant) =>
        state.baseline.participants[participant.id]?.displayName !==
        participant.displayName,
    )
    .map(({ id, displayName }) => ({ id, displayName }));

  const transcriptChanges = state.present.transcriptOrder.flatMap((id) => {
    const before = state.baseline.transcripts[id];
    const after = state.present.transcripts[id];
    if (!before || !after) return [];
    const change: ReviewRecordPatch["transcriptChanges"][number] = { id };
    if (before.text !== after.text) change.text = after.text;
    if (before.participantId !== after.participantId) {
      change.participantId = after.participantId;
    }
    return Object.keys(change).length > 1 ? [change] : [];
  });

  return {
    meetingId: state.meetingId,
    expectedVersion: state.recordVersion,
    participantChanges,
    transcriptChanges,
  };
}
