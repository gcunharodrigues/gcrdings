'use client';

import { useEffect } from 'react';
import { AlertTriangle } from 'lucide-react';

export default function RouteError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    console.error('[RouteError]', error);
  }, [error]);

  return (
    <div role="alert" className="flex h-screen flex-col items-center justify-center gap-4 bg-muted/40 p-8 text-center">
      <AlertTriangle className="size-8 text-amber-600" aria-hidden="true" />
      <div>
        <h1 className="text-lg font-semibold text-foreground">This view could not be displayed</h1>
        <p className="mt-1 max-w-md text-sm text-muted-foreground">
          Your Sessions and recordings are stored on this Mac and were not changed. Retrying is safe.
        </p>
      </div>
      <pre className="max-w-lg overflow-x-auto rounded border border-border bg-card p-3 text-left text-xs text-muted-foreground">
        {error.message}
      </pre>
      <div className="flex gap-2">
        <button
          type="button"
          onClick={reset}
          className="rounded bg-blue-700 px-4 py-2 text-sm font-medium text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2"
        >
          Try again
        </button>
        <button
          type="button"
          onClick={() => { window.location.href = '/'; }}
          className="rounded border border-border px-4 py-2 text-sm font-medium text-foreground/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
        >
          Go home
        </button>
      </div>
    </div>
  );
}
