-- User-defined organisation for Sessions.
--
-- Two mechanisms because they answer different questions. A folder answers
-- "where does this live" and a Session has exactly one. A tag answers "what is
-- this about" and a Session has many. Forcing either to do the other's job is
-- what makes filing systems collapse.

CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE CASCADE
);

-- Sibling names must be unique so the tree stays readable, but two different
-- parents may each hold a "Clients". COALESCE keeps root-level folders (NULL
-- parent) inside the same constraint, since NULLs are distinct in SQLite.
CREATE UNIQUE INDEX folders_unique_name_per_parent
ON folders(COALESCE(parent_id, ''), name COLLATE NOCASE);

CREATE INDEX folders_by_parent ON folders(parent_id);

-- Deleting a folder must never delete recordings. The Session falls back to
-- the root instead, which is why this is SET NULL and not CASCADE.
ALTER TABLE meetings ADD COLUMN folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL;

CREATE INDEX meetings_by_folder ON meetings(folder_id);

CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX tags_unique_name ON tags(name COLLATE NOCASE);

CREATE TABLE meeting_tags (
    meeting_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (meeting_id, tag_id),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

-- "Which Sessions carry this tag" is the read the filter performs.
CREATE INDEX meeting_tags_by_tag ON meeting_tags(tag_id);
