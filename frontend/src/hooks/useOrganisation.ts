'use client';

import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { log } from '@/lib/logger';
import { organisationErrorMessage, type Folder, type Tag } from '@/types/organisation';

/**
 * Folders and tags are both loaded here because the sidebar renders them
 * together and every mutation invalidates the counts on the other list.
 */
export function useOrganisation() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const reload = useCallback(async () => {
    try {
      const [nextFolders, nextTags] = await Promise.all([
        invoke<Folder[]>('api_list_folders'),
        invoke<Tag[]>('api_list_tags'),
      ]);
      setFolders(nextFolders);
      setTags(nextTags);
    } catch (reason) {
      log.warn('[organisation] Failed to load folders and tags:', reason);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => { void reload(); }, [reload]);

  /** Every mutation follows the same shape: act, report, reload. */
  const run = useCallback(async (action: () => Promise<unknown>, success?: string) => {
    try {
      await action();
      await reload();
      if (success) toast.success(success);
      return true;
    } catch (reason) {
      log.warn('[organisation] Mutation failed:', reason);
      toast.error(organisationErrorMessage(reason));
      return false;
    }
  }, [reload]);

  return {
    folders,
    tags,
    isLoading,
    reload,
    createFolder: (name: string, parentId: string | null = null) =>
      run(() => invoke('api_create_folder', { name, parentId }), `Folder "${name}" created`),
    renameFolder: (folderId: string, name: string) =>
      run(() => invoke('api_rename_folder', { folderId, name })),
    moveFolder: (folderId: string, parentId: string | null) =>
      run(() => invoke('api_move_folder', { folderId, parentId })),
    deleteFolder: (folderId: string) =>
      run(() => invoke('api_delete_folder', { folderId }), 'Folder deleted. Its Sessions moved to the root.'),
    setSessionFolder: (meetingId: string, folderId: string | null) =>
      run(() => invoke('api_set_session_folder', { meetingId, folderId })),
    /** Returns the new tag's id so the caller can attach it without re-reading. */
    createTag: async (name: string, color: string | null = null): Promise<string | null> => {
      try {
        const tagId = await invoke<string>('api_create_tag', { name, color });
        await reload();
        return tagId;
      } catch (reason) {
        log.warn('[organisation] Failed to create tag:', reason);
        toast.error(organisationErrorMessage(reason));
        return null;
      }
    },
    renameTag: (tagId: string, name: string, color: string | null = null) =>
      run(() => invoke('api_rename_tag', { tagId, name, color })),
    deleteTag: (tagId: string) => run(() => invoke('api_delete_tag', { tagId }), 'Tag deleted'),
    attachTag: (meetingId: string, tagId: string) =>
      run(() => invoke('api_attach_tag', { meetingId, tagId })),
    detachTag: (meetingId: string, tagId: string) =>
      run(() => invoke('api_detach_tag', { meetingId, tagId })),
  };
}

/** Tags of one Session, kept separate so the panel does not reload the world. */
export function useSessionTags(meetingId: string | null) {
  const [sessionTags, setSessionTags] = useState<Tag[]>([]);

  const reload = useCallback(async () => {
    if (!meetingId) {
      setSessionTags([]);
      return;
    }
    try {
      setSessionTags(await invoke<Tag[]>('api_get_session_tags', { meetingId }));
    } catch (reason) {
      log.warn('[organisation] Failed to load Session tags:', reason);
    }
  }, [meetingId]);

  useEffect(() => { void reload(); }, [reload]);

  return { sessionTags, reload };
}
