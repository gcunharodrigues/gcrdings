'use client';

import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { log } from '@/lib/logger';
import { formatMarkerOffset, type PendingMarker } from '@/types/markers';

/**
 * Markers dropped while recording, before the Session exists.
 *
 * The buffer lives in Rust rather than in React state so a window reload during
 * a long recording does not lose them. api_save_transcript drains it once the
 * meeting row is created.
 */
export function useRecordingMarkers(isRecording: boolean, activeDurationSeconds: number | null) {
  const [pending, setPending] = useState<PendingMarker[]>([]);

  const refresh = useCallback(async () => {
    try {
      setPending(await invoke<PendingMarker[]>('api_get_pending_markers'));
    } catch (reason) {
      log.warn('[markers] Failed to read pending markers:', reason);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const mark = useCallback(async () => {
    if (!isRecording) return;
    // The active clock excludes paused time, which is what transcript
    // timestamps use — a wall-clock offset would land in the wrong place.
    const offsetMs = Math.max(0, Math.round((activeDurationSeconds ?? 0) * 1000));

    try {
      await invoke<number>('api_add_pending_marker', { offsetMs, label: null });
      await refresh();
      toast.success(`Marked ${formatMarkerOffset(offsetMs)}`, {
        description: 'You can name it after the recording is saved.',
      });
    } catch (reason) {
      log.warn('[markers] Failed to add marker:', reason);
      toast.error('The marker could not be saved.');
    }
  }, [activeDurationSeconds, isRecording, refresh]);

  const clear = useCallback(async () => {
    try {
      await invoke('api_clear_pending_markers');
      setPending([]);
    } catch (reason) {
      log.warn('[markers] Failed to clear pending markers:', reason);
    }
  }, []);

  return { pending, mark, clear, refresh };
}
