ALTER TABLE transcripts ADD COLUMN language TEXT;

CREATE TABLE processing_jobs (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'interrupted')),
    language TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_manifest_hash TEXT NOT NULL CHECK (length(input_manifest_hash) = 64),
    source_origins TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    warning_code TEXT,
    warning_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX processing_jobs_one_active_per_meeting
ON processing_jobs(meeting_id)
WHERE status = 'running';
