CREATE TABLE provider_security_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    legacy_secret_purge_pending INTEGER NOT NULL CHECK (legacy_secret_purge_pending IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO provider_security_state (id, legacy_secret_purge_pending)
VALUES (1, 1);
