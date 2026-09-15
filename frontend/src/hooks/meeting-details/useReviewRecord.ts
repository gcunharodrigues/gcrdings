import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import {
  createReviewRecordState,
  deriveReviewRecordPatch,
  reviewRecordErrorMessage,
  reviewRecordReducer,
  type ReviewRecordAction,
  type ReviewRecordInput,
  type ReviewRecordState,
} from "@/lib/review-record";

interface SessionRecordResponse {
  meeting_id: string;
  record_version: number;
  participants: Array<{ id: string; display_name: string }>;
  transcripts: Array<{
    id: string;
    text: string;
    participant_id: string;
  }>;
}

interface SaveSessionRecordResponse {
  recordVersion?: number;
  record_version?: number;
}

export interface UseReviewRecordResult {
  state: ReviewRecordState | null;
  isLoading: boolean;
  loadError: string | null;
  dispatch: (action: ReviewRecordAction) => void;
  save: () => Promise<boolean>;
  retrySave: () => Promise<boolean>;
  reload: () => Promise<void>;
}

type SessionAction =
  | ReviewRecordAction
  | { type: "loaded"; input: ReviewRecordInput }
  | { type: "reset" };

function sessionReducer(
  state: ReviewRecordState | null,
  action: SessionAction,
): ReviewRecordState | null {
  if (action.type === "reset") return null;
  if (action.type === "loaded") return createReviewRecordState(action.input);
  return state ? reviewRecordReducer(state, action) : state;
}

function fromResponse(response: SessionRecordResponse): ReviewRecordInput {
  return {
    meetingId: response.meeting_id,
    recordVersion: response.record_version,
    participants: response.participants.map(({ id, display_name }) => ({
      id,
      displayName: display_name,
    })),
    transcripts: response.transcripts.map(({ id, text, participant_id }) => ({
      id,
      text,
      participantId: participant_id,
    })),
  };
}

export function useReviewRecord(meetingId: string | null): UseReviewRecordResult {
  const [state, sessionDispatch] = useReducer(sessionReducer, null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const stateRef = useRef(state);
  const savingRef = useRef(false);
  const generationRef = useRef(0);
  const activeState = state?.meetingId === meetingId ? state : null;
  stateRef.current = activeState;

  const load = useCallback(async () => {
    const generation = ++generationRef.current;
    savingRef.current = false;
    sessionDispatch({ type: "reset" });
    if (!meetingId) {
      setIsLoading(false);
      setLoadError(null);
      return;
    }

    setIsLoading(true);
    setLoadError(null);
    try {
      const response = await invoke<SessionRecordResponse>(
        "api_get_session_record",
        { meetingId },
      );
      if (generation === generationRef.current) {
        sessionDispatch({ type: "loaded", input: fromResponse(response) });
      }
    } catch (error) {
      if (generation === generationRef.current) {
        setLoadError(reviewRecordErrorMessage(error));
      }
    } finally {
      if (generation === generationRef.current) setIsLoading(false);
    }
  }, [meetingId]);

  useEffect(() => {
    void load();
    return () => {
      generationRef.current += 1;
      savingRef.current = false;
    };
  }, [load]);

  const save = useCallback(async (): Promise<boolean> => {
    const current = stateRef.current;
    if (!current?.dirty || savingRef.current) return false;

    savingRef.current = true;
    const generation = generationRef.current;
    const savedSnapshot = current.present;
    sessionDispatch({ type: "save-started" });
    try {
      const receipt = await invoke<SaveSessionRecordResponse>(
        "api_save_session_record",
        { ...deriveReviewRecordPatch(current) },
      );
      if (generation !== generationRef.current) return false;
      const recordVersion = receipt.recordVersion ?? receipt.record_version;
      if (recordVersion === undefined) throw new Error("Save returned no record version");
      sessionDispatch({ type: "save-succeeded", recordVersion, savedSnapshot });
      return true;
    } catch (error) {
      if (generation === generationRef.current) {
        sessionDispatch({
          type: "save-failed",
          error: reviewRecordErrorMessage(error),
        });
      }
      return false;
    } finally {
      if (generation === generationRef.current) savingRef.current = false;
    }
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      const key = event.key.toLowerCase();
      if (key === "s") {
        event.preventDefault();
        void save();
      } else if (key === "z") {
        event.preventDefault();
        sessionDispatch({ type: event.shiftKey ? "redo" : "undo" });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [save]);

  // Autosave. Corrections were only ever persisted by an explicit ⌘S or button
  // press, so a crash, a quit or a forgotten tab lost them — and a dirty draft
  // also blocked Agent Handoff, which made the omission compound.
  const AUTOSAVE_DELAY_MS = 2500;

  useEffect(() => {
    if (!activeState?.dirty || activeState.saveStatus.type === "saving") return;
    // A failed save must not be retried on a timer: it would hammer a backend
    // that already refused, and bury the error the user needs to read.
    if (activeState.saveStatus.type === "failed") return;

    const timer = setTimeout(() => { void save(); }, AUTOSAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [activeState?.dirty, activeState?.present, activeState?.saveStatus.type, save]);

  useEffect(() => {
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!stateRef.current?.dirty) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  return {
    state: activeState,
    isLoading,
    loadError,
    dispatch: sessionDispatch,
    save,
    retrySave: save,
    reload: load,
  };
}
