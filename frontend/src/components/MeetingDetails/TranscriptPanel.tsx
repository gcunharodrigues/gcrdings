"use client";

import type { Transcript } from "@/types";
import type { ReviewRecordAction, ReviewRecordState } from "@/lib/review-record";
import type { SessionAudioPlayer } from "@/hooks/useAudioPlayer";
import { AudioPlayer } from "@/components/AudioPlayer";
import { useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { TranscriptButtonGroup } from "./TranscriptButtonGroup";

interface TranscriptPanelProps {
  state: ReviewRecordState;
  sourceTranscripts: Transcript[];
  dispatch: (action: ReviewRecordAction) => void;
  save: () => Promise<boolean>;
  reload: () => Promise<void>;
  audioPlayer: SessionAudioPlayer;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  loadedCount?: number;
  totalCount?: number;
  onLoadMore?: () => void;
  meetingId: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;
  confirmDestructiveOperation: () => Promise<boolean>;
  onOpenMeetingFolder: () => Promise<void>;
}

function formatTimestamp(seconds?: number): string {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < 0) return "--:--";
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.floor(seconds % 60);
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

function timestampDateTime(seconds?: number): string | undefined {
  return seconds !== undefined && Number.isFinite(seconds) && seconds >= 0
    ? `PT${seconds}S`
    : undefined;
}

export function TranscriptPanel({
  state,
  sourceTranscripts,
  dispatch,
  save,
  reload,
  audioPlayer,
  hasMore = false,
  isLoadingMore = false,
  loadedCount,
  totalCount,
  onLoadMore,
  meetingId,
  meetingFolderPath,
  onRefetchTranscripts,
  confirmDestructiveOperation,
  onOpenMeetingFolder,
}: TranscriptPanelProps) {
  const sourceById = new Map(sourceTranscripts.map((item) => [item.id, item]));
  const visibleIds = sourceTranscripts.length
    ? sourceTranscripts.map(({ id }) => id).filter((id) => state.present.transcripts[id])
    : state.present.transcriptOrder;
  const saving = state.saveStatus.type === "saving";
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: visibleIds.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 150,
    overscan: 5,
  });

  const playPassage = async (id: string, seconds?: number) => {
    const index = visibleIds.indexOf(id);
    if (index >= 0) virtualizer.scrollToIndex(index, { align: "center" });
    if (seconds === undefined) {
      await audioPlayer.seekAndPlay(Number.NaN);
      return;
    }
    if (await audioPlayer.seekAndPlay(seconds)) setSelectedId(id);
  };

  return (
    <section
      aria-labelledby="principal-transcript-heading"
      data-review-column="transcript"
      className="flex min-h-0 flex-col overflow-hidden border-x border-gray-200 bg-white"
    >
      <header className="flex flex-wrap items-center justify-between gap-2 border-b border-gray-200 p-4">
        <div>
          <h2 id="principal-transcript-heading" className="font-semibold text-gray-900">
            Principal transcript
          </h2>
          <p className="text-xs text-gray-500">
            {loadedCount ?? visibleIds.length} of {totalCount ?? state.present.transcriptOrder.length} passages
          </p>
        </div>
        <TranscriptButtonGroup
          transcriptCount={totalCount ?? visibleIds.length}
          onCopyTranscript={() => void navigator.clipboard.writeText(visibleIds.map((id) => state.present.transcripts[id].text).join("\n\n"))}
          onOpenMeetingFolder={onOpenMeetingFolder}
          meetingId={meetingId}
          meetingFolderPath={meetingFolderPath}
          onRefetchTranscripts={onRefetchTranscripts}
          confirmDestructiveOperation={confirmDestructiveOperation}
        />
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => dispatch({ type: "undo" })}
            disabled={state.past.length === 0 || saving}
            aria-label="Undo transcript edit"
            className="rounded border px-3 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
          >
            Undo
          </button>
          <button
            type="button"
            onClick={() => dispatch({ type: "redo" })}
            disabled={state.future.length === 0 || saving}
            aria-label="Redo transcript edit"
            className="rounded border px-3 py-1.5 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
          >
            Redo
          </button>
          <button
            type="button"
            onClick={() => void save()}
            disabled={!state.dirty || saving}
            aria-label="Save principal transcript"
            className="rounded bg-blue-600 px-3 py-1.5 text-sm font-medium text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 focus-visible:ring-offset-2 disabled:opacity-50"
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </header>

      {state.saveStatus.type === "failed" && (
        <div role="alert" className="flex flex-wrap items-center justify-between gap-2 border-b border-red-200 bg-red-50 px-4 py-2 text-sm text-red-800">
          <p>Save failed. Your edits remain available. {state.saveStatus.error}</p>
          <button
            type="button"
            onClick={() => {
              if (window.confirm("Reload the latest saved record and discard this draft?")) void reload();
            }}
            className="rounded border border-red-300 bg-white px-3 py-1.5 font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-700"
          >
            Reload latest
          </button>
        </div>
      )}

      <AudioPlayer player={audioPlayer} />

      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto p-4">
        {visibleIds.length === 0 ? (
          <p className="rounded border border-dashed p-6 text-center text-sm text-gray-600">
            No transcript passages are available yet.
          </p>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((row) => {
          const id = visibleIds[row.index];
          const transcript = state.present.transcripts[id];
          const source = sourceById.get(id);
          const assignedParticipant = state.present.participants[transcript.participantId];
          return (
            <article
              key={id}
              ref={virtualizer.measureElement}
              data-index={row.index}
              aria-current={selectedId === id ? "true" : undefined}
              className={`absolute left-0 top-0 w-full rounded-lg border p-3 focus-within:ring-2 focus-within:ring-blue-600 ${selectedId === id ? "border-blue-600 bg-blue-50" : "border-gray-200 bg-white"}`}
              style={{ transform: `translateY(${row.start}px)` }}
            >
              <div className="mb-2 flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => void playPassage(id, source?.audio_start_time)}
                  aria-label={`Play from ${formatTimestamp(source?.audio_start_time)}`}
                  className="min-w-12 rounded font-mono text-xs text-blue-700 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
                >
                  <time dateTime={timestampDateTime(source?.audio_start_time)}>{formatTimestamp(source?.audio_start_time)}</time>
                </button>
                <label className="sr-only" htmlFor={`participant-${id}`}>Participant for passage {id}</label>
                <select
                  id={`participant-${id}`}
                  value={transcript.participantId}
                  disabled={saving}
                  onChange={(event) => dispatch({
                    type: "transcript-assigned",
                    transcriptId: id,
                    participantId: event.target.value,
                  })}
                  className="min-w-0 flex-1 rounded border border-gray-300 px-2 py-1 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
                >
                  {!assignedParticipant && (
                    <option value={transcript.participantId}>Unknown participant · {transcript.participantId}</option>
                  )}
                  {Object.values(state.present.participants).map((participant) => (
                    <option key={participant.id} value={participant.id}>
                      {participant.displayName} · {participant.id}
                    </option>
                  ))}
                </select>
              </div>
              <label className="sr-only" htmlFor={`transcript-${id}`}>Exact transcript text for passage {id}</label>
              <textarea
                id={`transcript-${id}`}
                value={transcript.text}
                disabled={saving}
                onChange={(event) => dispatch({
                  type: "transcript-text-edited",
                  transcriptId: id,
                  text: event.target.value,
                })}
                rows={Math.max(2, Math.ceil(transcript.text.length / 80))}
                className="w-full resize-y rounded border border-gray-300 p-2 text-sm leading-6 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              />
              {transcript.text.length === 0 && (
                <p className="mt-1 text-xs text-amber-800">This saved passage will be empty.</p>
              )}
            </article>
          );
        })}
          </div>
        )}
        {hasMore && (
          <button
            type="button"
            onClick={onLoadMore}
            disabled={isLoadingMore}
            className="w-full rounded border border-gray-300 px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50"
          >
            {isLoadingMore ? "Loading…" : "Load more passages"}
          </button>
        )}
      </div>
    </section>
  );
}
