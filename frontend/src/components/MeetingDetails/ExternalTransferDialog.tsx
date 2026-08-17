'use client';

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { useExternalProvider } from '@/hooks/meeting-details/useExternalProvider';
import { providerTransferCanRetry } from '@/types/verifiable-record';

const DATA_TYPE_LABELS: Record<string, string> = {
  session_metadata: 'Session metadata',
  participants: 'Participants',
  corrected_transcript: 'Complete corrected transcript',
  local_generation: 'Current local findings status and result',
};

export function ExternalTransferDialog({ meetingId, disabled = false }: { meetingId: string; disabled?: boolean }) {
  const transfer = useExternalProvider(meetingId);

  return (
    <>
      <button type="button" onClick={() => void transfer.loadPreview()} disabled={disabled || transfer.busy} className="rounded border border-gray-300 px-3 py-1.5 text-sm font-medium text-gray-800 hover:bg-gray-50 disabled:opacity-50">
        Send to provider
      </button>
      {transfer.open && (
      <Dialog open onOpenChange={(next) => { if (!next) transfer.close(); }}>
        <DialogContent aria-describedby="external-transfer-description">
          <DialogHeader>
            <DialogTitle>Confirm external transfer</DialogTitle>
            <DialogDescription id="external-transfer-description">
              Review the exact destination and data types. Confirmation is valid once and never retries automatically.
            </DialogDescription>
          </DialogHeader>

          {transfer.busy && !transfer.preview && <p aria-live="polite">Building a current Session preview…</p>}
          {transfer.preview && !transfer.result && (
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
              <dt className="font-medium">Provider</dt><dd>{transfer.preview.providerDisplayName}</dd>
              <dt className="font-medium">Session</dt><dd>{transfer.preview.sessionTitle}</dd>
              <dt className="font-medium">Purpose</dt><dd>{transfer.preview.purpose}</dd>
              <dt className="font-medium">Task</dt><dd>Agent Handoff transfer</dd>
              <dt className="font-medium">Transcript revision</dt><dd>{transfer.preview.principalTranscriptRevision}</dd>
              <dt className="font-medium">Data sent</dt>
              <dd><ul className="list-disc pl-5">{transfer.preview.dataTypes.map((type) => <li key={type}>{DATA_TYPE_LABELS[type] ?? type}</li>)}</ul></dd>
            </dl>
          )}
          {transfer.result && (
            <div role="status" className="rounded border border-green-200 bg-green-50 p-3 text-sm text-green-900">
              Transfer completed with {transfer.result.provider} for task {transfer.result.task}.
            </div>
          )}
          {transfer.error && <p role="alert" className="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-900">{transfer.error}</p>}

          <DialogFooter>
            <button type="button" onClick={() => transfer.close()} disabled={transfer.busy} className="rounded border px-3 py-2 text-sm">Cancel</button>
            {transfer.error && providerTransferCanRetry(transfer.errorCode) && <button type="button" onClick={() => void transfer.loadPreview()} disabled={transfer.busy} className="rounded border px-3 py-2 text-sm">Review fresh preview</button>}
            {transfer.preview && !transfer.result && !transfer.error && (
              <button type="button" onClick={() => void transfer.confirmTransfer()} disabled={transfer.busy} className="rounded bg-blue-700 px-3 py-2 text-sm text-white disabled:opacity-50">Confirm and send once</button>
            )}
            {transfer.result && <button type="button" onClick={() => transfer.close()} className="rounded bg-blue-700 px-3 py-2 text-sm text-white">Done</button>}
          </DialogFooter>
        </DialogContent>
      </Dialog>
      )}
    </>
  );
}
