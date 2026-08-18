import { generationMessage, type VerifiableRecord } from "@/types/verifiable-record";
import type { RecordMode } from "@/types/record-modes";
import { RECORD_SHAPES } from "@/types/record-modes";
import { BriefView, MapView, ProseView, TableView, TimelineView, type ShapeProps } from "./shapes/FindingViews";
import { ChartView } from "./shapes/ChartView";

const SHAPE_VIEWS: Record<RecordMode["shape"], (props: ShapeProps) => JSX.Element> = {
  brief: BriefView,
  prose: ProseView,
  table: TableView,
  timeline: TimelineView,
  map: MapView,
  chart: ChartView,
};

export function SummaryPanel({ record, loadError, hasUnsavedTranscript, mode, onGenerate, onCancel, onSeek }: {
  record: VerifiableRecord | null;
  loadError: string | null;
  hasUnsavedTranscript: boolean;
  mode: RecordMode;
  onGenerate: () => void;
  onCancel: () => void;
  onSeek: (timestampMs: number) => void;
}) {
  const processing = record?.generation_status === "processing";
  // A record generated under one record type cannot be re-laid-out into
  // another type's categories, so say so instead of rendering empty groups.
  const typeMatches = record?.generated?.record_type === mode.recordType;
  const ShapeView = SHAPE_VIEWS[mode.shape];

  return (
    <section aria-labelledby="findings-heading" className="flex min-h-0 flex-1 flex-col overflow-y-auto p-4">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-gray-200 pb-3">
        <div>
          <h1 id="findings-heading" className="text-lg font-semibold text-gray-900">Grounded findings</h1>
          <p className="text-xs text-gray-500">Every accepted finding resolves to a playable principal-transcript passage.</p>
        </div>
        {processing ? (
          <button type="button" onClick={onCancel} className="rounded border border-red-700 px-3 py-2 text-sm font-medium text-red-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-700">Cancel</button>
        ) : (
          <button type="button" onClick={onGenerate} disabled={!record || hasUnsavedTranscript || record.transcript.length === 0} className="rounded bg-blue-700 px-3 py-2 text-sm font-medium text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2 disabled:opacity-50">
            {typeMatches ? "Regenerate findings" : "Generate local findings"}
          </button>
        )}
      </div>

      {hasUnsavedTranscript && <p className="mt-3 rounded bg-amber-50 p-3 text-sm text-amber-900">Save transcript corrections before generating so evidence uses the principal record.</p>}
      <p aria-live="polite" className="mt-3 text-sm text-gray-600">{loadError ?? (record ? generationMessage(record.generation_status, record.error_code ?? undefined) : "Loading local findings…")}</p>

      {record?.generated && record.generation_status === "completed" && (
        typeMatches ? (
          <div className="mt-5">
            {mode.shape !== "prose" && <p className="mb-4 leading-7 text-gray-800">{record.generated.summary}</p>}
            <ShapeView record={record} onSeek={onSeek} />
          </div>
        ) : (
          <p className="mt-5 rounded border border-dashed p-4 text-sm text-gray-700">
            These findings were generated as a {record.generated.record_type} record. Generate again to get {mode.recordType} findings, or switch the record type back.
          </p>
        )
      )}

      {record?.generated && record.generation_status === "completed" && typeMatches && (
        <p className="mt-4 text-xs text-gray-400">Showing the {RECORD_SHAPES[mode.shape].label.toLowerCase()} layout.</p>
      )}
    </section>
  );
}
