"use client";

import { Suspense, useEffect, useState } from "react";
import { useSearchParams } from "next/navigation";
import { LoaderIcon } from "lucide-react";
import { useSidebar } from "@/components/Sidebar/SidebarProvider";
import { usePaginatedTranscripts } from "@/hooks/usePaginatedTranscripts";
import PageContent from "./page-content";

function MeetingDetailsContent() {
  const meetingId = useSearchParams().get("id");
  const { setCurrentMeeting, navigate } = useSidebar();
  const [meeting, setMeeting] = useState<any>(null);
  const { metadata, transcripts, isLoading, isLoadingMore, hasMore, totalCount, loadedCount, loadMore, refetch, error } = usePaginatedTranscripts({ meetingId: meetingId ?? "" });

  useEffect(() => {
    if (!metadata || !meetingId || meetingId === "intro-call") return;
    setMeeting({ ...metadata, transcripts });
    setCurrentMeeting({ id: metadata.id, title: metadata.title });
  }, [meetingId, metadata, setCurrentMeeting, transcripts]);

  if (!meetingId || meetingId === "intro-call" || error) {
    return <div className="flex h-screen items-center justify-center"><div className="text-center"><p className="mb-4 text-red-700">{error ?? "No Session selected"}</p><button type="button" onClick={() => navigate("/")} className="rounded bg-blue-700 px-4 py-2 text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-700 focus-visible:ring-offset-2">Go back</button></div></div>;
  }
  if (isLoading || !meeting) return <div className="flex h-screen items-center justify-center"><LoaderIcon aria-label="Loading Session" className="size-6 animate-spin" /></div>;
  return <PageContent meeting={meeting} onRefetchTranscripts={refetch} hasMore={hasMore} isLoadingMore={isLoadingMore} totalCount={totalCount} loadedCount={loadedCount} onLoadMore={loadMore} />;
}

export default function MeetingDetails() {
  return <Suspense fallback={<div className="flex h-screen items-center justify-center"><LoaderIcon aria-label="Loading Session" className="size-6 animate-spin" /></div>}><MeetingDetailsContent /></Suspense>;
}
