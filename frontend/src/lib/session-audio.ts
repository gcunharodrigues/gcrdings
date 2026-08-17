export type SeekValidation =
  | { ok: true; seconds: number }
  | { ok: false; reason: "not-ready" | "invalid-time" | "past-end" };

export function validateSessionSeek(
  seconds: number,
  duration: number,
  ready: boolean,
): SeekValidation {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return { ok: false, reason: "invalid-time" };
  }
  if (!ready || !Number.isFinite(duration) || duration <= 0) {
    return { ok: false, reason: "not-ready" };
  }
  return seconds > duration
    ? { ok: false, reason: "past-end" }
    : { ok: true, seconds };
}
