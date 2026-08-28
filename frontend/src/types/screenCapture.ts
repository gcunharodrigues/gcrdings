/** What a screen recording points at. Never "everything". */
export type CaptureTarget =
  | { kind: 'display'; id: number; width: number; height: number }
  | { kind: 'window'; id: number; title: string; app: string };

export interface CaptureTargets {
  displays: CaptureTarget[];
  windows: CaptureTarget[];
}

export interface ScreenRecordingState {
  isRecording: boolean;
  target: CaptureTarget | null;
  outputPath: string | null;
  /** Milliseconds into the audio recording at which the screen started. */
  startedAtOffsetMs: number | null;
}

export function describeTarget(target: CaptureTarget): string {
  return target.kind === 'display'
    ? `Screen · ${target.width}×${target.height}`
    : `${target.app || 'Window'} · ${target.title}`;
}

const MESSAGES: Record<string, string> = {
  permission_denied:
    'Screen Recording is off. Enable gcrdings in System Settings → Privacy & Security → Screen Recording.',
  target_unavailable: 'That window or screen is no longer available.',
  already_recording: 'A screen recording is already running.',
  not_recording: 'No screen recording is running.',
  unsupported: 'This macOS version cannot record the screen to a file.',
  failed: 'The screen recording failed.',
};

export function screenCaptureErrorMessage(error: unknown): string {
  const code = (error as { code?: string })?.code;
  return (code && MESSAGES[code]) || MESSAGES.failed;
}
