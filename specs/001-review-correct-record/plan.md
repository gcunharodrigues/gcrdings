# Implementation Plan: Review and Correct Verifiable Records

**Branch**: `codex/ticket-7-verifiable-record` | **Date**: 2026-08-13 | **Spec**: [spec.md](./spec.md)

**Input**: `specs/001-review-correct-record/spec.md`

## Summary

Replace the inherited read-only meeting-details split view with the accepted Evidence desk. Add one atomic,
version-checked Session-record save command for participant names and transcript corrections. Keep undo and
redo in frontend memory. Resolve mixed-track playback from the stored Session folder inside Rust. Reuse the
existing paginated transcript loader, SQLite pool, audio player hook, virtualizer, and UI primitives.

## Technical Context

**Language/Version**: Rust 2021 (`rust-version = 1.77`); TypeScript 5.7; React 18; Next.js 14

**Primary Dependencies**: Tauri 2.6, SQLx 0.8/SQLite, Tokio, React, `@tanstack/react-virtual`, existing Radix UI primitives

**Storage**: Existing SQLite database for the principal record and participant identity; existing Session folder for immutable origins and mixed `audio.*`

**Testing**: Rust unit/integration tests, Bun frontend seam tests, TypeScript compile, Next production build

**Target Platform**: Apple Silicon macOS 26+ Tauri desktop application

**Project Type**: Local-first desktop application with Rust native backend and static Next.js frontend

**Performance Goals**: Save 10,000 transcript blocks atomically in under 2 seconds on the target Mac; navigate a 10,000-block Session without decoding the complete audio more than once

**Constraints**: Offline-only; timestamp seek within 100 ms; no source-origin mutation; no transcript content in logs; renderer input cannot escape the stored Session folder; no V1 durable revision tree

**Scale/Scope**: One local user, one open Session, up to 10,000 transcript blocks, ordinary meeting participant counts; three-column Evidence desk plus compact-width fallback

## Constitution Check

*GATE: Passed before Phase 0 and after Phase 1.*

- **Authority stays visible**: B-86 Ticket 7 and ADR 0011 define scope. Ticket 8 evidence generation remains excluded.
- **Smallest fitting path**: Reuse SQLite, pagination, audio playback, virtualization, and UI primitives. Add no dependency or generic editor framework.
- **Observable value**: Public seams are one atomic save contract, one trusted mixed-track resolver, and the keyboard review workflow.
- **Parallelize only independence**: Backend persistence/playback and frontend edit-state logic can start independently. Workspace integration waits for both.
- **Separate production and assurance**: Production stays in the isolated worktree. One integrated candidate gets deterministic gates and one independent read-only review.

Twelve-Factor review: no exception. Dependencies stay pinned. Configuration and secrets do not change. SQLite
remains the explicit backing service. Build/release/run separation stays intact. Writes are transactional and
retry-safe. Logs contain metadata, not Session content. Migrations remain versioned.

## Project Structure

### Documentation

```text
specs/001-review-correct-record/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/session-record.md
└── tasks.md
```

### Source Code

```text
frontend/
├── src/
│   ├── app/meeting-details/
│   ├── components/MeetingDetails/
│   ├── hooks/meeting-details/
│   ├── lib/
│   ├── services/
│   └── types/
├── src-tauri/
│   ├── migrations/
│   └── src/
│       ├── api/
│       ├── database/
│       └── lib.rs
└── tests/lib/
```

**Structure Decision**: Extend the meeting-details vertical slice. Keep durable invariants in Rust and SQLite.
Keep ephemeral editing history in one frontend reducer. Keep display components shallow.
