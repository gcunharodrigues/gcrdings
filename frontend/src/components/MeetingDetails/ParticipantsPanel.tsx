"use client";

import type { ReviewRecordAction, ReviewRecordState } from "@/lib/review-record";

export function ParticipantsPanel({
  state,
  dispatch,
}: {
  state: ReviewRecordState;
  dispatch: (action: ReviewRecordAction) => void;
}) {
  const saving = state.saveStatus.type === "saving";
  return (
    <section aria-labelledby="participants-heading" className="shrink-0 border-t border-gray-200 p-4">
      <h2 id="participants-heading" className="mb-3 font-semibold text-gray-900">Participants</h2>
      <div className="space-y-3">
        {Object.values(state.present.participants).map((participant) => (
          <div key={participant.id}>
            <label htmlFor={`participant-name-${participant.id}`} className="mb-1 block text-xs font-medium text-gray-600">
              Display name
            </label>
            <input
              id={`participant-name-${participant.id}`}
              value={participant.displayName}
              onChange={(event) => dispatch({
                type: "participant-renamed",
                participantId: participant.id,
                displayName: event.target.value,
              })}
              aria-invalid={participant.displayName.trim() === ""}
              disabled={saving}
              className="w-full rounded border border-gray-300 px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
            />
            {participant.displayName.trim() === "" && (
              <p role="alert" className="mt-1 text-xs text-red-800">Participant name cannot be blank.</p>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}
