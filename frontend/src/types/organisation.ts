/** Where a Session lives. Exactly one per Session, or none for the root. */
export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  createdAt: string;
  /** Sessions filed directly here, excluding children. */
  sessionCount: number;
}

/** What a Session is about. Many per Session. */
export interface Tag {
  id: string;
  name: string;
  color: string | null;
  createdAt: string;
  sessionCount: number;
}

export interface OrganisationErrorShape {
  code: 'not_found' | 'duplicate_name' | 'circular_parent' | 'blank_name' | 'storage';
  message: string;
}

const MESSAGES: Record<OrganisationErrorShape['code'], string> = {
  not_found: 'That folder or tag no longer exists.',
  duplicate_name: 'Something here already has that name.',
  circular_parent: 'A folder cannot be moved inside itself.',
  blank_name: 'The name cannot be blank.',
  storage: 'The change could not be saved.',
};

export function organisationErrorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const code = (error as OrganisationErrorShape).code;
    if (code in MESSAGES) return MESSAGES[code];
  }
  return MESSAGES.storage;
}

/** Depth-first order with a depth for indentation, so the tree renders flat. */
export function flattenFolders(folders: Folder[]): Array<Folder & { depth: number }> {
  const byParent = new Map<string | null, Folder[]>();
  for (const folder of folders) {
    const siblings = byParent.get(folder.parentId);
    if (siblings) siblings.push(folder);
    else byParent.set(folder.parentId, [folder]);
  }

  const output: Array<Folder & { depth: number }> = [];
  const walk = (parentId: string | null, depth: number) => {
    // A folder whose parent vanished would otherwise be invisible; the walk
    // starts at null so orphans are the caller's problem, not a silent loss.
    for (const folder of byParent.get(parentId) ?? []) {
      output.push({ ...folder, depth });
      walk(folder.id, depth + 1);
    }
  };
  walk(null, 0);
  return output;
}
