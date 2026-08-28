"use client";

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppWindowMac, FolderOpen, Monitor, TriangleAlert } from "lucide-react";
import { log } from "@/lib/logger";
import { formatMarkerOffset } from "@/types/markers";

interface SessionScreenRecording {
  id: string;
  filePath: string;
  startedAtOffsetMs: number;
  targetKind: 'display' | 'window';
  targetLabel: string | null;
  createdAt: string;
  fileExists: boolean;
}

/**
 * The screen recordings captured while this Session was being recorded.
 *
 * Until this existed the video was written and then never referenced again —
 * it played nowhere and appeared in no list, which is indistinguishable from
 * the recording having failed.
 */
export function ScreenRecordingsPanel({ meetingId }: { meetingId: string }) {
  const [recordings, setRecordings] = useState<SessionScreenRecording[]>([]);

  const load = useCallback(async () => {
    try {
      setRecordings(
        await invoke<SessionScreenRecording[]>('api_get_session_screen_recordings', { meetingId }),
      );
    } catch (reason) {
      log.warn('[screen] Could not load Session recordings:', reason);
    }
  }, [meetingId]);

  useEffect(() => { void load(); }, [load]);

  if (recordings.length === 0) return null;

  return (
    <section aria-labelledby="screen-recordings-heading" className="shrink-0 border-t border-border p-4">
      <h2
        id="screen-recordings-heading"
        className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground"
      >
        <Monitor className="size-3" aria-hidden="true" />
        Screen ({recordings.length})
      </h2>

      <ul className="space-y-2">
        {recordings.map((recording) => (
          <li key={recording.id} className="rounded-lg border border-border p-2">
            <div className="flex items-center gap-2">
              {recording.targetKind === 'window' ? (
                <AppWindowMac className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              ) : (
                <Monitor className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
              )}
              <span className="min-w-0 flex-1 truncate text-sm" title={recording.targetLabel ?? ''}>
                {recording.targetLabel ?? 'Screen recording'}
              </span>
              {recording.fileExists && (
                <button
                  type="button"
                  onClick={() => void invoke('api_reveal_screen_recording', { recordingId: recording.id })}
                  aria-label="Show the recording in Finder"
                  className="shrink-0 rounded p-1 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
                >
                  <FolderOpen className="size-3.5" aria-hidden="true" />
                </button>
              )}
            </div>

            {recording.fileExists ? (
              <>
                <video
                  src={`asset://localhost/${encodeURI(recording.filePath)}`}
                  controls
                  preload="metadata"
                  className="mt-2 w-full rounded border border-border bg-black"
                />
                {recording.startedAtOffsetMs > 0 && (
                  <p className="mt-1 text-[11px] text-muted-foreground">
                    Started {formatMarkerOffset(recording.startedAtOffsetMs)} into the audio.
                  </p>
                )}
              </>
            ) : (
              <p role="alert" className="mt-1.5 flex items-center gap-1.5 text-xs text-amber-800">
                <TriangleAlert className="size-3" aria-hidden="true" />
                The file was moved or deleted.
              </p>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}
