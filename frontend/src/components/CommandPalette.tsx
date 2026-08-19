'use client';

import { useEffect, useMemo, useState } from 'react';
import { FileText, Home, Mic, Search, Settings, Upload } from 'lucide-react';
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from '@/components/ui/command';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { shortDateLabel } from '@/lib/session-grouping';

/**
 * ⌘K used to expand the sidebar and focus its filter box, which only ever
 * filtered the list already on screen. This searches Sessions and commands in
 * one place, and can hand a full-text query to the results page.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const { meetings, navigate, setCurrentMeeting, handleRecordingToggle } = useSidebar();
  const { openImportDialog } = useImportDialog();
  const { isRecording } = useRecordingState();

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const usesMac = navigator.platform.toUpperCase().includes('MAC');
      if (event.key.toLowerCase() !== 'k' || !(usesMac ? event.metaKey : event.ctrlKey)) return;
      event.preventDefault();
      setOpen((current) => !current);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  const recent = useMemo(() => meetings.slice(0, 20), [meetings]);
  const trimmed = query.trim();

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput
        placeholder="Search Sessions, or type to search inside transcripts…"
        value={query}
        onValueChange={setQuery}
      />
      <CommandList>
        <CommandEmpty>Nothing matched that.</CommandEmpty>

        {trimmed.length > 1 && (
          <CommandGroup heading="Full text">
            <CommandItem
              value={`search-transcripts-${trimmed}`}
              onSelect={() => run(() => navigate(`/search?q=${encodeURIComponent(trimmed)}`))}
            >
              <Search className="mr-2 size-4" aria-hidden="true" />
              Search transcripts for &ldquo;{trimmed}&rdquo;
            </CommandItem>
          </CommandGroup>
        )}

        {recent.length > 0 && (
          <CommandGroup heading="Sessions">
            {recent.map((session) => (
              <CommandItem
                key={session.id}
                value={session.title}
                onSelect={() =>
                  run(() => {
                    if (navigate(`/meeting-details?id=${session.id}`)) {
                      setCurrentMeeting({ id: session.id, title: session.title });
                    }
                  })
                }
              >
                <FileText className="mr-2 size-4" aria-hidden="true" />
                <span className="truncate">{session.title}</span>
                <span className="ml-auto pl-3 text-xs tabular-nums text-muted-foreground">
                  {shortDateLabel(session.createdAt)}
                </span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}

        <CommandSeparator />

        <CommandGroup heading="Commands">
          <CommandItem value="start recording" disabled={isRecording} onSelect={() => run(handleRecordingToggle)}>
            <Mic className="mr-2 size-4" aria-hidden="true" />
            Start Recording
            <span className="ml-auto text-xs text-muted-foreground">⌘R</span>
          </CommandItem>
          <CommandItem value="import media" onSelect={() => run(() => openImportDialog())}>
            <Upload className="mr-2 size-4" aria-hidden="true" />
            Import Media
          </CommandItem>
          <CommandItem value="home" onSelect={() => run(() => navigate('/'))}>
            <Home className="mr-2 size-4" aria-hidden="true" />
            Home
          </CommandItem>
          <CommandItem value="settings" onSelect={() => run(() => navigate('/settings'))}>
            <Settings className="mr-2 size-4" aria-hidden="true" />
            Settings
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}
