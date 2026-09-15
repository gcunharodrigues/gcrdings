'use client';

import { useState } from 'react';
import { ChevronDown, ChevronRight, Folder as FolderIcon, FolderPlus, Tag as TagIcon, Trash2, X } from 'lucide-react';
import { useOrganisation } from '@/hooks/useOrganisation';
import { flattenFolders } from '@/types/organisation';

export interface OrganisationSelection {
  folderId: string | null;
  tagIds: string[];
}

/**
 * Folders and tags act as filters over one list rather than nesting the list
 * inside a tree. The Session list keeps the same shape whatever is selected,
 * so people do not have to re-learn where things are when they file something.
 */
export function OrganisationFilter({
  selection,
  onChange,
}: {
  selection: OrganisationSelection;
  onChange: (selection: OrganisationSelection) => void;
}) {
  const { folders, tags, createFolder, deleteFolder, createTag, deleteTag } = useOrganisation();
  const [expanded, setExpanded] = useState(false);
  const [draftFolder, setDraftFolder] = useState<string | null>(null);
  const [draftTag, setDraftTag] = useState<string | null>(null);

  const hasAny = folders.length > 0 || tags.length > 0;
  const isFiltering = selection.folderId !== null || selection.tagIds.length > 0;

  const toggleTag = (tagId: string) => {
    onChange({
      ...selection,
      tagIds: selection.tagIds.includes(tagId)
        ? selection.tagIds.filter((id) => id !== tagId)
        : [...selection.tagIds, tagId],
    });
  };

  const submitDraft = async (kind: 'folder' | 'tag', value: string) => {
    const name = value.trim();
    if (name) await (kind === 'folder' ? createFolder(name) : createTag(name));
    if (kind === 'folder') setDraftFolder(null);
    else setDraftTag(null);
  };

  return (
    <div className="mx-3 mt-2">
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={() => setExpanded((current) => !current)}
          aria-expanded={expanded}
          className="flex flex-1 items-center gap-1 rounded px-1 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
        >
          {expanded ? <ChevronDown className="size-3" aria-hidden="true" /> : <ChevronRight className="size-3" aria-hidden="true" />}
          Organise
          {!expanded && hasAny && (
            <span className="ml-1 font-normal normal-case tracking-normal">
              {folders.length} folders · {tags.length} tags
            </span>
          )}
        </button>
        {isFiltering && (
          <button
            type="button"
            onClick={() => onChange({ folderId: null, tagIds: [] })}
            aria-label="Clear filters"
            className="rounded p-1 text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
          >
            <X className="size-3" aria-hidden="true" />
          </button>
        )}
      </div>

      {expanded && (
        <div className="mt-1 space-y-3 pb-2">
          <section>
            <div className="mb-1 flex items-center justify-between">
              <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/70">Folders</span>
              <button
                type="button"
                onClick={() => setDraftFolder('')}
                aria-label="New folder"
                className="rounded p-0.5 text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              >
                <FolderPlus className="size-3.5" aria-hidden="true" />
              </button>
            </div>

            {flattenFolders(folders).map((folder) => (
              <div key={folder.id} className="group flex items-center gap-1">
                <button
                  type="button"
                  onClick={() =>
                    onChange({ ...selection, folderId: selection.folderId === folder.id ? null : folder.id })
                  }
                  aria-pressed={selection.folderId === folder.id}
                  style={{ paddingLeft: `${folder.depth * 10 + 4}px` }}
                  className={`flex min-w-0 flex-1 items-center gap-1.5 rounded py-1 pr-1 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 ${
                    selection.folderId === folder.id ? 'bg-blue-100 font-medium text-blue-700' : 'hover:bg-accent'
                  }`}
                >
                  <FolderIcon className="size-3 shrink-0" aria-hidden="true" />
                  <span className="truncate">{folder.name}</span>
                  <span className="ml-auto shrink-0 pl-1 tabular-nums text-muted-foreground/70">{folder.sessionCount}</span>
                </button>
                <button
                  type="button"
                  onClick={() => void deleteFolder(folder.id)}
                  aria-label={`Delete folder ${folder.name}`}
                  className="shrink-0 rounded p-0.5 text-muted-foreground opacity-0 hover:text-red-600 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 group-hover:opacity-100"
                >
                  <Trash2 className="size-3" aria-hidden="true" />
                </button>
              </div>
            ))}

            {draftFolder !== null && (
              <input
                autoFocus
                value={draftFolder}
                aria-label="New folder name"
                placeholder="Folder name"
                onChange={(event) => setDraftFolder(event.target.value)}
                onBlur={() => void submitDraft('folder', draftFolder)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') void submitDraft('folder', draftFolder);
                  else if (event.key === 'Escape') setDraftFolder(null);
                }}
                className="mt-1 w-full rounded border border-blue-400 bg-card px-1.5 py-0.5 text-xs focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            )}
          </section>

          <section>
            <div className="mb-1 flex items-center justify-between">
              <span className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground/70">Tags</span>
              <button
                type="button"
                onClick={() => setDraftTag('')}
                aria-label="New tag"
                className="rounded p-0.5 text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600"
              >
                <TagIcon className="size-3.5" aria-hidden="true" />
              </button>
            </div>

            <div className="flex flex-wrap gap-1">
              {tags.map((tag) => (
                <button
                  key={tag.id}
                  type="button"
                  onClick={() => toggleTag(tag.id)}
                  onAuxClick={() => void deleteTag(tag.id)}
                  aria-pressed={selection.tagIds.includes(tag.id)}
                  title={`${tag.name} · ${tag.sessionCount} Sessions`}
                  className={`rounded-full px-2 py-0.5 text-[11px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-600 ${
                    selection.tagIds.includes(tag.id)
                      ? 'bg-blue-600 text-white'
                      : 'bg-secondary text-foreground/90 hover:bg-accent'
                  }`}
                >
                  {tag.name}
                </button>
              ))}
            </div>

            {draftTag !== null && (
              <input
                autoFocus
                value={draftTag}
                aria-label="New tag name"
                placeholder="Tag name"
                onChange={(event) => setDraftTag(event.target.value)}
                onBlur={() => void submitDraft('tag', draftTag)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') void submitDraft('tag', draftTag);
                  else if (event.key === 'Escape') setDraftTag(null);
                }}
                className="mt-1 w-full rounded border border-blue-400 bg-card px-1.5 py-0.5 text-xs focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            )}
          </section>
        </div>
      )}
    </div>
  );
}
