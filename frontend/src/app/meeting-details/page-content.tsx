"use client";

import { useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import type { Transcript } from "@/types";
import { useSidebar } from "@/components/Sidebar/SidebarProvider";
import { TranscriptPanel } from "@/components/MeetingDetails/TranscriptPanel";
import { SummaryPanel } from "@/components/MeetingDetails/SummaryPanel";
import { ParticipantsPanel } from "@/components/MeetingDetails/ParticipantsPanel";
import { AgentHandoffMenu } from "@/components/MeetingDetails/AgentHandoffMenu";
import { ExternalTransferDialog } from "@/components/MeetingDetails/ExternalTransferDialog";
import { SessionHeader } from "@/components/MeetingDetails/SessionHeader";
import { ConfirmationModal } from "@/components/ConfirmationModel/confirmation-modal";
import { RecordModeSelector } from "@/components/MeetingDetails/RecordModeSelector";
import { readDefaultRecordMode, readSessionRecordMode, writeDefaultRecordMode, writeSessionRecordMode } from "@/lib/record-mode-preferences";
import type { RecordMode } from "@/types/record-modes";
import { useMeetingOperations } from "@/hooks/meeting-details/useMeetingOperations";
import { useReviewRecord } from "@/hooks/meeting-details/useReviewRecord";
import { useVerifiableRecord } from "@/hooks/meeting-details/useVerifiableRecord";
import { useAudioPlayer } from "@/hooks/useAudioPlayer";

export default function PageContent({ meeting, onRefetchTranscripts, hasMore, isLoadingMore, totalCount, loadedCount, onLoadMore }: {
  meeting: any;
  onRefetchTranscripts?: () => Promise<void>;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;
}) {
  const { setHasUnsavedReviewChanges } = useSidebar();
  const reviewRecord = useReviewRecord(meeting.id);
  const findings = useVerifiableRecord(meeting.id);
  const audioPlayer = useAudioPlayer(meeting.id);
  const meetingOperations = useMeetingOperations({ meeting });

  useEffect(() => {
    setHasUnsavedReviewChanges(Boolean(reviewRecord.state?.dirty));
    return () => setHasUnsavedReviewChanges(false);
  }, [reviewRecord.state?.dirty, setHasUnsavedReviewChanges]);

  useEffect(() => {
    if (!reviewRecord.state || meeting.transcripts.length === 0) return;
    reviewRecord.dispatch({
      type: "transcripts-merged",
      transcripts: meeting.transcripts.flatMap((transcript: Transcript) => transcript.participant_id ? [{ id: transcript.id, text: transcript.text, participantId: transcript.participant_id }] : []),
    });
  }, [meeting.transcripts, reviewRecord.state?.meetingId]);

  // The dialog resolves the promise the caller is awaiting, so the operation
  // still blocks on a real answer instead of the OS confirm sheet.
  const [discardPrompt, setDiscardPrompt] = useState(false);
  const discardResolver = useRef<((confirmed: boolean) => void) | null>(null);

  const answerDiscardPrompt = (confirmed: boolean) => {
    setDiscardPrompt(false);
    discardResolver.current?.(confirmed);
    discardResolver.current = null;
  };

  // The reader's default applies until this Session is given its own mode.
  const [mode, setMode] = useState<RecordMode>(() => readSessionRecordMode(meeting.id));
  const [defaultMode, setDefaultMode] = useState<RecordMode>(readDefaultRecordMode);

  const applyMode = (next: RecordMode) => {
    setMode(next);
    writeSessionRecordMode(meeting.id, next);
    if (next.recordType !== mode.recordType) void findings.selectType(next.recordType);
  };

  const makeModeDefault = () => {
    writeDefaultRecordMode(mode);
    setDefaultMode(mode);
  };

  const confirmDestructiveOperation = async () => {
    if (!reviewRecord.state?.dirty) return true;
    const confirmed = await new Promise<boolean>((resolve) => {
      discardResolver.current = resolve;
      setDiscardPrompt(true);
    });
    if (!confirmed) return false;
    await reviewRecord.reload();
    return true;
  };

  const savePrincipal = async () => {
    const saved = await reviewRecord.save();
    if (saved) await findings.reload();
    return saved;
  };
  const seek = (timestampMs: number) => { void audioPlayer.seekAndPlay(timestampMs / 1000); };

  // Only trustworthy once every passage is loaded, otherwise it would report
  // the duration of the loaded page instead of the Session.
  const sessionDurationMs = hasMore
    ? undefined
    : meeting.transcripts.reduce((longest: number, transcript: Transcript) => Math.max(longest, (transcript.audio_end_time ?? 0) * 1000), 0) || undefined;

  return (
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.3, ease: "easeOut" }} className="flex h-screen flex-col bg-background">
      <ConfirmationModal
        isOpen={discardPrompt}
        title="Discard transcript corrections?"
        text="This operation replaces the current draft. Unsaved transcript corrections are discarded."
        confirmLabel="Discard and continue"
        cancelLabel="Cancel"
        onConfirm={() => answerDiscardPrompt(true)}
        onCancel={() => answerDiscardPrompt(false)}
      />
      <header className="flex min-h-12 items-center gap-4 border-b border-border bg-card px-4 py-2">
        <SessionHeader
          title={meeting.title}
          createdAt={meeting.created_at}
          passageCount={totalCount ?? meeting.transcripts.length}
          durationMs={sessionDurationMs}
        />
        <RecordModeSelector
          mode={mode}
          onChange={applyMode}
          onSaveAsDefault={makeModeDefault}
          isDefault={mode.recordType === defaultMode.recordType && mode.voice === defaultMode.voice && mode.shape === defaultMode.shape}
          hasParticipants={(findings.record?.participants.length ?? 0) > 0}
          hasTimestamps={Boolean(findings.record?.transcript.some((passage) => passage.start_ms > 0))}
          disabled={findings.record?.generation_status === "processing"}
        />
        <ExternalTransferDialog meetingId={meeting.id} disabled={Boolean(reviewRecord.state?.dirty)} />
        <AgentHandoffMenu
          meetingId={meeting.id}
          hasUnsavedTranscript={Boolean(reviewRecord.state?.dirty)}
          onSaveTranscript={savePrincipal}
        />
      </header>
      <main className="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto lg:grid-cols-[minmax(18rem,1fr)_minmax(28rem,2fr)] lg:overflow-hidden">
        <section aria-label="Session findings" data-review-column="context" className="flex min-h-0 flex-col overflow-hidden bg-card">
          <SummaryPanel record={findings.record} loadError={findings.error} hasUnsavedTranscript={Boolean(reviewRecord.state?.dirty)} mode={mode} onGenerate={() => void findings.generate()} onCancel={() => void findings.cancel()} onSeek={seek} />
          {reviewRecord.state && <ParticipantsPanel state={reviewRecord.state} dispatch={reviewRecord.dispatch} />}
        </section>
        {reviewRecord.state ? (
          <TranscriptPanel state={reviewRecord.state} sourceTranscripts={meeting.transcripts} dispatch={reviewRecord.dispatch} save={savePrincipal} reload={reviewRecord.reload} audioPlayer={audioPlayer} hasMore={hasMore} isLoadingMore={isLoadingMore} totalCount={totalCount} loadedCount={loadedCount} onLoadMore={onLoadMore} meetingId={meeting.id} meetingFolderPath={meeting.folder_path} onRefetchTranscripts={onRefetchTranscripts} confirmDestructiveOperation={confirmDestructiveOperation} onOpenMeetingFolder={meetingOperations.handleOpenMeetingFolder} />
        ) : (
          <section aria-live="polite" data-review-column="transcript" className="flex items-center justify-center border-x border-border bg-card p-8 text-center"><div><p className="text-sm text-foreground/90">{reviewRecord.loadError ?? "Loading principal transcript…"}</p>{reviewRecord.loadError && <button type="button" onClick={() => void reviewRecord.reload()} className="mt-3 rounded bg-blue-700 px-3 py-2 text-sm text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2">Retry</button>}</div></section>
        )}
      </main>
    </motion.div>
  );
}
