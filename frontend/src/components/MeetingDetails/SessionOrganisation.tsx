"use client";

import { useState } from "react";
import { Check, FolderIcon, Plus, TagIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useOrganisation, useSessionTags } from "@/hooks/useOrganisation";
import { flattenFolders } from "@/types/organisation";

/**
 * Filing a Session from the Session itself, rather than only from the sidebar.
 * This is where someone actually is when they realise what a recording was
 * about.
 */
export function SessionOrganisation({ meetingId }: { meetingId: string }) {
  const { folders, tags, attachTag, detachTag, createTag, setSessionFolder } = useOrganisation();
  const { sessionTags, reload } = useSessionTags(meetingId);
  const [draftTag, setDraftTag] = useState("");

  const owned = new Set(sessionTags.map((tag) => tag.id));

  const toggleTag = async (tagId: string) => {
    if (owned.has(tagId)) await detachTag(meetingId, tagId);
    else await attachTag(meetingId, tagId);
    await reload();
  };

  const addTag = async () => {
    const name = draftTag.trim();
    if (!name) return;
    setDraftTag("");
    // Reuse an existing tag rather than failing on the unique index: to a
    // person, typing a name that already exists means "use that one".
    const existing = tags.find((tag) => tag.name.toLowerCase() === name.toLowerCase());
    if (existing) {
      await toggleTag(existing.id);
      return;
    }
    const created = await createTag(name);
    if (created) await toggleTag(created);
  };

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {sessionTags.map((tag) => (
        <button
          key={tag.id}
          type="button"
          onClick={() => void toggleTag(tag.id)}
          aria-label={`Remove tag ${tag.name}`}
          className="rounded-full bg-blue-50 px-2 py-0.5 text-[11px] font-medium text-blue-800 hover:bg-blue-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
        >
          {tag.name}
        </button>
      ))}

      <Popover>
        <PopoverTrigger asChild>
          <Button type="button" variant="ghost" size="sm" aria-label="Organise this Session">
            <Plus className="size-3.5" aria-hidden="true" />
            {sessionTags.length === 0 ? "Tag" : ""}
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-64 p-2">
          <p className="mb-1 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            <TagIcon className="size-3" aria-hidden="true" /> Tags
          </p>
          <div className="max-h-40 space-y-0.5 overflow-y-auto">
            {tags.map((tag) => (
              <button
                key={tag.id}
                type="button"
                onClick={() => void toggleTag(tag.id)}
                aria-pressed={owned.has(tag.id)}
                className="flex w-full items-center gap-2 rounded px-2 py-1 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              >
                <Check className={`size-3.5 shrink-0 ${owned.has(tag.id) ? "text-blue-700" : "text-transparent"}`} aria-hidden="true" />
                <span className="truncate">{tag.name}</span>
              </button>
            ))}
          </div>
          <input
            value={draftTag}
            aria-label="New tag for this Session"
            placeholder="New tag…"
            onChange={(event) => setDraftTag(event.target.value)}
            onKeyDown={(event) => { if (event.key === "Enter") void addTag(); }}
            className="mt-2 w-full rounded border border-border bg-card px-2 py-1 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
          />

          <p className="mb-1 mt-3 flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            <FolderIcon className="size-3" aria-hidden="true" /> Folder
          </p>
          <div className="max-h-32 space-y-0.5 overflow-y-auto">
            <button
              type="button"
              onClick={() => void setSessionFolder(meetingId, null)}
              className="w-full rounded px-2 py-1 text-left text-sm text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
            >
              No folder
            </button>
            {flattenFolders(folders).map((folder) => (
              <button
                key={folder.id}
                type="button"
                onClick={() => void setSessionFolder(meetingId, folder.id)}
                style={{ paddingLeft: `${folder.depth * 10 + 8}px` }}
                className="w-full truncate rounded py-1 pr-2 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              >
                {folder.name}
              </button>
            ))}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
