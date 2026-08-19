import type { ReviewRecordSnapshot } from "@/lib/review-record";

/**
 * What changed between the last saved record and the working draft.
 *
 * The product promises revision history, but the only comparison the frontend
 * can make honestly today is baseline versus present: earlier revisions are not
 * kept anywhere the app can read. Reviewing a draft before saving is the part
 * that is answerable now, and it is also the part that prevents a bad
 * correction from becoming the principal record.
 */
export type ReviewChangeKind = "participant-renamed" | "text-edited" | "speaker-reassigned";

export interface ReviewChange {
  kind: ReviewChangeKind;
  /** Transcript or participant id, for seeking and for stable keys. */
  id: string;
  label: string;
  before: string;
  after: string;
}

const KIND_LABELS: Record<ReviewChangeKind, string> = {
  "participant-renamed": "Participant renamed",
  "text-edited": "Passage edited",
  "speaker-reassigned": "Speaker reassigned",
};

export function reviewChangeLabel(kind: ReviewChangeKind): string {
  return KIND_LABELS[kind];
}

function participantName(snapshot: ReviewRecordSnapshot, participantId: string): string {
  return snapshot.participants[participantId]?.displayName || "Unknown participant";
}

export function diffReviewRecord(
  baseline: ReviewRecordSnapshot,
  present: ReviewRecordSnapshot,
): ReviewChange[] {
  const changes: ReviewChange[] = [];

  for (const [id, participant] of Object.entries(present.participants)) {
    const before = baseline.participants[id]?.displayName;
    if (before !== undefined && before !== participant.displayName) {
      changes.push({
        kind: "participant-renamed",
        id,
        label: before,
        before,
        after: participant.displayName,
      });
    }
  }

  // Walk transcriptOrder rather than the record, so changes read in the order
  // they appear in the transcript instead of whatever order the keys landed in.
  for (const id of present.transcriptOrder) {
    const now = present.transcripts[id];
    const then = baseline.transcripts[id];
    if (!now || !then) continue;

    if (then.text !== now.text) {
      changes.push({ kind: "text-edited", id, label: id, before: then.text, after: now.text });
    }

    if (then.participantId !== now.participantId) {
      changes.push({
        kind: "speaker-reassigned",
        id,
        label: id,
        before: participantName(baseline, then.participantId),
        after: participantName(present, now.participantId),
      });
    }
  }

  return changes;
}
