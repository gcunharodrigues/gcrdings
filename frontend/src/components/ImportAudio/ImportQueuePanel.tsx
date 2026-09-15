'use client';

import { useState } from 'react';
import { ChevronDown, ChevronUp, Loader2, X } from 'lucide-react';
import { formatRemaining, useImportQueue } from '@/hooks/useImportQueue';

/**
 * A persistent, non-blocking view of what is importing.
 *
 * Importing used to be a modal that could not be dismissed, so a five-minute
 * extraction meant five minutes of not using the app. The work is the same; it
 * just no longer owns the window.
 */
export function ImportQueuePanel() {
  const { queue, stage, remainingMs, remove } = useImportQueue();
  const [collapsed, setCollapsed] = useState(false);

  const total = queue.completed + (queue.active ? 1 : 0) + queue.pending.length;
  if (!queue.active && queue.pending.length === 0) return null;

  return (
    <aside
      aria-label="Import queue"
      className="fixed bottom-4 right-4 z-50 w-80 overflow-hidden rounded-xl border border-border bg-card shadow-lg"
    >
      <button
        type="button"
        onClick={() => setCollapsed((current) => !current)}
        aria-expanded={!collapsed}
        className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-600"
      >
        <Loader2 className="size-4 shrink-0 animate-spin text-blue-600" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate text-sm font-medium">
          Importing {queue.completed + 1} of {total}
        </span>
        {collapsed ? (
          <ChevronUp className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        ) : (
          <ChevronDown className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        )}
      </button>

      {!collapsed && (
        <div className="border-t border-border px-3 pb-3 pt-2">
          {queue.active && (
            <>
              <p className="truncate text-sm text-foreground" title={queue.active.title}>
                {queue.active.title}
              </p>
              <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-secondary">
                <div
                  className="h-full rounded-full bg-blue-600 transition-[width] duration-500"
                  style={{ width: `${Math.min(100, stage?.progress ?? 0)}%` }}
                />
              </div>
              <p aria-live="polite" className="mt-1 flex justify-between gap-2 text-xs text-muted-foreground">
                <span className="truncate">{stage?.message ?? 'Preparing…'}</span>
                <span className="shrink-0 tabular-nums">
                  {remainingMs !== null ? formatRemaining(remainingMs) : ''}
                </span>
              </p>
            </>
          )}

          {queue.pending.length > 0 && (
            <ul className="mt-3 space-y-1 border-t border-border pt-2">
              {queue.pending.map((item) => (
                <li key={item.id} className="group flex items-center gap-2 text-xs">
                  <span className="min-w-0 flex-1 truncate text-muted-foreground">{item.title}</span>
                  <button
                    type="button"
                    onClick={() => void remove(item.id)}
                    aria-label={`Remove ${item.title} from the queue`}
                    className="shrink-0 rounded p-0.5 text-muted-foreground opacity-0 hover:text-red-600 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 group-hover:opacity-100"
                  >
                    <X className="size-3" aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
          )}

          {queue.failed.length > 0 && (
            <p className="mt-2 border-t border-border pt-2 text-xs text-amber-800">
              Could not import: {queue.failed.join(', ')}
            </p>
          )}
        </div>
      )}
    </aside>
  );
}
