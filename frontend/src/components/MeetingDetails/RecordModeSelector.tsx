"use client";

import { Check, Settings2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  RECORD_SHAPES,
  RECORD_TYPE_LABELS,
  RECORD_VOICES,
  type RecordMode,
  type RecordShape,
  type RecordVoice,
} from "@/types/record-modes";
import type { RecordType } from "@/types/verifiable-record";

interface RecordModeSelectorProps {
  mode: RecordMode;
  onChange: (mode: RecordMode) => void;
  onSaveAsDefault: () => void;
  isDefault: boolean;
  /** Shapes needing data this Session lacks are offered but explained. */
  hasParticipants: boolean;
  hasTimestamps: boolean;
  disabled?: boolean;
}

function OptionRow({
  label,
  description,
  selected,
  disabledReason,
  onSelect,
}: {
  label: string;
  description: string;
  selected: boolean;
  disabledReason?: string;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      disabled={Boolean(disabledReason)}
      aria-pressed={selected}
      className={`flex w-full items-start gap-2 rounded px-2 py-1.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 disabled:opacity-50 ${
        selected ? "bg-blue-50" : "hover:bg-muted/40"
      }`}
    >
      <Check
        aria-hidden="true"
        className={`mt-0.5 size-3.5 shrink-0 ${selected ? "text-blue-700" : "text-transparent"}`}
      />
      <span>
        <span className="block text-sm font-medium text-foreground">{label}</span>
        <span className="block text-xs text-muted-foreground">{disabledReason ?? description}</span>
      </span>
    </button>
  );
}

export function RecordModeSelector({
  mode,
  onChange,
  onSaveAsDefault,
  isDefault,
  hasParticipants,
  hasTimestamps,
  disabled = false,
}: RecordModeSelectorProps) {
  const shapeUnavailable = (shape: RecordShape): string | undefined => {
    const definition = RECORD_SHAPES[shape];
    if (definition.requiresParticipants && !hasParticipants) return "Needs named participants.";
    if (definition.requiresTimestamps && !hasTimestamps) return "Needs evidence timestamps.";
    return undefined;
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button type="button" variant="outline" size="sm" disabled={disabled} aria-label="Change how this record reads and looks">
          <Settings2 className="size-4" aria-hidden="true" />
          {RECORD_VOICES[mode.voice].label} · {RECORD_SHAPES[mode.shape].label}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[24rem] p-0">
        <div className="max-h-[28rem] overflow-y-auto p-3">
          <section>
            <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Record type</h3>
            <p className="mb-1.5 text-xs text-muted-foreground">Which findings are extracted.</p>
            <div className="space-y-0.5">
              {(Object.keys(RECORD_TYPE_LABELS) as RecordType[]).map((recordType) => (
                <OptionRow
                  key={recordType}
                  label={RECORD_TYPE_LABELS[recordType]}
                  description={
                    recordType === "meeting"
                      ? "Decisions, action items, key points."
                      : recordType === "interview"
                        ? "Answers, themes, follow-ups."
                        : "Claims, outline, source notes."
                  }
                  selected={mode.recordType === recordType}
                  onSelect={() => onChange({ ...mode, recordType })}
                />
              ))}
            </div>
          </section>

          <section className="mt-4 border-t border-border pt-3">
            <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Voice</h3>
            <p className="mb-1.5 text-xs text-muted-foreground">How the findings are worded.</p>
            <div className="space-y-0.5">
              {(Object.keys(RECORD_VOICES) as RecordVoice[]).map((voice) => (
                <OptionRow
                  key={voice}
                  label={RECORD_VOICES[voice].label}
                  description={RECORD_VOICES[voice].description}
                  selected={mode.voice === voice}
                  onSelect={() => onChange({ ...mode, voice })}
                />
              ))}
            </div>
            <p className="mt-2 rounded bg-muted/40 p-2 text-xs italic leading-5 text-foreground/90">
              &ldquo;{RECORD_VOICES[mode.voice].sample}&rdquo;
            </p>
          </section>

          <section className="mt-4 border-t border-border pt-3">
            <h3 className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Layout</h3>
            <p className="mb-1.5 text-xs text-muted-foreground">How they are shown. Changing this needs no regeneration.</p>
            <div className="space-y-0.5">
              {(Object.keys(RECORD_SHAPES) as RecordShape[]).map((shape) => (
                <OptionRow
                  key={shape}
                  label={RECORD_SHAPES[shape].label}
                  description={RECORD_SHAPES[shape].description}
                  disabledReason={shapeUnavailable(shape)}
                  selected={mode.shape === shape}
                  onSelect={() => onChange({ ...mode, shape })}
                />
              ))}
            </div>
          </section>
        </div>

        <div className="flex items-center justify-between gap-2 border-t border-border px-3 py-2">
          <p className="text-xs text-muted-foreground">
            {isDefault ? "This is your default." : "Applies to this Session only."}
          </p>
          <Button type="button" variant="ghost" size="sm" onClick={onSaveAsDefault} disabled={isDefault}>
            Make default
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
