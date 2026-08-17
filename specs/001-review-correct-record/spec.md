# Feature Specification: Review and Correct Verifiable Records

**Feature Branch**: `codex/ticket-7-verifiable-record`

**Created**: 2026-08-13

**Status**: Approved intake; ready for planning

**Input**: B-86 Ticket 7 and the accepted Evidence desk workspace recorded in ADR 0011.

## User Scenarios & Testing

### User Story 1 - Correct the Principal Transcript (Priority: P1)

After processing a Session, the user reviews its transcript, gives participants meaningful names, corrects
transcript text or participant assignments, and saves the corrected result as the principal transcript.

**Why this priority**: The record is not verifiable or reusable until the user can correct processing errors
and persist the trusted result.

**Independent Test**: Open a processed Session, rename a participant, edit one block's text and participant,
save, close, and reopen the Session. The corrected values remain and the original Recording remains intact.

**Acceptance Scenarios**:

1. **Given** a processed Session with speaker clusters, **When** the user renames one participant, **Then** all
   transcript blocks assigned to that participant show the new name.
2. **Given** a transcript block, **When** the user changes its text or participant and saves, **Then** the saved
   values become the principal transcript state.
3. **Given** saved corrections, **When** the user closes and reopens the Session, **Then** every saved correction
   is restored without modifying source audio or claiming a revision history.
4. **Given** unsaved changes, **When** a save fails, **Then** the visible edits remain available for retry and
   the last saved principal transcript remains valid.

---

### User Story 2 - Verify a Passage Against Audio (Priority: P2)

The user selects a transcript timestamp and immediately hears the corresponding part of the mixed recording
with enough surrounding transcript context to judge the passage.

**Why this priority**: Timestamped playback turns corrected text into a verifiable record instead of an
unsupported document.

**Independent Test**: Open a completed Session, select a transcript timestamp, and confirm that it selects the
relevant transcript block and seeks the mixed track within the accepted 100 ms alignment tolerance.

**Acceptance Scenarios**:

1. **Given** a valid transcript timestamp, **When** the user activates it, **Then** playback seeks to the
   corresponding source position and the related block is selected in context.
2. **Given** missing or invalid audio timing, **When** the user activates the timestamp, **Then** the workspace
   explains that playback is unavailable and does not seek to an unrelated position.

---

### User Story 3 - Recover from Editing Mistakes (Priority: P3)

During one editing session, the user reverses and reapplies transcript text, participant assignment, and
participant-name changes without losing the saved record.

**Why this priority**: Safe correction requires a fast local recovery path before the user commits changes.

**Independent Test**: Make each supported edit type, undo to the initial session state, redo to the latest
state, save, and confirm that undo history resets at the documented session boundary.

**Acceptance Scenarios**:

1. **Given** one or more unsaved edits, **When** the user activates undo or redo, **Then** exactly one edit is
   reversed or reapplied and the displayed transcript remains internally consistent.
2. **Given** no available undo or redo action, **When** the user reaches that boundary, **Then** the unavailable
   action is disabled and the record does not change.
3. **Given** keyboard-only operation, **When** the user reviews, edits, saves, undoes, redoes, and seeks a
   timestamp, **Then** focus remains visible and every control has an accessible label and non-color state.

### Edge Cases

- A participant rename is blank or contains only whitespace.
- Two participants are given the same display name.
- A transcript block is empty after editing.
- A participant referenced by a block is absent or deleted by malformed legacy data.
- Concurrent save attempts complete out of order.
- A Session closes with unsaved changes.
- The mixed track is missing, unreadable, or shorter than a stored timestamp.
- The transcript contains overlapping origins or ambiguous duplicated passages.
- A long transcript uses virtualized rendering and the selected block is outside the current viewport.
- Editing or playback is attempted while processing is incomplete, failed, or unavailable.

## Requirements

### Functional Requirements

- **FR-001**: The workspace MUST preserve the accepted Evidence desk structure: summary and participants in
  the left context column, timestamped transcript and mixed-track player at the center, and evidence in the
  right verification column.
- **FR-002**: The user MUST be able to rename a participant, and the workspace MUST apply that name to all
  transcript blocks assigned to the participant.
- **FR-003**: The user MUST be able to change transcript block text and participant assignment.
- **FR-004**: Saving MUST atomically establish the displayed corrected state as the principal transcript.
- **FR-005**: Closing and reopening a Session MUST restore the last saved participant and transcript state.
- **FR-006**: V1 MUST NOT claim or expose a durable revision tree; undo and redo apply only to the current
  editing session.
- **FR-007**: Undo and redo MUST cover participant names, transcript text, and participant assignments in the
  order the user changed them.
- **FR-008**: Activating a valid transcript timestamp MUST select the corresponding transcript block and seek
  the mixed track to its stored position.
- **FR-009**: Timestamp seeking MUST remain within the accepted 100 ms alignment tolerance established by
  Ticket 6.
- **FR-010**: Save or playback failure MUST leave source origins and the last valid saved record unchanged and
  MUST present an actionable state.
- **FR-011**: The complete workflow MUST support keyboard navigation, visible focus, accessible labels, and
  state indicators that do not depend on color alone.
- **FR-012**: The workspace MUST preserve source-origin, speaker-cluster, stable-identifier, and timing
  provenance when the user edits principal transcript fields.
- **FR-013**: Empty, processing, model-unavailable, failed, and completed Session states MUST remain explicit
  and usable within the accepted workspace structure.
- **FR-014**: The feature MUST make no external request and MUST not add Session content to diagnostics.

### Key Entities

- **Session**: The user-facing record that owns a Recording, principal transcript, participants, findings,
  processing state, and Session type.
- **Participant**: A stable identity and editable display name associated with one or more speaker clusters or
  the configured local microphone participant.
- **Transcript Block**: A stable, timed passage with editable text and participant assignment plus immutable
  origin and timing provenance.
- **Edit Session**: The in-memory ordered history of unsaved changes available for undo and redo until the
  workspace closes or establishes a new session boundary.

## Success Criteria

### Measurable Outcomes

- **SC-001**: A user can rename a participant, correct one transcript block, save, close, and verify the
  restored result in under two minutes without Terminal use.
- **SC-002**: Automated reopen checks preserve 100% of saved participant names, transcript text, assignments,
  stable identifiers, origins, and timing data across all normal and failure fixtures.
- **SC-003**: Every valid transcript timestamp tested seeks the mixed track within 100 ms and selects the
  correct transcript context.
- **SC-004**: Undo and redo reproduce the expected state after every supported edit in the normal, boundary,
  and interleaved-edit case table.
- **SC-005**: Keyboard-only acceptance completes the full review workflow with no inaccessible control,
  invisible focus state, or color-only status.
- **SC-006**: Forced save and playback failures cause zero source-audio changes, zero corruption of the last
  saved principal transcript, and zero external requests.

## Assumptions

- Ticket 6's stable transcript identifiers, source provenance, and mixed-track alignment are available.
- The accepted Evidence desk layout in ADR 0011 remains the product decision; this feature implements it
  without another prototype cycle.
- Participant deletion, transcript block insertion or deletion, collaborative editing, and durable revision
  history remain outside V1.
- Ticket 7 renders the evidence column's explicit unavailable, pending, failed, or ready state. Ticket 8 owns
  structured findings, evidence citations, and evidence-to-transcript navigation.
- Saving explicit empty transcript text is allowed; the UI warns but does not invent replacement text.
- Duplicate participant display names are allowed because identity is stable and separate from display name.
- Unsaved edits require confirmation before the user leaves the workspace.
