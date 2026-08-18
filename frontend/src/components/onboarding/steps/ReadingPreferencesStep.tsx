'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { OnboardingContainer } from '../OnboardingContainer';
import { useOnboarding } from '@/contexts/OnboardingContext';
import { writeDefaultRecordMode } from '@/lib/record-mode-preferences';
import {
  DEFAULT_RECORD_MODE,
  RECORD_SHAPES,
  RECORD_TYPE_LABELS,
  RECORD_VOICES,
  type RecordMode,
  type RecordShape,
  type RecordVoice,
} from '@/types/record-modes';
import type { RecordType } from '@/types/verifiable-record';

/**
 * Three questions, all skippable. Asking once here is what lets every later
 * record arrive already in the shape this reader wants, instead of being
 * reconfigured Session by Session.
 */
function Choice({
  label,
  hint,
  selected,
  onSelect,
}: {
  label: string;
  hint: string;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-pressed={selected}
      className={`rounded-lg border p-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 ${
        selected ? 'border-blue-600 bg-blue-50' : 'border-gray-200 bg-white hover:border-gray-300'
      }`}
    >
      <span className="block text-sm font-medium text-gray-900">{label}</span>
      <span className="mt-0.5 block text-xs leading-5 text-gray-600">{hint}</span>
    </button>
  );
}

export function ReadingPreferencesStep() {
  const { completeOnboarding } = useOnboarding();
  const [mode, setMode] = useState<RecordMode>(DEFAULT_RECORD_MODE);
  const [isFinishing, setIsFinishing] = useState(false);

  const finish = async (save: boolean) => {
    setIsFinishing(true);
    try {
      if (save) writeDefaultRecordMode(mode);
      await completeOnboarding();
      window.location.reload();
    } catch (error) {
      console.error('Failed to complete onboarding:', error);
      setIsFinishing(false);
    }
  };

  return (
    <OnboardingContainer
      step={5}
      totalSteps={5}
      title="How do you like to read?"
      description="This sets the default for every record. You can change it per Session at any time."
    >
      <div className="space-y-6">
        <fieldset>
          <legend className="mb-2 text-sm font-semibold text-gray-900">What do you record most?</legend>
          <div className="grid gap-2 sm:grid-cols-3">
            {(Object.keys(RECORD_TYPE_LABELS) as RecordType[]).map((recordType) => (
              <Choice
                key={recordType}
                label={RECORD_TYPE_LABELS[recordType]}
                hint={
                  recordType === 'meeting'
                    ? 'Decisions and action items'
                    : recordType === 'interview'
                      ? 'Answers and themes'
                      : 'Claims and outline'
                }
                selected={mode.recordType === recordType}
                onSelect={() => setMode({ ...mode, recordType })}
              />
            ))}
          </div>
        </fieldset>

        <fieldset>
          <legend className="mb-2 text-sm font-semibold text-gray-900">How should it be written?</legend>
          <div className="grid gap-2 sm:grid-cols-2">
            {(Object.keys(RECORD_VOICES) as RecordVoice[]).map((voice) => (
              <Choice
                key={voice}
                label={RECORD_VOICES[voice].label}
                hint={RECORD_VOICES[voice].description}
                selected={mode.voice === voice}
                onSelect={() => setMode({ ...mode, voice })}
              />
            ))}
          </div>
          <p className="mt-2 rounded bg-gray-50 p-2 text-xs italic leading-5 text-gray-700">
            &ldquo;{RECORD_VOICES[mode.voice].sample}&rdquo;
          </p>
        </fieldset>

        <fieldset>
          <legend className="mb-2 text-sm font-semibold text-gray-900">How should it look?</legend>
          <div className="grid gap-2 sm:grid-cols-3">
            {(Object.keys(RECORD_SHAPES) as RecordShape[]).map((shape) => (
              <Choice
                key={shape}
                label={RECORD_SHAPES[shape].label}
                hint={RECORD_SHAPES[shape].description}
                selected={mode.shape === shape}
                onSelect={() => setMode({ ...mode, shape })}
              />
            ))}
          </div>
        </fieldset>

        <div className="flex flex-col gap-3 pt-2">
          <Button onClick={() => void finish(true)} disabled={isFinishing} className="h-11 w-full">
            Finish Setup
          </Button>
          <button
            type="button"
            onClick={() => void finish(false)}
            disabled={isFinishing}
            className="text-sm text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Skip — decide later
          </button>
        </div>
      </div>
    </OnboardingContainer>
  );
}
