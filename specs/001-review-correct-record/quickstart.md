# Quickstart: Validate Review and Correction

## Prerequisites

Use the target Apple Silicon Mac and `docs/BUILDING.md`. Prepare one captured Session and one imported Session.
Each needs at least two participants and timed transcript blocks.

## Automated checks

```bash
cargo test --workspace --locked
bun test frontend/tests/lib
pnpm --dir frontend exec tsc --noEmit
pnpm --dir frontend build
```

Expected: all applicable checks pass. Repository tests prove atomic commit, rollback, conflict, paginated
partial update, ownership, and provenance. Frontend tests prove edit, undo, redo, dirty, save-success,
save-failure, and Session-reset transitions.

## 2026-08-13 automated receipt

- `cargo test --workspace --locked`: 233 passed, 2 hardware tests ignored, 0 failed.
- `bun test frontend/tests/lib`: 53 passed, 0 failed.
- `pnpm --dir frontend exec tsc --noEmit`: passed.
- `pnpm --dir frontend build`: passed; 11 of 11 static pages generated.
- `git diff --check`: passed.
- Trusted-audio tests cover missing, empty, non-file, and symlink-escaped mixed tracks. The legacy
  renderer-selected `read_audio_file(filePath)` command is removed.

## 2026-08-14 native target-Mac receipt

- A uniquely identified, unsigned QA bundle used an isolated SQLite store. The normal application store was
  not modified.
- Captured and imported fixtures each loaded three timed passages and two participants. Rename, exact text
  correction, participant reassignment, undo, redo, save, reopen, and sparse SQLite persistence passed.
- SQLite inspection confirmed revision increments and preserved origin, speaker cluster, identifiers, and
  timing fields. No transcript JSON dual-write occurred.
- A 3.0-second timestamp started playback at the requested passage; the player reported 3.3 seconds after
  approximately 0.3 seconds of elapsed playback. Pure seek tests cover the exact value and invalid bounds.
- Forced revision conflict and SQLite write lock preserved the draft. Conflict and storage messages were
  readable, reload worked, and retry saved successfully.
- A 10,000-passage Session loaded in pages and saved exactly one changed row. An external audio symlink was
  rejected as unsafe.
- Keyboard traversal, visible focus, labels, disabled boundaries, shortcuts, non-color errors, compact layout,
  and the accepted three-column layout passed.
- The active-process network snapshot contained zero sockets. Unified-log and application-log scans found zero
  fixture text or Session-path matches.
- Real microphone capture reached `storing` and produced separate `microphone.mp4` and mixed `audio.mp4` files,
  each 1,177,308 bytes. System audio was unavailable and the UI reported that the remaining microphone source
  was preserved. Transcription was unavailable because the isolated QA model had been removed; Ticket 7 assumes
  Ticket 6 supplies a completed timestamped transcript.
- Computer Use auto-accepted `window.confirm`, so native selection of Cancel was not observable. The shared
  dirty-navigation contract test passed, discard was observed, and the operator accepted this automation
  limitation on 2026-08-14.

The QA app was stopped. The real-microphone test recording was moved to Trash, its isolated database row was
removed, and the source repository remained clean.

## User acceptance

1. Run `pnpm --dir frontend tauri:dev`.
2. Open a completed Session. Confirm the accepted three-column Evidence desk.
3. Rename a participant. Change one block's text and participant. Undo and redo each change.
4. Save. Close and reopen. Confirm corrected values and provenance remain.
5. Activate a timestamp. Confirm selection and playback seek within 100 ms.
6. Repeat with keyboard only. Confirm focus, labels, disabled states, and non-color feedback.
7. Make an edit and try each meeting-details exit. Cancel once, then discard.
8. Force stale-version and storage failures. Confirm retryable errors and unchanged saved state.
9. Replace mixed audio with an external symlink. Confirm playback is rejected.
10. Observe network and logs. Confirm no external request and no Session text or path in diagnostics.
