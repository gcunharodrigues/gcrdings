CREATE TABLE record_generations (
    meeting_id TEXT PRIMARY KEY,
    record_type TEXT NOT NULL CHECK (record_type IN ('meeting', 'interview', 'content')),
    status TEXT NOT NULL CHECK (status IN ('pending', 'processing', 'completed', 'unavailable', 'failed', 'stale')),
    input_revision INTEGER,
    result_json TEXT,
    error_code TEXT,
    model_version TEXT,
    prompt_version TEXT NOT NULL,
    active_revision INTEGER,
    active_request_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    CHECK ((status = 'processing') = (active_revision IS NOT NULL AND active_request_id IS NOT NULL))
);

