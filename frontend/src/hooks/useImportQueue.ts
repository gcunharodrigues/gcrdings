'use client';

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import { log } from '@/lib/logger';

export interface QueuedImport {
  id: string;
  path: string;
  title: string;
  language: string | null;
  model: string | null;
  provider: string | null;
}

export interface ImportQueueState {
  active: QueuedImport | null;
  pending: QueuedImport[];
  failed: string[];
  completed: number;
}

interface StageProgress {
  stage: string;
  progress: number;
  message: string;
}

const EMPTY: ImportQueueState = { active: null, pending: [], failed: [], completed: 0 };

/**
 * An estimate is worth showing only once it means something.
 *
 * At 2% after one second the arithmetic says "forty minutes" and is wrong by an
 * order of magnitude; a number that swings wildly is worse than no number.
 */
function estimateRemainingMs(elapsedMs: number, percent: number): number | null {
  if (percent < 8 || elapsedMs < 3000) return null;
  const total = elapsedMs / (percent / 100);
  const remaining = total - elapsedMs;
  return remaining > 0 ? remaining : null;
}

export function formatRemaining(remainingMs: number): string {
  const seconds = Math.round(remainingMs / 1000);
  if (seconds < 10) return 'a few seconds left';
  if (seconds < 60) return `about ${Math.round(seconds / 5) * 5}s left`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `about ${minutes} min left`;
  return `about ${Math.round(minutes / 60)}h left`;
}

/**
 * The import queue, its current stage, and how long the active item has left.
 *
 * Importing is serialised in Rust, but that is a scheduling constraint — it is
 * not a reason to hold the window hostage, which is what the modal dialog did.
 */
export function useImportQueue() {
  const [queue, setQueue] = useState<ImportQueueState>(EMPTY);
  const [stage, setStage] = useState<StageProgress | null>(null);
  const [remainingMs, setRemainingMs] = useState<number | null>(null);
  const startedAtRef = useRef<number | null>(null);
  const activeIdRef = useRef<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setQueue(await invoke<ImportQueueState>('api_get_import_queue'));
    } catch (reason) {
      log.warn('[import-queue] Could not read the queue:', reason);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  useEffect(() => {
    const subscriptions = [
      listen<ImportQueueState>('import-queue-changed', (event) => setQueue(event.payload)),
      listen<ImportQueueState>('import-queue-drained', (event) => {
        setQueue(event.payload);
        setStage(null);
        setRemainingMs(null);
      }),
      listen<StageProgress>('import-progress', (event) => setStage(event.payload)),
    ];
    return () => {
      void Promise.all(subscriptions).then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);

  // The clock restarts per item, not per queue: a small file after a large one
  // would otherwise inherit the big one's elapsed time and report nonsense.
  useEffect(() => {
    const activeId = queue.active?.id ?? null;
    if (activeId !== activeIdRef.current) {
      activeIdRef.current = activeId;
      startedAtRef.current = activeId ? Date.now() : null;
      setRemainingMs(null);
    }
  }, [queue.active?.id]);

  useEffect(() => {
    if (!stage || startedAtRef.current === null) return;
    setRemainingMs(estimateRemainingMs(Date.now() - startedAtRef.current, stage.progress));
  }, [stage]);

  const enqueue = useCallback(async (items: Array<Omit<QueuedImport, 'id'> & { id?: string }>) => {
    try {
      const next = await invoke<ImportQueueState>('api_enqueue_imports', {
        items: items.map((item) => ({ id: '', ...item })),
      });
      setQueue(next);
      return true;
    } catch (reason) {
      log.warn('[import-queue] Could not enqueue:', reason);
      return false;
    }
  }, []);

  const remove = useCallback(async (itemId: string) => {
    try {
      setQueue(await invoke<ImportQueueState>('api_remove_queued_import', { itemId }));
    } catch (reason) {
      log.warn('[import-queue] Could not remove the queued item:', reason);
    }
  }, []);

  return { queue, stage, remainingMs, enqueue, remove, refresh };
}
