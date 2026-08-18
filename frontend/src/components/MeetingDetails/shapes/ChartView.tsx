"use client";

import { useMemo } from "react";
import type { ShapeProps } from "./FindingViews";
import { findingGroups } from "./FindingViews";

/**
 * Two magnitude comparisons across a handful of categories, so horizontal bars
 * with direct labels rather than a plot. Every bar is labelled, so identity
 * never rests on colour alone.
 *
 * Palette validated for CVD separation (worst adjacent pair ΔE 16.6 protan).
 */
const CATEGORY_COLORS = ["#2563eb", "#f59e0b", "#0d9488"];

interface BarDatum {
  label: string;
  value: number;
  formatted: string;
  color: string;
}

function BarRow({ datum, max }: { datum: BarDatum; max: number }) {
  const width = max > 0 ? Math.max(2, (datum.value / max) * 100) : 0;
  return (
    <div className="grid grid-cols-[minmax(6rem,9rem)_1fr_auto] items-center gap-2">
      <span className="truncate text-xs text-gray-700" title={datum.label}>{datum.label}</span>
      <div className="h-3 rounded-sm bg-gray-100">
        <div className="h-full rounded-sm" style={{ width: `${width}%`, backgroundColor: datum.color }} />
      </div>
      <span className="tabular-nums text-xs text-gray-600">{datum.formatted}</span>
    </div>
  );
}

function BarGroup({ title, data, emptyMessage }: { title: string; data: BarDatum[]; emptyMessage: string }) {
  const max = Math.max(0, ...data.map((datum) => datum.value));
  return (
    <section>
      <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">{title}</h2>
      {data.length === 0 ? (
        <p className="rounded border border-dashed p-3 text-sm text-gray-600">{emptyMessage}</p>
      ) : (
        <div className="space-y-1.5">
          {data.map((datum) => <BarRow key={datum.label} datum={datum} max={max} />)}
        </div>
      )}
    </section>
  );
}

export function ChartView({ record }: ShapeProps) {
  const byCategory = useMemo<BarDatum[]>(
    () =>
      findingGroups(record.generated!).map((group, index) => ({
        label: group.label,
        value: group.findings.length,
        formatted: String(group.findings.length),
        color: CATEGORY_COLORS[index % CATEGORY_COLORS.length],
      })),
    [record.generated],
  );

  const byParticipant = useMemo<BarDatum[]>(() => {
    const spokenMs = new Map<string, number>();
    for (const passage of record.transcript) {
      const duration = Math.max(0, passage.end_ms - passage.start_ms);
      spokenMs.set(passage.participant_id, (spokenMs.get(passage.participant_id) ?? 0) + duration);
    }

    return record.participants
      .map((participant, index) => {
        const totalMs = spokenMs.get(participant.id) ?? 0;
        return {
          label: participant.display_name || "Unnamed participant",
          value: totalMs,
          formatted: `${Math.round(totalMs / 1000)}s`,
          color: CATEGORY_COLORS[index % CATEGORY_COLORS.length],
        };
      })
      .filter((datum) => datum.value > 0)
      .sort((a, b) => b.value - a.value);
  }, [record.participants, record.transcript]);

  return (
    <div className="space-y-6">
      <BarGroup title="Findings per category" data={byCategory} emptyMessage="No findings yet." />
      <BarGroup
        title="Speaking time"
        data={byParticipant}
        emptyMessage="Assign passages to participants to see who spoke for how long."
      />
      <p className="text-xs text-gray-500">
        Speaking time is measured from the principal transcript, so corrections change it.
      </p>
    </div>
  );
}
