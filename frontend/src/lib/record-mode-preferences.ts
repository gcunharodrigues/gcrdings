import {
  DEFAULT_RECORD_MODE,
  isRecordShape,
  isRecordType,
  isRecordVoice,
  type RecordMode,
} from "@/types/record-modes";

/**
 * The reader's default mode is a preference, not Session content, so it lives
 * beside the other local preferences rather than in the record. A Session may
 * override it, and that override is remembered per Session — regenerating one
 * Session as a table must not turn every other Session into a table.
 *
 * Mirrors the storage split already used for summary languages.
 */
export const RECORD_MODE_DEFAULT_KEY = "recordModeDefault";
const RECORD_MODE_SESSION_PREFIX = "recordMode";

function parseMode(raw: string | null): RecordMode | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<RecordMode>;
    if (!isRecordType(parsed.recordType)) return null;
    if (!isRecordVoice(parsed.voice)) return null;
    if (!isRecordShape(parsed.shape)) return null;
    return { recordType: parsed.recordType, voice: parsed.voice, shape: parsed.shape };
  } catch {
    return null;
  }
}

export function readDefaultRecordMode(): RecordMode {
  if (typeof window === "undefined") return DEFAULT_RECORD_MODE;
  try {
    return parseMode(window.localStorage.getItem(RECORD_MODE_DEFAULT_KEY)) ?? DEFAULT_RECORD_MODE;
  } catch (error) {
    console.warn("[recordMode] Failed to read the default mode:", error);
    return DEFAULT_RECORD_MODE;
  }
}

export function writeDefaultRecordMode(mode: RecordMode): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(RECORD_MODE_DEFAULT_KEY, JSON.stringify(mode));
  } catch (error) {
    console.warn("[recordMode] Failed to store the default mode:", error);
  }
}

function sessionKey(meetingId: string): string {
  return `${RECORD_MODE_SESSION_PREFIX}:${meetingId}`;
}

/** Falls back to the reader's default when a Session has no override. */
export function readSessionRecordMode(meetingId: string): RecordMode {
  if (typeof window === "undefined") return DEFAULT_RECORD_MODE;
  try {
    return parseMode(window.localStorage.getItem(sessionKey(meetingId))) ?? readDefaultRecordMode();
  } catch (error) {
    console.warn("[recordMode] Failed to read the Session mode:", error);
    return readDefaultRecordMode();
  }
}

export function writeSessionRecordMode(meetingId: string, mode: RecordMode): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(sessionKey(meetingId), JSON.stringify(mode));
  } catch (error) {
    console.warn("[recordMode] Failed to store the Session mode:", error);
  }
}

export function clearSessionRecordMode(meetingId: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(sessionKey(meetingId));
  } catch (error) {
    console.warn("[recordMode] Failed to clear the Session mode:", error);
  }
}
