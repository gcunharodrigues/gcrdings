"use client";

import { CalendarDays, Clock } from "lucide-react";

interface SessionHeaderProps {
  title: string;
  createdAt?: string;
  passageCount?: number;
  durationMs?: number;
}

function formatCreatedAt(value?: string): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleString(undefined, {
    day: "2-digit",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(durationMs?: number): string | null {
  if (durationMs === undefined || !Number.isFinite(durationMs) || durationMs <= 0) return null;
  const totalMinutes = Math.round(durationMs / 60000);
  if (totalMinutes < 60) return `${totalMinutes} min`;
  return `${Math.floor(totalMinutes / 60)}h ${totalMinutes % 60}min`;
}

export function SessionHeader({ title, createdAt, passageCount, durationMs }: SessionHeaderProps) {
  const created = formatCreatedAt(createdAt);
  const duration = formatDuration(durationMs);

  return (
    <div className="min-w-0 flex-1">
      <h1 className="truncate font-semibold text-foreground" title={title}>
        {title}
      </h1>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs text-muted-foreground">
        {created && (
          <span className="flex items-center gap-1">
            <CalendarDays className="size-3" aria-hidden="true" />
            {created}
          </span>
        )}
        {duration && (
          <span className="flex items-center gap-1">
            <Clock className="size-3" aria-hidden="true" />
            {duration}
          </span>
        )}
        {passageCount !== undefined && passageCount > 0 && (
          <span>{passageCount} passages</span>
        )}
      </div>
    </div>
  );
}
