'use client';

import { Suspense, useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { LoaderIcon, SearchX } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';

/**
 * api_search_transcripts already searched every Session, but its results only
 * ever tinted rows in the sidebar — there was nowhere to read them, compare
 * them, or jump from one to another.
 */
function SearchResults() {
  const query = useSearchParams().get('q') ?? '';
  const { searchTranscripts, searchResults, isSearching, navigate, setCurrentMeeting } = useSidebar();
  const [hasSearched, setHasSearched] = useState(false);

  useEffect(() => {
    if (!query.trim()) return;
    void searchTranscripts(query).then(() => setHasSearched(true));
  }, [query, searchTranscripts]);

  if (!query.trim()) {
    return (
      <div className="mx-auto max-w-2xl p-8 text-center text-sm text-muted-foreground">
        Type in the sidebar search, or press ⌘K, to search across every Session.
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-3xl p-8">
      <h1 className="text-xl font-semibold text-foreground">
        Results for &ldquo;{query}&rdquo;
      </h1>
      <p aria-live="polite" className="mt-1 text-sm text-muted-foreground">
        {isSearching
          ? 'Searching…'
          : `${searchResults.length} ${searchResults.length === 1 ? 'Session' : 'Sessions'} matched`}
      </p>

      {isSearching && <LoaderIcon aria-label="Searching" className="mt-6 size-5 animate-spin" />}

      {!isSearching && hasSearched && searchResults.length === 0 && (
        <div className="mt-10 flex flex-col items-center gap-2 text-center">
          <SearchX className="size-7 text-muted-foreground/70" aria-hidden="true" />
          <p className="text-sm text-muted-foreground">Nothing in any transcript matched that.</p>
        </div>
      )}

      <ul className="mt-6 space-y-2">
        {searchResults.map((result) => (
          <li key={result.id}>
            <button
              type="button"
              onClick={() => {
                if (navigate(`/meeting-details?id=${result.id}`)) {
                  setCurrentMeeting({ id: result.id, title: result.title });
                }
              }}
              className="w-full rounded-lg border border-border bg-card p-4 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
            >
              <span className="block font-medium text-foreground">{result.title}</span>
              {result.matchContext && (
                <span className="mt-1 block text-sm leading-6 text-muted-foreground">
                  …{result.matchContext}…
                </span>
              )}
              {result.timestamp && (
                <span className="mt-1 block font-mono text-xs text-muted-foreground/70">{result.timestamp}</span>
              )}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default function SearchPage() {
  return (
    <Suspense fallback={<div className="p-8"><LoaderIcon aria-label="Loading search" className="size-5 animate-spin" /></div>}>
      <SearchResults />
    </Suspense>
  );
}
