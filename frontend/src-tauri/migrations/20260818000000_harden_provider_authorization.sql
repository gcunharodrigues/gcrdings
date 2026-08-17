ALTER TABLE provider_authorizations ADD COLUMN generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_authorizations ADD COLUMN ready_generation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE provider_authorizations ADD COLUMN credential_account TEXT NOT NULL DEFAULT '';
ALTER TABLE meetings ADD COLUMN canonical_snapshot_version INTEGER NOT NULL DEFAULT 0;

CREATE TRIGGER provider_snapshot_meeting_update
AFTER UPDATE OF title, principal_transcript_revision ON meetings
BEGIN
    UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.id;
END;

CREATE TRIGGER provider_snapshot_participant_insert AFTER INSERT ON participants BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_participant_update AFTER UPDATE ON participants BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_participant_delete AFTER DELETE ON participants BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = OLD.meeting_id; END;
CREATE TRIGGER provider_snapshot_transcript_insert AFTER INSERT ON transcripts BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_transcript_update AFTER UPDATE ON transcripts BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_transcript_delete AFTER DELETE ON transcripts BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = OLD.meeting_id; END;
CREATE TRIGGER provider_snapshot_generation_insert AFTER INSERT ON record_generations BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_generation_update AFTER UPDATE ON record_generations BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = NEW.meeting_id; END;
CREATE TRIGGER provider_snapshot_generation_delete AFTER DELETE ON record_generations BEGIN UPDATE meetings SET canonical_snapshot_version = canonical_snapshot_version + 1 WHERE id = OLD.meeting_id; END;

ALTER TABLE provider_transfers RENAME TO provider_transfers_legacy;

CREATE TABLE provider_transfers (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    task TEXT NOT NULL,
    snapshot_revision INTEGER NOT NULL,
    snapshot_version INTEGER NOT NULL,
    snapshot_digest TEXT NOT NULL,
    configuration_generation INTEGER NOT NULL,
    preview_digest TEXT NOT NULL UNIQUE,
    data_types_json TEXT NOT NULL,
    purpose TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('confirmed', 'sending', 'completed', 'failed', 'outcome_unknown')),
    result_json TEXT,
    error_code TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

INSERT INTO provider_transfers (
    id, meeting_id, provider, task, snapshot_revision, snapshot_version, snapshot_digest,
    configuration_generation, preview_digest, data_types_json, purpose,
    status, result_json, error_code, created_at, completed_at
)
SELECT
    id, meeting_id, provider, task, snapshot_revision, 0, preview_digest,
    0, preview_digest, data_types_json, purpose,
    status, result_json, error_code, created_at, completed_at
FROM provider_transfers_legacy;

DROP TABLE provider_transfers_legacy;
