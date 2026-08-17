# Research: Review and Correct Verifiable Records

## Atomic principal-record save

**Decision**: Save only changed participant names and transcript editable fields in one SQLite transaction.
Require the client's loaded `record_version`; increment it only on successful commit.

**Rationale**: One command prevents partial participant/transcript state. Optimistic version checking prevents
an older save from silently overwriting a newer window or retry.

**Alternatives considered**: One command per edit creates partial durable state. Sending the complete transcript
conflicts with existing pagination and forces unnecessary loading. Last-write wins loses newer work. A durable
event log is the V2 revision tree already excluded.

## Participant identity

**Decision**: Add a `participants` table keyed by stable ID and store `participant_id` on transcript rows.
Seed participants deterministically from existing `speaker_cluster_id` or legacy `speaker`, with a stable
fallback for malformed rows.

**Rationale**: Display names are editable and may duplicate. They cannot be identity. Existing speaker-cluster
provenance remains immutable and can map many transcript blocks to one participant.

**Alternatives considered**: Updating the `speaker` string in every row loses identity. Storing only a JSON
participant map weakens ownership checks.

## Ephemeral undo and redo

**Decision**: Keep one reducer-owned edit-session state in the frontend. Store edit commands with before/after
values. Clear redo on a new edit. Reset history after successful save or Session change.

**Rationale**: Ticket 7 requires only current-session history. A pure reducer is deterministic and small.

**Alternatives considered**: A dependency adds surface. Durable history violates V1. Full transcript snapshots
per keystroke waste memory on long Sessions.

## Mixed-track playback boundary

**Decision**: Add a Rust command that accepts `meeting_id`, reads the registered folder, canonicalizes it, and
returns bytes only for a supported mixed-track filename inside that folder. Reuse `useAudioPlayer`.

**Rationale**: The existing hook works but `read_audio_file(file_path)` trusts a renderer-supplied path.
Resolving by Session identity closes that boundary and supports captured and imported `audio.*` files.

**Alternatives considered**: Asset protocol scope excludes recording folders. Passing `folder_path` preserves
path authority in React. A streaming server is unnecessary at accepted scale.

## Accepted workspace and Ticket 8 boundary

**Decision**: Implement the three-column Evidence desk. The right column shows evidence readiness states and
the current summary surface. It does not generate structured findings or navigate evidence citations.

**Rationale**: ADR 0011 fixes the layout. B-86 assigns structured evidence to Ticket 8.

## Navigation protection

**Decision**: Register `beforeunload` for window close/reload and use one shared confirmation function for
meeting-details navigation actions touched by this feature. Keep native confirmation text.

**Rationale**: Native confirmation covers abrupt exit. A shared function prevents new workspace controls from
bypassing the save boundary.

**Alternatives considered**: Monkey-patching the Next router or document clicks is fragile. Saving every
keystroke changes the accepted save boundary.
