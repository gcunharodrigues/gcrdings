"use client";

import { useState } from "react";
import { FolderOpen, Loader2, Scissors, Trash2, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useClips, type Clip } from "@/hooks/useClips";
import { useSessionMarkers } from "@/hooks/useSessionMarkers";
import { formatMarkerOffset } from "@/types/markers";

/**
 * Clips are cut from the Session's original media, so a clip of an imported
 * video stays video and a clip of a recording is audio — recordings here never
 * carry video in the first place.
 *
 * Two markers delimit the range. The pair is chosen from markers the person
 * already dropped, rather than asking them to type timestamps for a moment they
 * flagged precisely because they did not want to write anything down.
 */
function ClipRow({
  clip,
  onRename,
  onRemove,
  onReveal,
}: {
  clip: Clip;
  onRename: (id: string, title: string | null) => void;
  onRemove: (id: string) => void;
  onReveal: (id: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(clip.title ?? "");

  const commit = () => {
    onRename(clip.id, draft.trim() || null);
    setEditing(false);
  };

  return (
    <li className="group rounded-lg border border-border p-2.5">
      <div className="flex items-center gap-2">
        <span className="shrink-0 font-mono text-xs tabular-nums text-muted-foreground">
          {formatMarkerOffset(clip.startMs)}–{formatMarkerOffset(clip.endMs)}
        </span>

        {editing ? (
          <input
            autoFocus
            value={draft}
            aria-label="Clip name"
            onChange={(event) => setDraft(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") commit();
              else if (event.key === "Escape") setEditing(false);
            }}
            className="min-w-0 flex-1 rounded border border-blue-400 bg-card px-1.5 py-0.5 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        ) : (
          <button
            type="button"
            onClick={() => { setDraft(clip.title ?? ""); setEditing(true); }}
            className="min-w-0 flex-1 truncate rounded px-1 py-0.5 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
          >
            {clip.title ?? <span className="text-muted-foreground/70">Name this clip…</span>}
          </button>
        )}

        {clip.status === "ready" && (
          <button
            type="button"
            onClick={() => onReveal(clip.id)}
            aria-label="Show the clip in Finder"
            className="shrink-0 rounded p-1 text-muted-foreground opacity-0 hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 group-hover:opacity-100"
          >
            <FolderOpen className="size-3.5" aria-hidden="true" />
          </button>
        )}
        <button
          type="button"
          onClick={() => onRemove(clip.id)}
          aria-label="Delete this clip"
          className="shrink-0 rounded p-1 text-muted-foreground opacity-0 hover:text-red-600 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 group-hover:opacity-100"
        >
          <Trash2 className="size-3.5" aria-hidden="true" />
        </button>
      </div>

      {(clip.status === "pending" || clip.status === "rendering") && (
        <p aria-live="polite" className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
          <Loader2 className="size-3 animate-spin" aria-hidden="true" />
          Cutting…
        </p>
      )}
      {clip.status === "failed" && (
        <p role="alert" className="mt-1 flex items-center gap-1.5 text-xs text-amber-800">
          <TriangleAlert className="size-3" aria-hidden="true" />
          This clip could not be cut. The Session itself is untouched.
        </p>
      )}
    </li>
  );
}

export function ClipsPanel({ meetingId }: { meetingId: string }) {
  const { clips, create, rename, remove, reveal } = useClips(meetingId);
  const { markers } = useSessionMarkers(meetingId);
  const [startId, setStartId] = useState<string>("");
  const [endId, setEndId] = useState<string>("");

  const start = markers.find((marker) => marker.id === startId);
  const end = markers.find((marker) => marker.id === endId);
  const canCut = Boolean(start && end && start.offsetMs !== end.offsetMs);

  const cut = async () => {
    if (!start || !end) return;
    if (await create(start.offsetMs, end.offsetMs)) {
      setStartId("");
      setEndId("");
    }
  };

  return (
    <div className="flex min-h-0 flex-col gap-3 p-4">
      {markers.length < 2 ? (
        <p className="rounded border border-dashed p-4 text-center text-sm text-muted-foreground">
          Drop at least two markers with ⌘M while recording, or add them from the Markers list,
          and a clip is the span between any two of them.
        </p>
      ) : (
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex min-w-28 flex-1 flex-col gap-1">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">From</span>
            <select
              value={startId}
              onChange={(event) => setStartId(event.target.value)}
              className="rounded border border-border bg-card px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
            >
              <option value="">Pick a marker</option>
              {markers.map((marker) => (
                <option key={marker.id} value={marker.id}>
                  {formatMarkerOffset(marker.offsetMs)} {marker.label ? `· ${marker.label}` : ""}
                </option>
              ))}
            </select>
          </label>

          <label className="flex min-w-28 flex-1 flex-col gap-1">
            <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">To</span>
            <select
              value={endId}
              onChange={(event) => setEndId(event.target.value)}
              className="rounded border border-border bg-card px-2 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
            >
              <option value="">Pick a marker</option>
              {markers.map((marker) => (
                <option key={marker.id} value={marker.id}>
                  {formatMarkerOffset(marker.offsetMs)} {marker.label ? `· ${marker.label}` : ""}
                </option>
              ))}
            </select>
          </label>

          <Button type="button" size="sm" onClick={() => void cut()} disabled={!canCut}>
            <Scissors className="size-4" aria-hidden="true" />
            Cut
          </Button>
        </div>
      )}

      {clips.length === 0 ? (
        <p className="text-xs text-muted-foreground">No clips yet.</p>
      ) : (
        <ul className="space-y-1.5 overflow-y-auto">
          {clips.map((clip) => (
            <ClipRow key={clip.id} clip={clip} onRename={rename} onRemove={remove} onReveal={reveal} />
          ))}
        </ul>
      )}
    </div>
  );
}
