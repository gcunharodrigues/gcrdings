'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppWindowMac, Loader2, Monitor } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { log } from '@/lib/logger';
import { describeTarget, type CaptureTarget, type CaptureTargets } from '@/types/screenCapture';

type Tab = 'screens' | 'windows';

function targetKey(target: CaptureTarget): string {
  return `${target.kind}-${target.id}`;
}

/**
 * Picking what to record by looking at it.
 *
 * A list of window titles asks someone to remember which "Untitled" is which,
 * and the cost of guessing wrong here is recording something they did not mean
 * to share. Every target shows a still of exactly what would be captured.
 */
function TargetTile({
  target,
  selected,
  onSelect,
}: {
  target: CaptureTarget;
  selected: boolean;
  onSelect: () => void;
}) {
  const [preview, setPreview] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    // One at a time, so the grid appears immediately and fills in. Capturing
    // twenty stills before showing anything would stall the dialog for seconds.
    void invoke<string | null>('api_capture_target_thumbnail', { target, width: 400 })
      .then((image) => {
        if (cancelled) return;
        if (image) setPreview(image);
        else setFailed(true);
      })
      .catch((reason) => {
        log.warn('[screen] Preview failed:', reason);
        if (!cancelled) setFailed(true);
      });
    return () => { cancelled = true; };
  }, [target]);

  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={`group flex flex-col gap-1.5 rounded-lg border-2 p-2 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 ${
        selected ? 'border-blue-600 bg-blue-50/50' : 'border-transparent hover:bg-accent'
      }`}
    >
      <div className="flex aspect-video items-center justify-center overflow-hidden rounded border border-border bg-muted">
        {preview ? (
          /* A base64 data URI needs no optimisation pipeline, and a desktop
             app has no image server for next/image to reach. */
          // eslint-disable-next-line @next/next/no-img-element
          <img src={preview} alt="" className="size-full object-contain" />
        ) : failed ? (
          <span className="text-[10px] text-muted-foreground">No preview</span>
        ) : (
          <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden="true" />
        )}
      </div>
      <span className="flex items-center gap-1.5 truncate text-xs" title={describeTarget(target)}>
        {target.kind === 'display' ? (
          <Monitor className="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
        ) : (
          <AppWindowMac className="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
        )}
        <span className="truncate">
          {target.kind === 'display' ? `Screen ${target.width}×${target.height}` : target.title}
        </span>
      </span>
    </button>
  );
}

export function ScreenTargetPicker({
  open,
  onOpenChange,
  targets,
  isLoading,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  targets: CaptureTargets;
  isLoading: boolean;
  onConfirm: (target: CaptureTarget) => void;
}) {
  const [tab, setTab] = useState<Tab>('screens');
  const [selected, setSelected] = useState<CaptureTarget | null>(null);

  // A stale selection from a previous open would let someone confirm a window
  // that has since closed.
  useEffect(() => { if (!open) setSelected(null); }, [open]);

  const shown = tab === 'screens' ? targets.displays : targets.windows;
  const confirm = useCallback(() => {
    if (selected) onConfirm(selected);
  }, [onConfirm, selected]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Choose what to record</DialogTitle>
        </DialogHeader>

        <p className="text-sm text-muted-foreground">
          Only what you pick is recorded, and it stays on this Mac.
        </p>

        <div role="tablist" className="flex gap-1 border-b border-border">
          {(['screens', 'windows'] as Tab[]).map((value) => (
            <button
              key={value}
              type="button"
              role="tab"
              aria-selected={tab === value}
              onClick={() => setTab(value)}
              className={`-mb-px border-b-2 px-3 py-1.5 text-sm capitalize transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 ${
                tab === value
                  ? 'border-blue-600 font-medium text-foreground'
                  : 'border-transparent text-muted-foreground hover:text-foreground'
              }`}
            >
              {value === 'screens' ? 'Entire screen' : 'Window'}
              <span className="ml-1.5 tabular-nums text-xs text-muted-foreground">
                {value === 'screens' ? targets.displays.length : targets.windows.length}
              </span>
            </button>
          ))}
        </div>

        <div className="max-h-[46vh] min-h-40 overflow-y-auto">
          {isLoading ? (
            <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              Looking for screens and windows…
            </p>
          ) : shown.length === 0 ? (
            <p className="p-6 text-center text-sm text-muted-foreground">
              {tab === 'windows' ? 'No shareable windows are open.' : 'No screens found.'}
            </p>
          ) : (
            <div className="grid grid-cols-2 gap-2 p-1 sm:grid-cols-3">
              {shown.map((target) => (
                <TargetTile
                  key={targetKey(target)}
                  target={target}
                  selected={selected !== null && targetKey(selected) === targetKey(target)}
                  onSelect={() => setSelected(target)}
                />
              ))}
            </div>
          )}
        </div>

        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="button" onClick={confirm} disabled={!selected}>
            Record
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
