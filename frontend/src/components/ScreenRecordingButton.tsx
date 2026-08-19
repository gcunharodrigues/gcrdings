'use client';

import { useState } from 'react';
import { MonitorPlay, MonitorStop } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { ScreenTargetPicker } from '@/components/ScreenTargetPicker';
import { useScreenRecording } from '@/hooks/useScreenRecording';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import type { CaptureTarget } from '@/types/screenCapture';

/**
 * Recording the screen alongside the audio, without a bot joining the call.
 *
 * A target is chosen before recording starts — one screen or one window, never
 * "everything". Capturing more than was asked for is the failure that matters
 * with screen capture, so there is no option that does it.
 */
export function ScreenRecordingButton() {
  const { state, targets, isLoadingTargets, loadTargets, start, stop } = useScreenRecording();
  const { isRecording: isRecordingAudio, activeDuration } = useRecordingState();
  const [open, setOpen] = useState(false);

  const openPicker = async (next: boolean) => {
    setOpen(next);
    if (next) await loadTargets();
  };

  const begin = async (target: CaptureTarget) => {
    // The backend picks the path. Choosing it here put recordings under the
    // bundle identifier while the database lived under the product name, so
    // the app wrote videos it could never find again.
    const offsetMs = Math.max(0, Math.round((activeDuration ?? 0) * 1000));
    if (await start(target, offsetMs)) setOpen(false);
  };

  if (state.isRecording) {
    return (
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => void stop()}
        aria-label="Stop recording the screen"
      >
        <MonitorStop className="size-4 text-record" aria-hidden="true" />
        Stop screen
      </Button>
    );
  }

  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => void openPicker(true)}
        aria-label="Record the screen"
      >
        <MonitorPlay className="size-4" aria-hidden="true" />
        Record screen
      </Button>

      <ScreenTargetPicker
        open={open}
        onOpenChange={setOpen}
        targets={targets}
        isLoading={isLoadingTargets}
        onConfirm={(target) => void begin(target)}
      />
    </>
  );
}
