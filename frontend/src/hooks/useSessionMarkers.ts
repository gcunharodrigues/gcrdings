'use client';

import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { log } from '@/lib/logger';
import type { SessionMarker } from '@/types/markers';

/** Markers already attached to a saved Session. */
export function useSessionMarkers(meetingId: string | null) {
  const [markers, setMarkers] = useState<SessionMarker[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!meetingId) {
      setMarkers([]);
      return;
    }
    try {
      setMarkers(await invoke<SessionMarker[]>('api_get_session_markers', { meetingId }));
      setError(null);
    } catch (reason) {
      log.warn('[markers] Failed to load Session markers:', reason);
      setError('Markers could not be loaded.');
    }
  }, [meetingId]);

  useEffect(() => { void load(); }, [load]);

  const rename = useCallback(async (markerId: string, label: string | null) => {
    // Optimistic: renaming a marker is trivially reversible and the round trip
    // would otherwise make the input feel laggy.
    setMarkers((current) => current.map((m) => (m.id === markerId ? { ...m, label } : m)));
    try {
      await invoke('api_update_session_marker', { markerId, label });
    } catch (reason) {
      log.warn('[markers] Failed to rename marker:', reason);
      await load();
    }
  }, [load]);

  const remove = useCallback(async (markerId: string) => {
    const previous = markers;
    setMarkers((current) => current.filter((m) => m.id !== markerId));
    try {
      await invoke('api_delete_session_marker', { markerId });
    } catch (reason) {
      log.warn('[markers] Failed to delete marker:', reason);
      setMarkers(previous);
    }
  }, [markers]);

  return { markers, error, reload: load, rename, remove };
}
