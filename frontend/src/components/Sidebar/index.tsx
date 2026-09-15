'use client';

import React, { useState, useMemo, useEffect, useCallback, useRef } from 'react';
import { ChevronDown, ChevronRight, File, Settings, ChevronLeftCircle, ChevronRightCircle, Calendar, StickyNote, Home, Trash2, Mic, Square, Plus, Search, Pencil, NotebookPen, SearchIcon, X, Upload } from 'lucide-react';
import { usePathname } from 'next/navigation';
import { useSidebar } from './SidebarProvider';
import type { CurrentMeeting } from '@/components/Sidebar/SidebarProvider';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { SettingTabs } from '../SettingTabs';
import { TranscriptModelProps } from '@/components/TranscriptSettings';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { toast } from 'sonner';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { useConfig } from '@/contexts/ConfigContext';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { SidebarAction } from './SidebarAction';
import { OrganisationFilter, type OrganisationSelection } from './OrganisationFilter';
import { BatchImportDialog } from '@/components/ImportAudio/BatchImportDialog';
import { FolderInput } from 'lucide-react';
import { groupByRecency, shortDateLabel } from '@/lib/session-grouping';

import { MessageToast } from '../MessageToast';
import Logo from '../Logo';
import Info from '../Info';
import { ComplianceNotification } from '../ComplianceNotification';
import { Input } from '../ui/input';
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '../ui/input-group';
import { log } from '@/lib/logger';

interface SidebarItem {
  id: string;
  title: string;
  type: 'folder' | 'file';
  createdAt?: string;
  folderId?: string | null;
  children?: SidebarItem[];
}

const Sidebar: React.FC = () => {
  const pathname = usePathname();
  const {
    currentMeeting,
    setCurrentMeeting,
    sidebarItems,
    isCollapsed,
    toggleCollapse,
    handleRecordingToggle,
    searchTranscripts,
    searchResults,
    isSearching,
    meetings,
    setMeetings,
    serverAddress,
    navigate,
    refetchMeetings,
  } = useSidebar();

  // Get recording state from RecordingStateContext (single source of truth)
  const { isRecording } = useRecordingState();
  const { openImportDialog } = useImportDialog();
  const { betaFeatures } = useConfig();
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set(['meetings']));
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [showModelSettings, setShowModelSettings] = useState(false);
  const [modelConfig, setModelConfig] = useState<ModelConfig>({
    provider: 'ollama',
    model: '',
    whisperModel: '',
    apiKey: null,
    ollamaEndpoint: null
  });
  const [transcriptModelConfig, setTranscriptModelConfig] = useState<TranscriptModelProps>({
    provider: 'parakeet',
    model: 'parakeet-tdt-0.6b-v3-int8',
  });
  const [settingsSaveSuccess, setSettingsSaveSuccess] = useState<boolean | null>(null);

  // Inline title editing: a row swaps to an input in place, so renaming is a
  // double-click plus Enter instead of a hover, a pencil and a modal.
  const [editingMeetingId, setEditingMeetingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState<string>('');

  // Ensure 'meetings' folder is always expanded
  useEffect(() => {
    if (!expandedFolders.has('meetings')) {
      const newExpanded = new Set(expandedFolders);
      newExpanded.add('meetings');
      setExpandedFolders(newExpanded);
    }
  }, [expandedFolders]);

  // useEffect(() => {
  //   if (settingsSaveSuccess !== null) {
  //     const timer = setTimeout(() => {
  //       setSettingsSaveSuccess(null);
  //     }, 3000);
  //   }
  // }, [settingsSaveSuccess]);


  // A delete hides the Session immediately and only reaches the backend once the
  // undo window closes, so nothing on disk is destroyed while undo is offered.
  const [pendingDeletions, setPendingDeletions] = useState<Record<string, CurrentMeeting>>({});
  const deleteTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  useEffect(() => {
    // Note: Don't set hardcoded defaults - let DB be the source of truth
    const fetchModelConfig = async () => {
      // Only make API call if serverAddress is loaded
      if (!serverAddress) {
        log.debug('Waiting for server address to load before fetching model config');
        return;
      }

      try {
        const data = await invoke('api_get_model_config') as any;
        if (data && data.provider !== null) {
          // Fetch API key if not included and provider requires it
          if (data.provider !== 'ollama' && !data.apiKey) {
            try {
              const apiKeyData = await invoke('api_get_api_key', {
                provider: data.provider
              }) as string;
              data.apiKey = apiKeyData;
            } catch (err) {
              console.error('Failed to fetch API key:', err);
            }
          }
          setModelConfig(data);
        }
      } catch (error) {
        console.error('Failed to fetch model config:', error);
      }
    };

    fetchModelConfig();
  }, [serverAddress]);


  useEffect(() => {
    // Note: Don't set hardcoded defaults - let DB be the source of truth
    const fetchTranscriptSettings = async () => {
      // Only make API call if serverAddress is loaded
      if (!serverAddress) {
        log.debug('Waiting for server address to load before fetching transcript settings');
        return;
      }

      try {
        const data = await invoke('api_get_transcript_config') as any;
        if (data && data.provider !== null) {
          setTranscriptModelConfig(data);
        }
      } catch (error) {
        console.error('Failed to fetch transcript settings:', error);
      }
    };
    fetchTranscriptSettings();
  }, [serverAddress]);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<ModelConfig>('model-config-updated', (event) => {
        log.debug('Sidebar received model-config-updated event:', event.payload);
        setModelConfig(event.payload);
      });

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);



  // Handle model config save
  const handleSaveModelConfig = async (config: ModelConfig) => {
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        whisperModel: config.whisperModel,
        apiKey: config.apiKey,
        ollamaEndpoint: config.ollamaEndpoint,
      });

      setModelConfig(config);
      log.debug('Model config saved successfully');
      setSettingsSaveSuccess(true);

      // Emit event to sync other components
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', config);

      // Track settings change
      await Analytics.trackSettingsChanged('model_config', `${config.provider}_${config.model}`);
    } catch (error) {
      console.error('Error saving model config:', error);
      setSettingsSaveSuccess(false);
    }
  };

  const handleSaveTranscriptConfig = async (updatedConfig?: TranscriptModelProps) => {
    try {
      const configToSave = updatedConfig || transcriptModelConfig;
      const payload = {
        provider: configToSave.provider,
        model: configToSave.model,
        apiKey: configToSave.apiKey ?? null
      };
      log.debug('Saving transcript config with payload:', payload);

      await invoke('api_save_transcript_config', {
        provider: payload.provider,
        model: payload.model,
        apiKey: payload.apiKey,
      });


      setSettingsSaveSuccess(true);

      // Track settings change
      const transcriptConfigToSave = updatedConfig || transcriptModelConfig;
      await Analytics.trackSettingsChanged('transcript_config', `${transcriptConfigToSave.provider}_${transcriptConfigToSave.model}`);
    } catch (error) {
      console.error('Failed to save transcript config:', error);
      setSettingsSaveSuccess(false);
    }
  };

  // Handle search input changes
  const handleSearchChange = useCallback(async (value: string) => {
    setSearchQuery(value);

    // If search query is empty, just return to normal view
    if (!value.trim()) return;

    // Search through transcripts
    await searchTranscripts(value);

    // Make sure the meetings folder is expanded when searching
    if (!expandedFolders.has('meetings')) {
      const newExpanded = new Set(expandedFolders);
      newExpanded.add('meetings');
      setExpandedFolders(newExpanded);
    }
  }, [expandedFolders, searchTranscripts]);

  // Combine search results with sidebar items
  const filteredSidebarItems = useMemo(() => {
    if (!searchQuery.trim()) return sidebarItems;

    // If we have search results, highlight matching meetings
    if (searchResults.length > 0) {
      // Get the IDs of meetings that matched in transcripts
      const matchedMeetingIds = new Set(searchResults.map(result => result.id));

      return sidebarItems
        .map(folder => {
          // Always include folders in the results
          if (folder.type === 'folder') {
            if (!folder.children) return folder;

            // Filter children based on search results or title match
            const filteredChildren = folder.children.filter(item => {
              // Include if the meeting ID is in our search results
              if (matchedMeetingIds.has(item.id)) return true;

              // Or if the title matches the search query
              return item.title.toLowerCase().includes(searchQuery.toLowerCase());
            });

            return {
              ...folder,
              children: filteredChildren
            };
          }

          // For non-folder items, check if they match the search
          return (matchedMeetingIds.has(folder.id) ||
            folder.title.toLowerCase().includes(searchQuery.toLowerCase()))
            ? folder : undefined;
        })
        .filter((item): item is SidebarItem => item !== undefined); // Type-safe filter
    } else {
      // Fall back to title-only filtering if no transcript results
      return sidebarItems
        .map(folder => {
          // Always include folders in the results
          if (folder.type === 'folder') {
            if (!folder.children) return folder;

            // Filter children based on search query
            const filteredChildren = folder.children.filter(item =>
              item.title.toLowerCase().includes(searchQuery.toLowerCase())
            );

            return {
              ...folder,
              children: filteredChildren
            };
          }

          // For non-folder items, check if they match the search
          return folder.title.toLowerCase().includes(searchQuery.toLowerCase()) ? folder : undefined;
        })
        .filter((item): item is SidebarItem => item !== undefined); // Type-safe filter
    }
  }, [sidebarItems, searchQuery, searchResults, expandedFolders]);


  const UNDO_WINDOW_MS = 8000;

  const commitDelete = async (item: CurrentMeeting) => {
    delete deleteTimers.current[item.id];
    setPendingDeletions((current) => {
      const { [item.id]: _removed, ...rest } = current;
      return rest;
    });

    try {
      await invoke('api_delete_meeting', { meetingId: item.id });
      Analytics.trackMeetingDeleted(item.id);
    } catch (error) {
      console.error('Failed to delete meeting:', error);
      // The Session was only hidden, so restoring the list is enough.
      setMeetings([...meetings, item]);
      toast.error('Failed to delete the Session', {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const undoDelete = (item: CurrentMeeting) => {
    clearTimeout(deleteTimers.current[item.id]);
    delete deleteTimers.current[item.id];
    setPendingDeletions((current) => {
      const { [item.id]: _restored, ...rest } = current;
      return rest;
    });
  };

  const handleDelete = (itemId: string) => {
    const item = meetings.find((m: CurrentMeeting) => m.id === itemId);
    if (!item || deleteTimers.current[itemId]) return;

    setPendingDeletions((current) => ({ ...current, [itemId]: item }));

    if (currentMeeting?.id === itemId) {
      setCurrentMeeting({ id: 'intro-call', title: '+ New Call' });
      navigate('/', true);
    }

    deleteTimers.current[itemId] = setTimeout(() => { void commitDelete(item); }, UNDO_WINDOW_MS);

    toast(`Deleted "${item.title}"`, {
      description: 'The recording and transcript are removed when this closes.',
      duration: UNDO_WINDOW_MS,
      action: { label: 'Undo', onClick: () => undoDelete(item) },
    });
  };

  // Never destroy anything on unmount: a Session still inside its undo window
  // stays on disk and reappears on the next launch.
  useEffect(() => {
    const timers = deleteTimers.current;
    return () => { Object.values(timers).forEach(clearTimeout); };
  }, []);

  const handleEditStart = (meetingId: string, currentTitle: string) => {
    setEditingMeetingId(meetingId);
    setEditingTitle(currentTitle);
  };

  const handleEditConfirm = async () => {
    const newTitle = editingTitle.trim();
    const meetingId = editingMeetingId;

    if (!meetingId) return;

    // Prevent empty titles
    if (!newTitle) {
      toast.error("Meeting title cannot be empty");
      return;
    }

    try {
      await invoke('api_save_meeting_title', {
        meetingId: meetingId,
        title: newTitle,
      });

      // Update local state
      const updatedMeetings = meetings.map((m: CurrentMeeting) =>
        m.id === meetingId ? { ...m, title: newTitle } : m
      );
      setMeetings(updatedMeetings);

      // Update current meeting if it's the one being edited
      if (currentMeeting?.id === meetingId) {
        setCurrentMeeting({ id: meetingId, title: newTitle });
      }

      // Track the edit
      Analytics.trackButtonClick('edit_meeting_title', 'sidebar');

      toast.success("Session title updated");

      setEditingMeetingId(null);
      setEditingTitle('');
    } catch (error) {
      console.error('Failed to update meeting title:', error);
      toast.error("Failed to update the Session title", {
        description: error instanceof Error ? error.message : String(error)
      });
    }
  };

  const handleEditCancel = () => {
    setEditingMeetingId(null);
    setEditingTitle('');
  };

  const [showBatchImport, setShowBatchImport] = useState(false);
  const [organisationFilter, setOrganisationFilter] = useState<OrganisationSelection>({
    folderId: null,
    tagIds: [],
  });
  // meetingId -> tagIds. Filtering by tag otherwise means one query per row.
  const [tagMembership, setTagMembership] = useState<Record<string, string[]>>({});

  useEffect(() => {
    if (organisationFilter.tagIds.length === 0) return;
    let cancelled = false;

    const loadMembership = async () => {
      const entries = await Promise.all(
        meetings.map(async (meeting: CurrentMeeting) => {
          try {
            const tags = await invoke<Array<{ id: string }>>('api_get_session_tags', {
              meetingId: meeting.id,
            });
            return [meeting.id, tags.map((tag) => tag.id)] as const;
          } catch {
            return [meeting.id, []] as const;
          }
        }),
      );
      if (!cancelled) setTagMembership(Object.fromEntries(entries));
    };

    void loadMembership();
    return () => { cancelled = true; };
  }, [meetings, organisationFilter.tagIds.length]);

  // A Session must satisfy the folder AND every selected tag. Tags narrow
  // rather than widen: picking two means "both", which is what people expect
  // from a filing system even though search boxes usually mean "either".
  const matchesOrganisation = useCallback((item: SidebarItem) => {
    if (organisationFilter.folderId !== null && item.folderId !== organisationFilter.folderId) {
      return false;
    }
    if (organisationFilter.tagIds.length === 0) return true;
    const owned = tagMembership[item.id] ?? [];
    return organisationFilter.tagIds.every((tagId) => owned.includes(tagId));
  }, [organisationFilter, tagMembership]);

  const searchInputRef = useRef<HTMLInputElement>(null);

  useKeyboardShortcuts(
    useMemo(
      () => [
        {
          key: 'b',
          mod: true,
          handler: toggleCollapse,
        },
        {
          key: 'r',
          mod: true,
          handler: () => { if (!isRecording) handleRecordingToggle(); },
        },
      ],
      [isRecording, toggleCollapse, handleRecordingToggle],
    ),
  );

  const toggleFolder = (folderId: string) => {
    // Normal toggle behavior for all folders
    const newExpanded = new Set(expandedFolders);
    if (newExpanded.has(folderId)) {
      newExpanded.delete(folderId);
    } else {
      newExpanded.add(folderId);
    }
    setExpandedFolders(newExpanded);
  };

  // Expose setShowModelSettings to window for Rust tray to call
  useEffect(() => {
    (window as any).openSettings = () => {
      setShowModelSettings(true);
    };

    // Cleanup on unmount
    return () => {
      delete (window as any).openSettings;
    };
  }, []);

  // One list drives both the collapsed rail and the expanded footer, so the two
  // can no longer drift apart the way they had.
  const sidebarActions = [
    {
      key: 'home',
      places: ['rail'] as const,
      icon: <Home className="w-5 h-5" />,
      label: 'Home',
      onClick: () => navigate('/'),
      tone: 'neutral' as const,
      isActive: pathname === '/',
    },
    {
      key: 'record',
      places: ['rail', 'footer'] as const,
      icon: isRecording ? <Square className="w-5 h-5" /> : <Mic className="w-5 h-5" />,
      label: isRecording ? 'Recording in progress' : 'Start Recording',
      onClick: handleRecordingToggle,
      tone: 'primary' as const,
      disabled: isRecording,
      shortcut: '⌘R',
    },
    {
      key: 'import',
      places: ['rail', 'footer'] as const,
      icon: <Upload className="w-5 h-5" />,
      label: 'Import Media',
      onClick: () => openImportDialog(),
      tone: 'accent' as const,
    },
    {
      key: 'import-folder',
      places: ['footer'] as const,
      icon: <FolderInput className="w-5 h-5" />,
      label: 'Import a folder',
      onClick: () => setShowBatchImport(true),
      tone: 'accent' as const,
    },
    {
      key: 'sessions',
      places: ['rail'] as const,
      icon: <NotebookPen className="w-5 h-5" />,
      label: 'Sessions',
      onClick: () => {
        if (isCollapsed) toggleCollapse();
        toggleFolder('meetings');
      },
      tone: 'neutral' as const,
      isActive: Boolean(pathname?.includes('/meeting-details')),
    },
    {
      key: 'settings',
      places: ['rail', 'footer'] as const,
      icon: <Settings className="w-5 h-5" />,
      label: 'Settings',
      onClick: () => navigate('/settings'),
      tone: 'neutral' as const,
      isActive: pathname === '/settings',
    },
  ];

  // The expanded sidebar already shows Home and the Session list as rows, so
  // repeating them as footer buttons would be a third copy of the same thing.
  const renderActions = (place: 'rail' | 'footer') =>
    sidebarActions
      .filter((action) => (action.places as readonly string[]).includes(place))
      .map(({ key, places: _places, ...action }) => (
        <SidebarAction key={key} collapsed={place === 'rail'} {...action} />
      ));

  const renderCollapsedIcons = () => {
    if (!isCollapsed) return null;
    return (
      <div className="flex flex-col items-center space-y-4 mt-4">
        <Logo isCollapsed={isCollapsed} />
        {renderActions('rail')}
        <Info isCollapsed={isCollapsed} />
      </div>
    );
  };

  // Find matching transcript snippet for a meeting item
  const findMatchingSnippet = (itemId: string) => {
    if (!searchQuery.trim() || !searchResults.length) return null;
    return searchResults.find(result => result.id === itemId);
  };

  const renderItem = (item: SidebarItem, depth = 0) => {
    if (pendingDeletions[item.id]) return null;
    const isExpanded = expandedFolders.has(item.id);
    const paddingLeft = `${depth * 12 + 12}px`;
    const isActive = item.type === 'file' && currentMeeting?.id === item.id;
    const isMeetingItem = item.id.includes('-') && !item.id.startsWith('intro-call');

    // Check if this item has a matching transcript snippet
    const matchingResult = isMeetingItem ? findMatchingSnippet(item.id) : null;
    const hasTranscriptMatch = !!matchingResult;

    if (isCollapsed) return null;

    return (
      <div key={item.id}>
        <div
          className={`flex items-center transition-all duration-150 group ${item.type === 'folder' && depth === 0
            ? 'p-3 text-lg font-semibold h-10 mx-3 mt-3 rounded-lg'
            : `px-3 py-2 my-0.5 rounded-md text-sm ${isActive ? 'bg-blue-100 text-blue-700 font-medium' :
              hasTranscriptMatch ? 'bg-yellow-50' : 'hover:bg-muted/40'
            } cursor-pointer`
            }`}
          style={item.type === 'folder' && depth === 0 ? {} : { paddingLeft }}
          onDoubleClick={() => {
            if (item.type === 'file' && isMeetingItem) handleEditStart(item.id, item.title);
          }}
          onClick={() => {
            if (editingMeetingId === item.id) return;
            if (item.type === 'folder') {
              toggleFolder(item.id);
            } else {
              const basePath = item.id.startsWith('intro-call') ? '/' : `/meeting-details?id=${item.id}`;
              if (navigate(basePath)) setCurrentMeeting({ id: item.id, title: item.title });
            }
          }}
        >
          {item.type === 'folder' ? (
            <>
              {item.id === 'meetings' ? (
                <Calendar className="w-4 h-4 mr-2" />
              ) : item.id === 'notes' ? (
                <Calendar className="w-4 h-4 mr-2" />
              ) : null}
              <span className={depth === 0 ? "" : "font-medium"}>{item.title}</span>
              <div className="ml-auto">
                {isExpanded ? (
                  <ChevronDown className="w-4 h-4 text-muted-foreground" />
                ) : (
                  <ChevronRight className="w-4 h-4 text-muted-foreground" />
                )}
              </div>
              {searchQuery && item.id === 'meetings' && isSearching && (
                <span className="ml-2 text-xs text-blue-500 animate-pulse">Searching...</span>
              )}
            </>
          ) : (
            <div className="flex flex-col w-full">
              <div className="flex items-center w-full">
                {isMeetingItem ? (
                  <div className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full mr-2 bg-muted">
                    <File className="w-3.5 h-3.5 text-muted-foreground" />
                  </div>
                ) : (
                  <div className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full mr-2 bg-blue-100">
                    <Plus className="w-3.5 h-3.5 text-blue-600" />
                  </div>
                )}
                {editingMeetingId === item.id ? (
                  <input
                    type="text"
                    value={editingTitle}
                    autoFocus
                    aria-label="Session title"
                    onClick={(e) => e.stopPropagation()}
                    onChange={(e) => setEditingTitle(e.target.value)}
                    onBlur={() => void handleEditConfirm()}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') void handleEditConfirm();
                      else if (e.key === 'Escape') handleEditCancel();
                    }}
                    className="flex-1 min-w-0 rounded border border-blue-400 bg-card px-1.5 py-0.5 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                  />
                ) : (
                  <span className="flex-1 min-w-0 break-words">{item.title}</span>
                )}
                {isMeetingItem && editingMeetingId !== item.id && item.createdAt && (
                  <span className="ml-2 shrink-0 text-[11px] tabular-nums text-muted-foreground/70 group-hover:hidden">
                    {shortDateLabel(item.createdAt)}
                  </span>
                )}
                {isMeetingItem && editingMeetingId !== item.id && (
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity duration-150">
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleEditStart(item.id, item.title);
                      }}
                      className="hover:text-blue-600 p-1 rounded-md hover:bg-blue-50 flex-shrink-0"
                      aria-label={`Rename ${item.title}`}
                    >
                      <Pencil className="w-4 h-4" />
                    </button>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(item.id);
                      }}
                      className="hover:text-red-600 p-1 rounded-md hover:bg-red-50 flex-shrink-0"
                      aria-label={`Delete ${item.title}`}
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                )}
              </div>

              {/* Show transcript match snippet if available */}
              {hasTranscriptMatch && (
                <div className="mt-1 ml-8 text-xs text-muted-foreground bg-yellow-50 p-1.5 rounded border border-yellow-100 line-clamp-2">
                  <span className="font-medium text-yellow-600">Match:</span> {matchingResult.matchContext}
                </div>
              )}
            </div>
          )}
        </div>
        {item.type === 'folder' && isExpanded && item.children && (
          <div className="ml-1">
            {item.children.map(child => renderItem(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  return (
    <TooltipProvider>
    <div className="fixed top-0 left-0 h-screen z-40">
      {/* Floating collapse button */}
      <button
        onClick={toggleCollapse}
        aria-label={isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        aria-expanded={!isCollapsed}
        className="absolute -right-6 top-20 z-50 p-1 bg-card hover:bg-muted rounded-full shadow-lg border"
        style={{ transform: 'translateX(50%)' }}
      >
        {isCollapsed ? (
          <ChevronRightCircle className="w-6 h-6" />
        ) : (
          <ChevronLeftCircle className="w-6 h-6" />
        )}
      </button>

      <div
        className={`h-screen bg-card border-r border-border shadow-sm flex flex-col transition-all duration-300 ${isCollapsed ? 'w-16' : 'w-64'
          }`}
      >
        {/*  Header with traffic light spacing */}
        <div className="flex-shrink-0 h-22 flex items-center">

          {/* Title container */}



          <div className="flex-1">
            {!isCollapsed && (
              <div className="p-3">
                {/* <span className="text-lg text-center border rounded-full bg-blue-50 border-white font-semibold text-foreground/90 mb-2 block items-center">
                  <span>gcrdings</span>
                </span> */}
                <Logo isCollapsed={isCollapsed} />

                <div className="relative mb-1">
                  <InputGroup >
                    <InputGroupInput ref={searchInputRef} placeholder='Filter this list' value={searchQuery}
                      onChange={(e) => handleSearchChange(e.target.value)}
                    />
                    <InputGroupAddon>
                      <SearchIcon />
                    </InputGroupAddon>
                    {searchQuery &&
                      <InputGroupAddon align={'inline-end'}>
                        <InputGroupButton
                          onClick={() => handleSearchChange('')}
                        >
                          <X />
                        </InputGroupButton>
                      </InputGroupAddon>
                    }
                  </InputGroup>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Main content - scrollable area */}
        <div className="flex-1 flex flex-col min-h-0">
          {/* Fixed navigation items */}
          <div className="flex-shrink-0">
            {!isCollapsed && (
              <div
                onClick={() => navigate('/')}
                className="p-3  text-lg font-semibold items-center hover:bg-muted h-10   flex mx-3 mt-3 rounded-lg cursor-pointer"
              >
                <Home className="w-4 h-4 mr-2" />
                <span>Home</span>
              </div>
            )}
          </div>

          {/* Content area */}
          <div className="flex-1 flex flex-col min-h-0">
            {renderCollapsedIcons()}
            {/* Meeting Notes folder header - fixed */}
            {!isCollapsed && (
              <div className="flex-shrink-0">
                {filteredSidebarItems.filter(item => item.type === 'folder').map(item => (
                  <div key={item.id}>
                    <div
                      className="flex items-center transition-all duration-150 p-3 text-lg font-semibold h-10 mx-3 mt-3 rounded-lg"
                    >
                      <NotebookPen className="w-4 h-4 mr-2 text-muted-foreground" />
                      <span className="text-foreground/90">{item.title}</span>
                      {searchQuery && item.id === 'meetings' && isSearching && (
                        <span className="ml-2 text-xs text-blue-500 animate-pulse">Searching...</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}

            {!isCollapsed && (
              <OrganisationFilter selection={organisationFilter} onChange={setOrganisationFilter} />
            )}

            {/* Scrollable meeting items */}
            {!isCollapsed && (
              <div className="flex-1 overflow-y-auto custom-scrollbar min-h-0">
                {filteredSidebarItems
                  .filter(item => item.type === 'folder' && expandedFolders.has(item.id) && item.children)
                  .map(item => (
                    <div key={`${item.id}-children`} className="mx-3">
                      {groupByRecency(item.children!.filter(child => !pendingDeletions[child.id] && matchesOrganisation(child))).map(group => (
                        <div key={group.bucket}>
                          <p className="px-3 pb-1 pt-3 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/70">
                            {group.label}
                          </p>
                          {group.items.map(child => renderItem(child, 1))}
                        </div>
                      ))}
                    </div>
                  ))}
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        {!isCollapsed && (

          <div className="flex-shrink-0 p-2 border-t border-border flex flex-col gap-1">
            {renderActions('footer')}
            <Info isCollapsed={isCollapsed} />
            <div className="w-full flex items-center justify-center px-3 py-1 text-xs text-muted-foreground/70">
              v0.4.0
            </div>
          </div>
        )}
      </div>

      <BatchImportDialog
        open={showBatchImport}
        onOpenChange={setShowBatchImport}
        onFinished={() => void refetchMeetings()}
      />
    </div>
    </TooltipProvider>
  );
};

export default Sidebar;
