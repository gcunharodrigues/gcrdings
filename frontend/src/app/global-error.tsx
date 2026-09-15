'use client';

import { useEffect } from 'react';

export default function GlobalError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  useEffect(() => {
    console.error('[GlobalError]', error);
  }, [error]);

  return (
    <html lang="en">
      <body>
        <div role="alert" style={{ display: 'flex', height: '100vh', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '1rem', fontFamily: 'system-ui, sans-serif', textAlign: 'center', padding: '2rem' }}>
          <h1 style={{ fontSize: '1.125rem', fontWeight: 600 }}>gcrdings could not start this view</h1>
          <p style={{ fontSize: '0.875rem', color: '#4b5563', maxWidth: '28rem' }}>
            Your Sessions and recordings are stored on this Mac and were not changed.
          </p>
          <button type="button" onClick={reset} style={{ background: '#1d4ed8', color: 'white', border: 0, borderRadius: '0.25rem', padding: '0.5rem 1rem', fontSize: '0.875rem', cursor: 'pointer' }}>
            Try again
          </button>
        </div>
      </body>
    </html>
  );
}
