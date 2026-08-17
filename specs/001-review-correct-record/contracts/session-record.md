# Session Record Native Contract

## `api_get_session_record`

Input: `{ meetingId: string }`

Output: `meeting_id`, `record_version`, `participants[]`, and `transcripts[]`. Each transcript includes editable
text and participant assignment plus immutable provenance and timing. Missing Session is an error. Malformed
legacy assignments receive a stable fallback participant without changing provenance.

## `api_save_session_record`

Input: `meetingId`, `expectedVersion`, `participantChanges: [{ id, displayName }]`, and
`transcriptChanges: [{ id, text?, participantId? }]`. Only changed fields are sent.

Success: `{ recordVersion: expectedVersion + 1 }` after one transaction.

Validation errors: blank participant name; unknown or duplicate participant ID; unknown or duplicate
transcript ID; cross-Session participant; an empty patch; malformed version.

Conflict: current version differs. No row changes. UI keeps edits and offers reload/reconciliation.

Storage failure: no partial state and no version increment. UI keeps edits for retry.

## `api_read_session_audio`

Input: `{ meetingId: string }`.

Output: bytes of the registered mixed track. Backend obtains `folder_path` from SQLite, selects a supported
`audio.*`, canonicalizes folder and file, and rejects missing, empty, symlink-escaped, or non-file targets.
Frontend cannot supply a filesystem path.
