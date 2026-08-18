"use client";

import { GitCompare } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { diffReviewRecord, reviewChangeLabel, type ReviewChange } from "@/lib/review-diff";
import type { ReviewRecordState } from "@/lib/review-record";

function ChangeRow({ change, onSeek }: { change: ReviewChange; onSeek?: (id: string) => void }) {
  return (
    <li className="rounded-lg border border-border p-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {reviewChangeLabel(change.kind)}
        </span>
        {change.kind !== "participant-renamed" && onSeek && (
          <button
            type="button"
            onClick={() => onSeek(change.id)}
            className="font-mono text-xs text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
          >
            Go to passage
          </button>
        )}
      </div>
      <p className="mt-2 rounded bg-red-50 px-2 py-1 text-sm leading-6 text-red-900 line-through decoration-red-400">
        {change.before || <em className="not-italic opacity-70">(empty)</em>}
      </p>
      <p className="mt-1 rounded bg-emerald-50 px-2 py-1 text-sm leading-6 text-emerald-900">
        {change.after || <em className="not-italic opacity-70">(empty)</em>}
      </p>
    </li>
  );
}

export function ReviewChangesDialog({
  state,
  open,
  onOpenChange,
  onSeekPassage,
}: {
  state: ReviewRecordState;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSeekPassage?: (transcriptId: string) => void;
}) {
  const changes = diffReviewRecord(state.baseline, state.present);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[80vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <GitCompare className="size-4" aria-hidden="true" />
            Changes since the last save
          </DialogTitle>
        </DialogHeader>

        {changes.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            This draft matches the saved record. Nothing would change.
          </p>
        ) : (
          <>
            <p className="text-sm text-muted-foreground">
              {changes.length} {changes.length === 1 ? "change" : "changes"} will become revision{" "}
              {state.recordVersion + 1} of the principal transcript.
            </p>
            <ul className="mt-3 space-y-2">
              {changes.map((change, index) => (
                <ChangeRow key={`${change.kind}-${change.id}-${index}`} change={change} onSeek={onSeekPassage} />
              ))}
            </ul>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}

export function ReviewChangesTrigger({ state, onClick }: { state: ReviewRecordState; onClick: () => void }) {
  const count = diffReviewRecord(state.baseline, state.present).length;
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      onClick={onClick}
      disabled={count === 0}
      aria-label={`Review ${count} unsaved ${count === 1 ? "change" : "changes"}`}
    >
      <GitCompare className="size-4" aria-hidden="true" />
      Review changes{count > 0 ? ` (${count})` : ""}
    </Button>
  );
}
