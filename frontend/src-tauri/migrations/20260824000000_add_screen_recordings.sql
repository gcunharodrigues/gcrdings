-- Screen recordings captured alongside a Session's audio.
--
-- started_at_offset_ms is the point in the audio recording at which the screen
-- capture began. The two paths start moments apart, and a player that assumes
-- a shared zero drifts for the whole Session.
CREATE TABLE screen_recordings (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    started_at_offset_ms INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('display', 'window')),
    target_label TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX screen_recordings_by_meeting ON screen_recordings(meeting_id);
