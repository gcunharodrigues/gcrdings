'use client';

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AlertTriangle, FolderOpen, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { log } from '@/lib/logger';

interface BatchCandidate {
  path: string;
  title: string;
  sizeBytes: number;
  /** Why this file cannot be imported, if it cannot. */
  rejection: string | null;
}

interface BatchScan {
  folder: string;
  candidates: BatchCandidate[];
}

interface BatchProgress {
  completed: number;
  total: number;
  currentTitle: string;
  failed: string[];
}

function formatSize(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${Math.round(bytes / 1_000_000)} MB`;
  return `${Math.max(1, Math.round(bytes / 1000))} KB`;
}

/**
 * Importing a folder rather than a file at a time.
 *
 * Every file is validated during the scan, before anything is queued, so the
 * person sees what will actually import and what will not — finding out
 * halfway through a twenty-file batch is worse than not starting.
 */
export function BatchImportDialog({
  open,
  onOpenChange,
  onFinished,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onFinished?: () => void;
}) {
  const [scan, setScan] = useState<BatchScan | null>(null);
  const [isScanning, setIsScanning] = useState(false);
  const [progress, setProgress] = useState<BatchProgress | null>(null);

  useEffect(() => {
    if (!open) return;
    const unlisten: Array<Promise<() => void>> = [
      listen<BatchProgress>('batch-import-progress', (event) => setProgress(event.payload)),
      listen<BatchProgress>('batch-import-complete', (event) => {
        setProgress(null);
        const { total, failed } = event.payload;
        const imported = total - failed.length;
        if (failed.length === 0) {
          toast.success(`${imported} Sessions imported`);
        } else {
          toast.warning(`${imported} of ${total} imported`, {
            description: `Could not import: ${failed.join(', ')}`,
          });
        }
        onFinished?.();
        onOpenChange(false);
      }),
    ];
    return () => { void Promise.all(unlisten).then((fns) => fns.forEach((fn) => fn())); };
  }, [onFinished, onOpenChange, open]);

  const pickFolder = useCallback(async () => {
    setIsScanning(true);
    try {
      const result = await invoke<BatchScan | null>('api_select_import_folder');
      if (result) setScan(result);
    } catch (reason) {
      log.warn('[batch-import] Folder scan failed:', reason);
      toast.error('That folder could not be read.');
    } finally {
      setIsScanning(false);
    }
  }, []);

  const start = useCallback(async () => {
    if (!scan) return;
    try {
      const total = await invoke<number>('api_start_batch_import', { files: scan.candidates });
      setProgress({ completed: 0, total, currentTitle: '', failed: [] });
    } catch (reason) {
      log.warn('[batch-import] Batch could not start:', reason);
      toast.error('The import could not be started.', {
        description: 'Another import may already be running.',
      });
    }
  }, [scan]);

  const importable = scan?.candidates.filter((c) => c.rejection === null) ?? [];
  const rejected = scan?.candidates.filter((c) => c.rejection !== null) ?? [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Import a folder</DialogTitle>
        </DialogHeader>

        {progress ? (
          <div className="space-y-3 py-2">
            <p className="flex items-center gap-2 text-sm text-foreground">
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
              Importing {progress.currentTitle || '…'}
            </p>
            <div className="h-1.5 overflow-hidden rounded-full bg-secondary">
              <div
                className="h-full rounded-full bg-blue-600 transition-[width]"
                style={{ width: `${(progress.completed / Math.max(1, progress.total)) * 100}%` }}
              />
            </div>
            <p aria-live="polite" className="text-xs tabular-nums text-muted-foreground">
              {progress.completed} of {progress.total}
              {progress.failed.length > 0 && ` · ${progress.failed.length} failed`}
            </p>
            <p className="text-xs text-muted-foreground">
              Files import one after another, and a failure does not stop the rest.
            </p>
          </div>
        ) : !scan ? (
          <div className="py-4 text-center">
            <p className="mb-4 text-sm text-muted-foreground">
              Every audio and video file directly inside the folder is checked before anything is imported.
              Sub-folders are left alone.
            </p>
            <Button type="button" onClick={() => void pickFolder()} disabled={isScanning}>
              <FolderOpen className="size-4" aria-hidden="true" />
              {isScanning ? 'Scanning…' : 'Choose a folder'}
            </Button>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="truncate font-mono text-xs text-muted-foreground" title={scan.folder}>
              {scan.folder}
            </p>

            {importable.length === 0 ? (
              <p className="rounded border border-dashed p-4 text-center text-sm text-muted-foreground">
                Nothing in that folder can be imported.
              </p>
            ) : (
              <ul className="max-h-56 space-y-1 overflow-y-auto rounded border border-border p-2">
                {importable.map((candidate) => (
                  <li key={candidate.path} className="flex items-center gap-2 text-sm">
                    <span className="min-w-0 flex-1 truncate">{candidate.title}</span>
                    <span className="shrink-0 tabular-nums text-xs text-muted-foreground">
                      {formatSize(candidate.sizeBytes)}
                    </span>
                  </li>
                ))}
              </ul>
            )}

            {rejected.length > 0 && (
              <details className="rounded border border-amber-200 bg-amber-50 p-2 text-xs text-amber-900">
                <summary className="flex cursor-pointer items-center gap-1.5 font-medium">
                  <AlertTriangle className="size-3.5" aria-hidden="true" />
                  {rejected.length} {rejected.length === 1 ? 'file' : 'files'} will be skipped
                </summary>
                <ul className="mt-2 space-y-1">
                  {rejected.map((candidate) => (
                    <li key={candidate.path}>
                      <span className="font-medium">{candidate.title}</span> — {candidate.rejection}
                    </li>
                  ))}
                </ul>
              </details>
            )}

            <div className="flex justify-end gap-2 pt-1">
              <Button type="button" variant="outline" onClick={() => setScan(null)}>
                Choose another
              </Button>
              <Button type="button" onClick={() => void start()} disabled={importable.length === 0}>
                Import {importable.length}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
