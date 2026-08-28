'use client';

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { log } from '@/lib/logger';
import {
  screenCaptureErrorMessage,
  type CaptureTarget,
  type CaptureTargets,
  type ScreenRecordingState,
} from '@/types/screenCapture';

const IDLE: ScreenRecordingState = {
  isRecording: false,
  target: null,
  outputPath: null,
  startedAtOffsetMs: null,
};

export function useScreenRecording() {
  const [state, setState] = useState<ScreenRecordingState>(IDLE);
  const [targets, setTargets] = useState<CaptureTargets>({ displays: [], windows: [] });
  const [isLoadingTargets, setIsLoadingTargets] = useState(false);

  useEffect(() => {
    void invoke<ScreenRecordingState>('api_get_screen_recording_state')
      .then(setState)
      .catch((reason) => log.warn('[screen] Could not read the state:', reason));

    const subscription = listen<ScreenRecordingState>('screen-recording-changed', (event) =>
      setState(event.payload),
    );
    return () => { void subscription.then((unlisten) => unlisten()); };
  }, []);

  /// Enumerating is what triggers the macOS permission prompt, so it runs when
  /// the picker opens rather than at startup — asking before there is a reason
  /// is how people learn to refuse.
  const loadTargets = useCallback(async () => {
    setIsLoadingTargets(true);
    try {
      setTargets(await invoke<CaptureTargets>('api_list_capture_targets'));
      return true;
    } catch (reason) {
      log.warn('[screen] Could not list capture targets:', reason);
      toast.error(screenCaptureErrorMessage(reason));
      return false;
    } finally {
      setIsLoadingTargets(false);
    }
  }, []);

  const start = useCallback(
    async (target: CaptureTarget, startedAtOffsetMs: number) => {
      try {
        setState(
          await invoke<ScreenRecordingState>('api_start_screen_recording', {
            target,
            startedAtOffsetMs,
          }),
        );
        return true;
      } catch (reason) {
        log.warn('[screen] Could not start:', reason);
        toast.error(screenCaptureErrorMessage(reason));
        return false;
      }
    },
    [],
  );

  const stop = useCallback(async () => {
    try {
      setState(await invoke<ScreenRecordingState>('api_stop_screen_recording'));
      toast.success('Screen recording saved', {
        description: 'It attaches to the Session when you save the recording.',
      });
      return true;
    } catch (reason) {
      log.warn('[screen] Could not stop:', reason);
      toast.error(screenCaptureErrorMessage(reason));
      return false;
    }
  }, []);

  return { state, targets, isLoadingTargets, loadTargets, start, stop };
}
