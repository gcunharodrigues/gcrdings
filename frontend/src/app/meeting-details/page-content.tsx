"use client";

import { useEffect } from "react";
import { motion } from "framer-motion";
import type { Transcript } from "@/types";
import { useSidebar } from "@/components/Sidebar/SidebarProvider";
import { TranscriptPanel } from "@/components/MeetingDetails/TranscriptPanel";
import { SummaryPanel } from "@/components/MeetingDetails/SummaryPanel";
import { ParticipantsPanel } from "@/components/MeetingDetails/ParticipantsPanel";
import { EvidenceStatusPanel } from "@/components/MeetingDetails/EvidenceStatusPanel";
import { AgentHandoffMenu } from "@/components/MeetingDetails/AgentHandoffMenu";
import { ExternalTransferDialog } from "@/components/MeetingDetails/ExternalTransferDialog";
import { SessionHeader } from "@/components/MeetingDetails/SessionHeader";
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

  const confirmDestructiveOperation = async () => {
    if (!reviewRecord.state?.dirty) return true;
    if (!window.confirm("Discard unsaved transcript corrections and continue?")) return false;
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
    <motion.div initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.3, ease: "easeOut" }} className="flex h-screen flex-col bg-gray-50">
      <header className="flex min-h-12 items-center gap-4 border-b border-gray-200 bg-white px-4 py-2">
        <SessionHeader
          title={meeting.title}
          createdAt={meeting.created_at}
          passageCount={totalCount ?? meeting.transcripts.length}
          durationMs={sessionDurationMs}
        />
        <ExternalTransferDialog meetingId={meeting.id} disabled={Boolean(reviewRecord.state?.dirty)} />
        <AgentHandoffMenu meetingId={meeting.id} disabled={Boolean(reviewRecord.state?.dirty)} />
      </header>
      <main className="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto lg:grid-cols-[minmax(15rem,0.75fr)_minmax(24rem,1.8fr)_minmax(13rem,0.7fr)] lg:overflow-hidden">
        <section aria-label="Session findings" data-review-column="context" className="flex min-h-0 flex-col overflow-hidden bg-white">
          <SummaryPanel record={findings.record} loadError={findings.error} hasUnsavedTranscript={Boolean(reviewRecord.state?.dirty)} onSelectType={(type) => void findings.selectType(type)} onGenerate={() => void findings.generate()} onCancel={() => void findings.cancel()} onSeek={seek} />
          {reviewRecord.state && <ParticipantsPanel state={reviewRecord.state} dispatch={reviewRecord.dispatch} />}
        </section>
        {reviewRecord.state ? (
          <TranscriptPanel state={reviewRecord.state} sourceTranscripts={meeting.transcripts} dispatch={reviewRecord.dispatch} save={savePrincipal} reload={reviewRecord.reload} audioPlayer={audioPlayer} hasMore={hasMore} isLoadingMore={isLoadingMore} totalCount={totalCount} loadedCount={loadedCount} onLoadMore={onLoadMore} meetingId={meeting.id} meetingFolderPath={meeting.folder_path} onRefetchTranscripts={onRefetchTranscripts} confirmDestructiveOperation={confirmDestructiveOperation} onOpenMeetingFolder={meetingOperations.handleOpenMeetingFolder} />
        ) : (
          <section aria-live="polite" data-review-column="transcript" className="flex items-center justify-center border-x border-gray-200 bg-white p-8 text-center"><div><p className="text-sm text-gray-700">{reviewRecord.loadError ?? "Loading principal transcript…"}</p>{reviewRecord.loadError && <button type="button" onClick={() => void reviewRecord.reload()} className="mt-3 rounded bg-blue-700 px-3 py-2 text-sm text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2">Retry</button>}</div></section>
        )}
        <EvidenceStatusPanel record={findings.record} onSeek={seek} />
      </main>
    </motion.div>
  );
}
