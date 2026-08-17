CREATE TABLE provider_authorizations (
    provider TEXT NOT NULL,
    task TEXT NOT NULL,
    display_name TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    model TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (provider, task)
);

CREATE TABLE provider_transfers (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    task TEXT NOT NULL,
    snapshot_revision INTEGER NOT NULL,
    preview_digest TEXT NOT NULL UNIQUE,
    data_types_json TEXT NOT NULL,
    purpose TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('confirmed', 'sending', 'completed', 'failed')),
    result_json TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
