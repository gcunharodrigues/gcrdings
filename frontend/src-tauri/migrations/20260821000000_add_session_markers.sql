-- Markers a person drops while recording, to be revisited later.
--
-- offset_ms is measured against the ACTIVE recording clock, the same one
-- transcripts use for audio_start_time, so a marker and a passage recorded at
-- the same moment agree. Wall-clock would drift apart from the audio whenever
-- the recording is paused.
CREATE TABLE session_markers (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    offset_ms INTEGER NOT NULL CHECK (offset_ms >= 0),
    label TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Every read is "the markers of this Session, in playback order".
CREATE INDEX session_markers_by_meeting ON session_markers(meeting_id, offset_ms);
