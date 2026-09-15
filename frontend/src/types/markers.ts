/** A moment flagged during recording, kept against the active recording clock. */
export interface SessionMarker {
  id: string;
  meetingId: string;
  offsetMs: number;
  label: string | null;
  createdAt: string;
}

/** A marker captured before the Session exists in the database. */
export interface PendingMarker {
  offsetMs: number;
  label: string | null;
}

export function formatMarkerOffset(offsetMs: number): string {
  const totalSeconds = Math.max(0, Math.floor(offsetMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) return `${minutes}:${seconds.toString().padStart(2, '0')}`;
  const hours = Math.floor(minutes / 60);
  return `${hours}:${(minutes % 60).toString().padStart(2, '0')}:${seconds.toString().padStart(2, '0')}`;
}
