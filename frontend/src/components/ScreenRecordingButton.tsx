'use client';

import { useState } from 'react';
import { appDataDir, join } from '@tauri-apps/api/path';
import { AppWindowMac, Monitor, MonitorPlay, MonitorStop } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { useScreenRecording } from '@/hooks/useScreenRecording';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { describeTarget, type CaptureTarget } from '@/types/screenCapture';

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
    const stamp = new Date().toISOString().replace(/[:.]/g, '-');
    const destination = await join(await appDataDir(), 'screen-recordings', `${stamp}.mp4`);
    // The screen and the audio start moments apart, so the offset between them
    // is recorded rather than assumed to be zero.
    const offsetMs = Math.max(0, Math.round((activeDuration ?? 0) * 1000));
    if (await start(target, destination, offsetMs)) setOpen(false);
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
    <Popover open={open} onOpenChange={(next) => void openPicker(next)}>
      <PopoverTrigger asChild>
        <Button type="button" variant="outline" size="sm" aria-label="Record the screen">
          <MonitorPlay className="size-4" aria-hidden="true" />
          Record screen
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-2">
        {!isRecordingAudio && (
          <p className="mb-2 rounded bg-amber-50 p-2 text-xs text-amber-900">
            No audio recording is running. The screen will be captured on its own.
          </p>
        )}

        {isLoadingTargets ? (
          <p className="p-2 text-sm text-muted-foreground">Looking for screens and windows…</p>
        ) : (
          <div className="max-h-72 space-y-3 overflow-y-auto">
            <section>
              <p className="mb-1 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                <Monitor className="size-3" aria-hidden="true" /> Screens
              </p>
              {targets.displays.map((target) => (
                <button
                  key={`display-${target.id}`}
                  type="button"
                  onClick={() => void begin(target)}
                  className="w-full truncate rounded px-2 py-1.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
                >
                  {describeTarget(target)}
                </button>
              ))}
            </section>

            <section>
              <p className="mb-1 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                <AppWindowMac className="size-3" aria-hidden="true" /> Windows
              </p>
              {targets.windows.length === 0 ? (
                <p className="px-2 py-1 text-xs text-muted-foreground">No shareable windows.</p>
              ) : (
                targets.windows.map((target) => (
                  <button
                    key={`window-${target.id}`}
                    type="button"
                    onClick={() => void begin(target)}
                    className="w-full truncate rounded px-2 py-1.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
                    title={describeTarget(target)}
                  >
                    {describeTarget(target)}
                  </button>
                ))
              )}
            </section>
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
