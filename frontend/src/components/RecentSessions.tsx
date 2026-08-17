'use client';

import { FileText, Upload } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useImportDialog } from '@/contexts/ImportDialogContext';

/**
 * Shown on Home while nothing is being recorded. Without it the landing screen
 * is a headline over empty space even when the Mac holds dozens of Sessions.
 */
export function RecentSessions() {
  const { meetings, navigate, setCurrentMeeting } = useSidebar();
  const { openImportDialog } = useImportDialog();

  const recent = meetings.slice(0, 8);

  if (recent.length === 0) {
    return (
      <div className="mx-auto mt-8 max-w-md text-center">
        <p className="text-sm text-gray-600">No Sessions yet.</p>
        <p className="mt-1 text-xs text-gray-500">
          Press the record button below, or bring in audio you already have.
        </p>
        <button
          type="button"
          onClick={() => openImportDialog()}
          className="mt-4 inline-flex items-center gap-2 rounded-lg bg-blue-50 px-3 py-2 text-sm font-medium text-blue-700 transition-colors hover:bg-blue-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
        >
          <Upload className="size-4" aria-hidden="true" />
          Import Media
        </button>
      </div>
    );
  }

  return (
    <nav aria-label="Recent Sessions" className="mx-auto mt-8 w-full max-w-2xl px-4 text-left">
      <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">
        Recent Sessions
      </h2>
      <ul className="divide-y divide-gray-100 overflow-hidden rounded-lg border border-gray-200 bg-white">
        {recent.map((session) => (
          <li key={session.id}>
            <button
              type="button"
              onClick={() => {
                if (navigate(`/meeting-details?id=${session.id}`)) {
                  setCurrentMeeting({ id: session.id, title: session.title });
                }
              }}
              className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-gray-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-blue-600"
            >
              <FileText className="size-4 shrink-0 text-gray-400" aria-hidden="true" />
              <span className="truncate text-sm text-gray-800">{session.title}</span>
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
