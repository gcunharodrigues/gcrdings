# Tasks: Review and Correct Verifiable Records

**Input**: [spec.md](./spec.md), [plan.md](./plan.md), [research.md](./research.md),
[data-model.md](./data-model.md), [native contract](./contracts/session-record.md)

**Backlog**: B-86 Ticket 7

**Format**: `[ID] [P?] [Story?] Result — exact paths — independent check`

## Phase 1: Executable Baseline

- [x] T001 Pin and verify official Spec Kit `v0.16.2` for this repository — `.agents/skills/speckit-*/**`, `.specify/**` — every managed file matches its integration-manifest SHA-256.
- [x] T002 Record the executable Ticket 7 specification, design, contracts, and Waves — `specs/001-review-correct-record/**` — Spec Kit prerequisite scripts resolve the feature; checklist is green; no placeholder or clarification remains; `git diff --check` passes.

## Phase 2: User Story 1 — Correct the Principal Transcript (P1, MVP)

**Goal**: Rename participants, correct loaded transcript blocks, assign participants, save atomically, and
restore the corrected principal record after reopen.

**Independent Test**: Load a Session, edit name/text/assignment, save, reopen, and compare editable values plus
immutable provenance. Force stale revision and invalid-ID failures; confirm zero partial writes.

- [x] T003 [P] [US1] Define red backend principal-record contract tests for participant migration, paginated patch save, atomic rollback, revision conflict, ownership, reopen, and provenance preservation — `frontend/src-tauri/src/database/repositories/principal_record.rs`, `frontend/src-tauri/migrations/20260813000000_add_principal_record.sql` — focused `cargo test` fails only because the repository contract is not implemented.
- [x] T004 [P] [US1] Define red pure reducer tests for rename, text, assignment, merged pagination, dirty state, undo/redo boundaries, save success/failure, duplicate names, blank names, and empty text — `frontend/tests/lib/review-record.test.ts`, `frontend/src/lib/review-record.ts` — focused Bun test fails only on missing reducer behavior.
- [x] T005 [US1] Implement participant identity, revision-CAS patch persistence, typed Tauri get/save commands, and paginated `participant_id` projection without exposing editable content in logs — `frontend/src-tauri/migrations/20260813000000_add_principal_record.sql`, `frontend/src-tauri/src/database/models.rs`, `frontend/src-tauri/src/database/repositories/{mod.rs,meeting.rs,principal_record.rs}`, `frontend/src-tauri/src/api/api.rs`, `frontend/src-tauri/src/lib.rs`, `frontend/src/types/index.ts` — T003 focused Rust tests pass; existing meeting/transcript repository tests pass.
- [x] T006 [US1] Implement the pure edit reducer and Session hook with sparse patches, synchronous double-save guard, save retry, Session reset, keyboard shortcuts, and `beforeunload` dirty protection — `frontend/src/lib/review-record.ts`, `frontend/src/hooks/meeting-details/useReviewRecord.ts`, `frontend/tests/lib/review-record.test.ts` — T004 focused Bun tests pass.
- [x] T007 [US1] Deliver the accepted editable Evidence desk shell and shared internal-navigation guard — `frontend/src/app/meeting-details/{page.tsx,page-content.tsx}`, `frontend/src/components/MeetingDetails/{TranscriptPanel.tsx,SummaryPanel.tsx,ParticipantsPanel.tsx,EvidenceStatusPanel.tsx}`, `frontend/src/components/Sidebar/{SidebarProvider.tsx,index.tsx}`, `frontend/src/hooks/usePaginatedTranscripts.ts`, `frontend/tests/lib/review-workspace.test.ts` — production build and TypeScript pass; focused structural/contract test proves three columns, editable exact transcript text, participant resolution for paginated blocks, explicit evidence states, guarded navigation, and no use of the live transcript stop-word cleaner.

## Phase 3: User Story 2 — Verify a Passage Against Audio (P2)

**Goal**: Select any loaded transcript timestamp and hear the exact corresponding mixed-track passage.

**Independent Test**: Activate valid, missing, negative, non-finite, and beyond-duration timestamps. Only the
valid value selects, scrolls, seeks exactly, and starts playback.

- [x] T008 [US2] Define red trusted-audio and pure seek-validation tests — `frontend/src-tauri/src/api/api.rs`, `frontend/tests/lib/session-audio.test.ts`, `frontend/src/lib/session-audio.ts` — focused Rust and Bun tests fail only on missing trusted resolution and seek rules.
- [x] T009 [US2] Resolve mixed audio by `meeting_id` inside Rust and reject missing, empty, non-file, or symlink-escaped targets — `frontend/src-tauri/src/api/api.rs`, `frontend/src-tauri/src/lib.rs` — T008 Rust tests pass and the renderer cannot supply a filesystem path.
- [x] T010 [US2] Implement race-safe Session audio state, accessible player controls, exact timestamp buttons, selection, and virtualizer scrolling — `frontend/src/lib/session-audio.ts`, `frontend/src/hooks/useAudioPlayer.ts`, `frontend/src/components/AudioPlayer.tsx`, `frontend/src/components/MeetingDetails/TranscriptPanel.tsx`, `frontend/tests/lib/session-audio.test.ts` — T008 Bun tests, TypeScript, and build pass; invalid timing produces an alert and zero seek.

## Phase 4: User Story 3 — Recover from Editing Mistakes (P3)

**Goal**: Complete keyboard-only undo, redo, save, review, and safe-leave behavior.

**Independent Test**: Use Tab/Shift+Tab, Enter/Space, Cmd+Z, Cmd+Shift+Z, and Cmd+S through the full workflow;
confirm visible focus, accessible labels, disabled boundaries, and non-color errors/status.

- [x] T011 [US3] Close accessibility and unsafe-operation boundaries, including draft confirmation before retranscription or summary regeneration — `frontend/src/app/meeting-details/page-content.tsx`, `frontend/src/components/MeetingDetails/{TranscriptPanel.tsx,ParticipantsPanel.tsx,SummaryPanel.tsx,TranscriptButtonGroup.tsx}`, `frontend/src/components/Sidebar/{SidebarProvider.tsx,index.tsx}`, `frontend/tests/lib/review-workspace.test.ts` — focused test and keyboard acceptance case pass.

## Phase 5: Integrated Verification and Assurance

- [x] T012 Run the full applicable gates and record immutable task evidence — `specs/001-review-correct-record/{quickstart.md,tasks.md}` — automated gates passed on 2026-08-13 and 2026-08-14; native target-Mac acceptance covered captured and imported Sessions, persistence, exact playback, forced conflict/storage/playback failures, 10,000 blocks, logs, network, keyboard accessibility, and real microphone capture. The native automation auto-accepts JavaScript confirmation dialogs, so the shared Cancel branch remained deterministic-test evidence; the operator accepted that automation limitation on 2026-08-14.
- [x] T013 Obtain one independent read-only review of the exact integrated candidate and land only if it passes — `specs/001-review-correct-record/tasks.md`, B-86 Ticket 7 pointer/status — the integrated candidate and the structured-error follow-up each received an independent read-only `PASS`; `main` landed at `2ad47b2` without overwriting concurrent work. B-86 remains active for Tickets 5, 6, and 8–11; only Ticket 7 is closed.

## Task Dependencies

| Task | Depends on | Reason |
|---|---|---|
| T001 | — | Pins the lane foundation. |
| T002 | T001 | Makes requirements and Waves executable. |
| T003 | T002 | Defines backend red seam. |
| T004 | T002 | Defines frontend red seam on disjoint files. |
| T005 | T003 | Implements the backend contract. |
| T006 | T004 | Implements the frontend state contract. |
| T007 | T005, T006 | Integrates durable and ephemeral state into the workspace. |
| T008 | T007 | Defines playback against the integrated transcript selection seam. |
| T009 | T008 | Registers the trusted native audio contract. |
| T010 | T009 | Consumes the native contract in the workspace. |
| T011 | T010 | Closes cross-cutting workflow boundaries after all controls exist. |
| T012 | T011 | Verifies the closed integrated candidate. |
| T013 | T012 | Assurance begins only after deterministic closure. |

## Execution Waves

| Wave | Tasks | Entry condition |
|---|---|---|
| 1 | T001 | Clean isolated worktree at fixed base. |
| 2 | T002 | Spec Kit foundation verified. |
| 3 | T003 [P], T004 [P] | Plan accepted; files and checks are disjoint. |
| 4 | T005 [P], T006 [P] | Each task's own red seam is recorded; files remain disjoint. |
| 5 | T007 | Backend and reducer contracts pass independently. |
| 6 | T008 | Editable workspace selection seam exists. |
| 7 | T009 | Trusted-audio red seam recorded. |
| 8 | T010 | Native audio command passes. |
| 9 | T011 | All workflow controls exist. |
| 10 | T012 | Production is closed to writes. |
| 11 | T013 | Integrated gates are green. |

Same-Wave tasks share no file, mutable state, or check output. T005 and T006 can each pass at its own commit
without the sibling. T009 serializes after T008 because both touch `api.rs`; T010 serializes after T007 because
both touch `TranscriptPanel.tsx`.

## Execution Evidence

| Task | Commit subject | Local check | Observed result |
|---|---|---|---|
| T001 | `chore(spec-kit): pin v0.16.2 code lane` | Integration manifest SHA-256 validation | `codex` and `speckit` manifests match; commit `745054f`. |
| T002 | `docs(ticket-7): define review workspace waves` | Spec Kit prerequisite, checklist, placeholder, task-format, and diff checks | Feature resolves; 16/16 checklist items pass; 13 tasks and 11 Waves validate. |
| T012 | `docs(ticket-7): close native acceptance` | Rust, Bun, TypeScript, production build, native target-Mac UAT, SQLite inspection, log/network scan | Rust 235 passed/2 hardware ignored; Bun 55 passed; TypeScript and 11/11-page build passed; commit `899ae85` records the native receipt in `quickstart.md`. |
| T013 | `fix(ticket-7): render structured save errors` | Independent read-only review of `3a49918..2ad47b2` after the integrated candidate review | Both reviews returned `PASS`; commit `2ad47b2` is on `main`. |

## Implementation Strategy

MVP is T001–T007. It delivers correction, persistence, reopen, and safe leave. T008–T010 add timestamp
verification. T011 closes the full keyboard and unsafe-operation boundary. No custom agent, dependency,
revision tree, evidence generator, or generic editor abstraction is added.
