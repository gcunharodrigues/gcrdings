import { generatedFindings, generationMessage, type RecordType, type VerifiableRecord } from "@/types/verifiable-record";

const LABELS: Record<RecordType, string> = { meeting: "Meeting", interview: "Interview", content: "Content" };

export function SummaryPanel({ record, loadError, hasUnsavedTranscript, onSelectType, onGenerate, onCancel, onSeek }: {
  record: VerifiableRecord | null;
  loadError: string | null;
  hasUnsavedTranscript: boolean;
  onSelectType: (type: RecordType) => void;
  onGenerate: () => void;
  onCancel: () => void;
  onSeek: (timestampMs: number) => void;
}) {
  const processing = record?.generation_status === "processing";
  return (
    <section aria-labelledby="findings-heading" className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
      <div className="flex flex-wrap items-end gap-3 border-b border-gray-200 pb-4">
        <div className="min-w-40 flex-1">
          <label htmlFor="record-type" className="mb-1 block text-xs font-semibold uppercase tracking-wide text-gray-600">Record type</label>
          <select id="record-type" value={record?.record_type ?? "meeting"} disabled={!record || processing} onChange={(event) => onSelectType(event.target.value as RecordType)} className="w-full rounded border border-gray-300 px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600">
            {Object.entries(LABELS).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
        </div>
        {processing ? (
          <button type="button" onClick={onCancel} className="rounded border border-red-700 px-3 py-2 text-sm font-medium text-red-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-700">Cancel</button>
        ) : (
          <button type="button" onClick={onGenerate} disabled={!record || hasUnsavedTranscript || record.transcript.length === 0} className="rounded bg-blue-700 px-3 py-2 text-sm font-medium text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2 disabled:opacity-50">Generate local findings</button>
        )}
      </div>

      <h1 id="findings-heading" className="mt-4 text-lg font-semibold text-gray-900">Grounded findings</h1>
      {hasUnsavedTranscript && <p className="mt-2 rounded bg-amber-50 p-3 text-sm text-amber-900">Save transcript corrections before generating so evidence uses the principal record.</p>}
      <p aria-live="polite" className="mt-2 text-sm text-gray-600">{loadError ?? (record ? generationMessage(record.generation_status, record.error_code ?? undefined) : "Loading local findings…")}</p>

      {record?.generated && record.generation_status === "completed" && (
        <div className="mt-5 space-y-5">
          <p className="leading-7 text-gray-800">{record.generated.summary}</p>
          <ul className="space-y-4">
            {generatedFindings(record.generated).map((finding, index) => (
              <li key={`${finding.label}-${index}`} className="rounded-lg border border-gray-200 p-4">
                <h2 className="font-semibold text-gray-900">{finding.label}</h2>
                <p className="mt-2 text-sm leading-6 text-gray-700">{finding.detail}</p>
                <div className="mt-3 flex flex-wrap gap-2">
                  {finding.evidence.map((evidence) => <button key={`${evidence.block_id}-${evidence.timestamp_ms}`} type="button" onClick={() => onSeek(evidence.timestamp_ms)} className="rounded bg-blue-50 px-2 py-1 text-xs font-medium text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700">Play evidence at {(evidence.timestamp_ms / 1000).toFixed(1)}s</button>)}
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
