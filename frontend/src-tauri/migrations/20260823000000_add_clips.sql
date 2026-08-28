-- A clip is a range of a Session, cut from the original media.
--
-- Two markers delimit it. Recordings made here are audio only, so a clip of a
-- recording is audio; a clip of an imported video keeps its video, because it
-- is cut from the preserved original rather than from the extracted audio.
CREATE TABLE clips (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    title TEXT,
    start_ms INTEGER NOT NULL CHECK (start_ms >= 0),
    end_ms INTEGER NOT NULL,
    -- Rendering runs ffmpeg and takes time, so a clip exists before its file
    -- does. Without a status the UI could not tell "still cutting" from "cut
    -- and then deleted from disk".
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'rendering', 'ready', 'failed')),
    file_path TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (end_ms > start_ms),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Every read is "the clips of this Session, in playback order".
CREATE INDEX clips_by_meeting ON clips(meeting_id, start_ms);
