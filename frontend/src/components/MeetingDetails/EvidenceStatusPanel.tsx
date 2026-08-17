import { generatedFindings, type VerifiableRecord } from "@/types/verifiable-record";

export function EvidenceStatusPanel({ record, onSeek }: { record: VerifiableRecord | null; onSeek: (timestampMs: number) => void }) {
  const evidence = record?.generated ? generatedFindings(record.generated).flatMap((finding) => finding.evidence.map((reference) => ({ ...reference, label: finding.label }))) : [];
  return (
    <aside aria-labelledby="evidence-heading" data-review-column="evidence" className="min-h-0 overflow-y-auto bg-white p-4">
      <h2 id="evidence-heading" className="font-semibold text-gray-900">Evidence</h2>
      <p className="mt-2 text-sm leading-6 text-gray-600">Every accepted finding resolves to a playable principal-transcript passage.</p>
      {evidence.length === 0 ? <p className="mt-4 rounded border border-dashed p-3 text-sm text-gray-600">No current evidence yet.</p> : (
        <ul className="mt-4 space-y-3">
          {evidence.map((reference) => (
            <li key={`${reference.block_id}-${reference.timestamp_ms}`} className="rounded border border-gray-200 p-3 text-sm">
              <p className="font-medium text-gray-900">{reference.label}</p>
              <p className="mt-1 font-mono text-xs text-gray-500">{reference.block_id}</p>
              <button type="button" onClick={() => onSeek(reference.timestamp_ms)} className="mt-2 text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700">Play at {(reference.timestamp_ms / 1000).toFixed(1)}s</button>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
