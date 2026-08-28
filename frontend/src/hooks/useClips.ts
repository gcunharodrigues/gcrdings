'use client';

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { log } from '@/lib/logger';

export type ClipStatus = 'pending' | 'rendering' | 'ready' | 'failed';

export interface Clip {
  id: string;
  meetingId: string;
  title: string | null;
  startMs: number;
  endMs: number;
  status: ClipStatus;
  filePath: string | null;
  errorCode: string | null;
  createdAt: string;
}

export function useClips(meetingId: string | null) {
  const [clips, setClips] = useState<Clip[]>([]);

  const reload = useCallback(async () => {
    if (!meetingId) {
      setClips([]);
      return;
    }
    try {
      setClips(await invoke<Clip[]>('api_list_clips', { meetingId }));
    } catch (reason) {
      log.warn('[clips] Could not load clips:', reason);
    }
  }, [meetingId]);

  useEffect(() => { void reload(); }, [reload]);

  // Cutting runs in the background, so the list refreshes when a clip's state
  // moves rather than leaving a row stuck on "rendering".
  useEffect(() => {
    const subscription = listen('clip-status-changed', () => void reload());
    return () => { void subscription.then((unlisten) => unlisten()); };
  }, [reload]);

  const create = useCallback(async (startMs: number, endMs: number, title?: string) => {
    if (!meetingId) return false;
    try {
      await invoke('api_create_clip', { meetingId, startMs, endMs, title: title ?? null });
      await reload();
      return true;
    } catch (reason) {
      log.warn('[clips] Could not create the clip:', reason);
      const code = (reason as { code?: string })?.code;
      toast.error(
        code === 'source_unavailable'
          ? 'This Session has no media to cut from.'
          : code === 'empty_range'
            ? 'Pick two different markers.'
            : 'The clip could not be created.',
      );
      return false;
    }
  }, [meetingId, reload]);

  const rename = useCallback(async (clipId: string, title: string | null) => {
    setClips((current) => current.map((c) => (c.id === clipId ? { ...c, title } : c)));
    try {
      await invoke('api_rename_clip', { clipId, title });
    } catch (reason) {
      log.warn('[clips] Could not rename the clip:', reason);
      await reload();
    }
  }, [reload]);

  const remove = useCallback(async (clipId: string) => {
    try {
      await invoke('api_delete_clip', { clipId });
      await reload();
    } catch (reason) {
      log.warn('[clips] Could not delete the clip:', reason);
    }
  }, [reload]);

  const reveal = useCallback(async (clipId: string) => {
    try {
      await invoke('api_reveal_clip', { clipId });
    } catch (reason) {
      log.warn('[clips] Could not reveal the clip:', reason);
      toast.error('The clip file could not be found.');
    }
  }, []);

  return { clips, reload, create, rename, remove, reveal };
}
