# Data Model: Review and Correct Verifiable Records

## Meeting / Session

- `id`: existing stable identifier.
- `record_version`: non-negative integer. Starts at `0`; increments once per atomic principal-record save.
- Existing title, folder, timestamps, summary, and processing state remain unchanged.

## Participant

- `id`: stable identifier unique inside a Session.
- `meeting_id`: owning Session.
- `display_name`: trimmed non-empty user-facing label; duplicates allowed.
- `speaker_cluster_id`: optional immutable processing identity used during migration and assignment.
- `created_at`, `updated_at`: storage timestamps.

Constraint: `(meeting_id, id)` is unique. A participant cannot move between Sessions.

## Transcript Block

Editable: `transcript` and `participant_id`.

Immutable during Ticket 7 save: `id`, `meeting_id`, wall-clock timestamp, source origin, speaker-cluster
identity, ambiguity group, alignment decision, audio start/end, and duration.

## Edit Session

Frontend-only state:

- `baseline`: last loaded or successfully saved editable values.
- `present`: displayed editable values.
- `past`: edit commands available to undo.
- `future`: edit commands available to redo.
- `dirty`: `present` differs from `baseline`.
- `save_status`: idle, saving, or failed.

```text
loaded/saved → edit → dirty → save(version matches) → saved(version + 1)
                       │             └─ conflict/failure → dirty + actionable error
                       ├─ undo ↔ redo
                       └─ leave → native confirmation → stay or discard
```

## Principal Record Save

Input contains `meeting_id`, `expected_version`, and only changed participant names and transcript editable
fields. The backend validates each changed ID and ownership, then commits all changes and the version increment
together. A mismatch returns a typed conflict and changes nothing. Unloaded paginated blocks remain unchanged.
