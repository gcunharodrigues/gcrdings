ALTER TABLE meetings ADD COLUMN principal_transcript_revision INTEGER NOT NULL DEFAULT 0;

CREATE TABLE participants (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
    speaker_cluster_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX participants_meeting_id_idx ON participants(meeting_id);

ALTER TABLE transcripts ADD COLUMN participant_id TEXT REFERENCES participants(id);

INSERT INTO participants (id, meeting_id, display_name, speaker_cluster_id)
SELECT
    meeting_id || ':participant:' || COALESCE(NULLIF(speaker_cluster_id, ''), NULLIF(speaker, ''), 'unknown'),
    meeting_id,
    COALESCE(MIN(NULLIF(trim(speaker), '')), MIN(NULLIF(trim(speaker_cluster_id), '')), 'Unknown participant'),
    MAX(NULLIF(speaker_cluster_id, ''))
FROM transcripts
GROUP BY meeting_id, COALESCE(NULLIF(speaker_cluster_id, ''), NULLIF(speaker, ''), 'unknown');

UPDATE transcripts
SET participant_id = meeting_id || ':participant:' || COALESCE(NULLIF(speaker_cluster_id, ''), NULLIF(speaker, ''), 'unknown');

CREATE TRIGGER transcripts_assign_principal_participant
AFTER INSERT ON transcripts
WHEN NEW.participant_id IS NULL
BEGIN
    INSERT OR IGNORE INTO participants (id, meeting_id, display_name, speaker_cluster_id)
    VALUES (
        NEW.meeting_id || ':participant:' || COALESCE(NULLIF(NEW.speaker_cluster_id, ''), NULLIF(NEW.speaker, ''), 'unknown'),
        NEW.meeting_id,
        COALESCE(NULLIF(trim(NEW.speaker), ''), NULLIF(trim(NEW.speaker_cluster_id), ''), 'Unknown participant'),
        NULLIF(NEW.speaker_cluster_id, '')
    );

    UPDATE transcripts
    SET participant_id = NEW.meeting_id || ':participant:' || COALESCE(NULLIF(NEW.speaker_cluster_id, ''), NULLIF(NEW.speaker, ''), 'unknown')
    WHERE id = NEW.id;
END;
