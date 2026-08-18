"use client";

import type { Finding, GeneratedRecord, VerifiableRecord } from "@/types/verifiable-record";
import { generatedFindings } from "@/types/verifiable-record";

export interface ShapeProps {
  record: VerifiableRecord;
  onSeek: (timestampMs: number) => void;
}

/** Category names differ per record type; the grouping logic does not. */
export function findingGroups(generated: GeneratedRecord): Array<{ label: string; findings: Finding[] }> {
  if (generated.record_type === "meeting") {
    return [
      { label: "Decisions", findings: generated.decisions },
      { label: "Action items", findings: generated.action_items },
      { label: "Key points", findings: generated.key_points },
    ];
  }
  if (generated.record_type === "interview") {
    return [
      { label: "Answers", findings: generated.answers },
      { label: "Themes", findings: generated.themes },
      { label: "Follow-ups", findings: generated.follow_ups },
    ];
  }
  return [
    { label: "Claims", findings: generated.claims },
    { label: "Outline", findings: generated.outline },
    { label: "Source notes", findings: generated.source_notes },
  ];
}

function earliestTimestampMs(finding: Finding): number {
  if (finding.evidence.length === 0) return Number.POSITIVE_INFINITY;
  return Math.min(...finding.evidence.map((reference) => reference.timestamp_ms));
}

function formatTimestamp(timestampMs: number): string {
  const totalSeconds = Math.floor(timestampMs / 1000);
  return `${Math.floor(totalSeconds / 60)}:${(totalSeconds % 60).toString().padStart(2, "0")}`;
}

function EvidenceButton({ timestampMs, onSeek, label }: { timestampMs: number; onSeek: (ms: number) => void; label?: string }) {
  return (
    <button
      type="button"
      onClick={() => onSeek(timestampMs)}
      className="rounded bg-blue-50 px-2 py-1 text-xs font-medium text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
    >
      {label ?? `Play evidence at ${formatTimestamp(timestampMs)}`}
    </button>
  );
}

export function BriefView({ record, onSeek }: ShapeProps) {
  return (
    <div className="space-y-5">
      {findingGroups(record.generated!).map((group) => (
        group.findings.length > 0 && (
          <section key={group.label}>
            <h2 className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-gray-500">{group.label}</h2>
            <ul className="space-y-1.5">
              {group.findings.map((finding, index) => (
                <li key={`${finding.label}-${index}`} className="flex items-baseline gap-2 text-sm">
                  <span className="mt-1.5 size-1 shrink-0 rounded-full bg-gray-400" aria-hidden="true" />
                  <span className="text-gray-800">{finding.label}</span>
                  {finding.evidence[0] && (
                    <button
                      type="button"
                      onClick={() => onSeek(finding.evidence[0].timestamp_ms)}
                      className="shrink-0 font-mono text-xs text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
                    >
                      {formatTimestamp(finding.evidence[0].timestamp_ms)}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        )
      ))}
    </div>
  );
}

export function ProseView({ record, onSeek }: ShapeProps) {
  return (
    <div className="space-y-5">
      <p className="leading-7 text-gray-800">{record.generated!.summary}</p>
      {findingGroups(record.generated!).map((group) => (
        group.findings.length > 0 && (
          <section key={group.label}>
            <h2 className="mb-2 font-semibold text-gray-900">{group.label}</h2>
            {group.findings.map((finding, index) => (
              <p key={`${finding.label}-${index}`} className="mb-3 leading-7 text-gray-800">
                <span className="font-medium">{finding.label}.</span> {finding.detail}{" "}
                {finding.evidence.map((reference) => (
                  <EvidenceButton key={`${reference.block_id}-${reference.timestamp_ms}`} timestampMs={reference.timestamp_ms} onSeek={onSeek} />
                ))}
              </p>
            ))}
          </section>
        )
      ))}
    </div>
  );
}

export function TableView({ record, onSeek }: ShapeProps) {
  const rows = findingGroups(record.generated!).flatMap((group) =>
    group.findings.map((finding) => ({ group: group.label, finding })),
  );

  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[34rem] border-collapse text-sm">
        <caption className="sr-only">Findings with their category and supporting evidence</caption>
        <thead>
          <tr className="border-b border-gray-200 text-left text-xs font-semibold uppercase tracking-wide text-gray-500">
            <th scope="col" className="py-2 pr-3">Category</th>
            <th scope="col" className="py-2 pr-3">Finding</th>
            <th scope="col" className="py-2">Evidence</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ group, finding }, index) => (
            <tr key={`${finding.label}-${index}`} className="border-b border-gray-100 align-top">
              <td className="py-2.5 pr-3 text-xs text-gray-500">{group}</td>
              <td className="py-2.5 pr-3">
                <span className="font-medium text-gray-900">{finding.label}</span>
                <span className="block text-gray-600">{finding.detail}</span>
              </td>
              <td className="py-2.5">
                <div className="flex flex-wrap gap-1">
                  {finding.evidence.map((reference) => (
                    <EvidenceButton
                      key={`${reference.block_id}-${reference.timestamp_ms}`}
                      timestampMs={reference.timestamp_ms}
                      onSeek={onSeek}
                      label={formatTimestamp(reference.timestamp_ms)}
                    />
                  ))}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function TimelineView({ record, onSeek }: ShapeProps) {
  const ordered = findingGroups(record.generated!)
    .flatMap((group) => group.findings.map((finding) => ({ group: group.label, finding })))
    .filter(({ finding }) => Number.isFinite(earliestTimestampMs(finding)))
    .sort((a, b) => earliestTimestampMs(a.finding) - earliestTimestampMs(b.finding));

  if (ordered.length === 0) {
    return <p className="rounded border border-dashed p-4 text-sm text-gray-600">No finding carries a timestamp yet.</p>;
  }

  return (
    <ol className="relative space-y-4 border-l border-gray-200 pl-5">
      {ordered.map(({ group, finding }, index) => {
        const timestampMs = earliestTimestampMs(finding);
        return (
          <li key={`${finding.label}-${index}`} className="relative">
            <span className="absolute -left-[1.4rem] top-1.5 size-2 rounded-full bg-blue-600 ring-2 ring-white" aria-hidden="true" />
            <button
              type="button"
              onClick={() => onSeek(timestampMs)}
              className="font-mono text-xs text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
            >
              {formatTimestamp(timestampMs)}
            </button>
            <span className="ml-2 text-xs text-gray-500">{group}</span>
            <p className="font-medium text-gray-900">{finding.label}</p>
            <p className="text-sm leading-6 text-gray-700">{finding.detail}</p>
          </li>
        );
      })}
    </ol>
  );
}

export function MapView({ record, onSeek }: ShapeProps) {
  return (
    <div className="space-y-4">
      <div className="rounded-lg border border-gray-300 bg-gray-50 px-3 py-2">
        <p className="text-sm font-semibold text-gray-900">{record.title}</p>
        <p className="text-xs text-gray-600">{generatedFindings(record.generated!).length} findings</p>
      </div>
      <ul className="space-y-3 border-l border-gray-200 pl-4">
        {findingGroups(record.generated!).map((group) => (
          group.findings.length > 0 && (
            <li key={group.label}>
              <p className="text-sm font-semibold text-gray-800">{group.label}</p>
              <ul className="mt-1.5 space-y-1.5 border-l border-gray-200 pl-4">
                {group.findings.map((finding, index) => (
                  <li key={`${finding.label}-${index}`}>
                    <p className="text-sm text-gray-800">{finding.label}</p>
                    {finding.evidence[0] && (
                      <button
                        type="button"
                        onClick={() => onSeek(finding.evidence[0].timestamp_ms)}
                        className="font-mono text-xs text-blue-800 underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700"
                      >
                        {formatTimestamp(finding.evidence[0].timestamp_ms)}
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </li>
          )
        ))}
      </ul>
    </div>
  );
}
