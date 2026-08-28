"use client";

import { useState } from "react";
import { Bookmark, Trash2 } from "lucide-react";
import { useSessionMarkers } from "@/hooks/useSessionMarkers";
import { formatMarkerOffset } from "@/types/markers";

/**
 * Markers are the cheapest navigation the app has: no model runs, nothing is
 * generated, and the person already told us where the interesting part was.
 */
export function MarkersPanel({
  meetingId,
  onSeek,
}: {
  meetingId: string;
  onSeek: (offsetMs: number) => void;
}) {
  const { markers, error, rename, remove } = useSessionMarkers(meetingId);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draftLabel, setDraftLabel] = useState("");

  if (error) {
    return <p role="alert" className="px-4 py-2 text-xs text-red-800">{error}</p>;
  }
  if (markers.length === 0) return null;

  const commit = (markerId: string) => {
    void rename(markerId, draftLabel.trim() || null);
    setEditingId(null);
  };

  return (
    <section aria-labelledby="markers-heading" className="shrink-0 border-t border-border p-4">
      <h2 id="markers-heading" className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        <Bookmark className="size-3" aria-hidden="true" />
        Markers ({markers.length})
      </h2>
      <ul className="space-y-1">
        {markers.map((marker) => (
          <li key={marker.id} className="group flex items-center gap-2">
            <button
              type="button"
              onClick={() => onSeek(marker.offsetMs)}
              className="shrink-0 font-mono text-xs tabular-nums text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
            >
              {formatMarkerOffset(marker.offsetMs)}
            </button>

            {editingId === marker.id ? (
              <input
                autoFocus
                value={draftLabel}
                aria-label="Marker name"
                onChange={(event) => setDraftLabel(event.target.value)}
                onBlur={() => commit(marker.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") commit(marker.id);
                  else if (event.key === "Escape") setEditingId(null);
                }}
                className="min-w-0 flex-1 rounded border border-blue-400 bg-card px-1.5 py-0.5 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            ) : (
              <button
                type="button"
                onClick={() => { setEditingId(marker.id); setDraftLabel(marker.label ?? ""); }}
                className="min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              >
                {marker.label ?? <span className="text-muted-foreground/70">Name this marker…</span>}
              </button>
            )}

            <button
              type="button"
              onClick={() => void remove(marker.id)}
              aria-label={`Delete marker at ${formatMarkerOffset(marker.offsetMs)}`}
              className="shrink-0 rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:text-red-600 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 group-hover:opacity-100"
            >
              <Trash2 className="size-3.5" aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
