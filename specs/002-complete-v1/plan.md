# Implementation Plan: Complete Audio-First V1

**Branch**: `codex/complete-v1-waves` | **Date**: 2026-08-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-complete-v1/spec.md`

## Summary

Close the remaining B-86 tickets through one canonical Session snapshot and seven dependency-safe Production Waves. First prove or repair the existing rebrand, local transcription, diarization, and captured-origin alignment. Then add typed Apple Foundation Models generation, derive equivalent exports from the same snapshot, migrate optional providers to explicit Keychain-backed authorization, and qualify one immutable local release on the target Mac. A separately authorized private GitHub effect shares source only; each collaborator builds a local ad-hoc DMG from the exact commit and locked inputs.

```mermaid
flowchart LR
    A["Wave 1: baseline"] --> B["Wave 2: imported transcript"]
    B --> C["Wave 3: captured timeline"]
    C --> D["accepted principal transcript"]
    D --> E["Wave 4: grounded local findings"]
    E --> F["Wave 5: Markdown and JSON handoff"]
    F --> G["Wave 6: optional provider authorization"]
    G --> H["Wave 7: integrated release qualification"]
    H -. separate authorization .-> I["Private GitHub source collaboration"]
```

## Technical Context

**Language/Version**: Rust 1.97.1 edition 2021; TypeScript 5; Swift from the pinned Xcode toolchain; shell and Node.js 22 for deterministic build and qualification scripts

**Primary Dependencies**: Tauri 2.6.2, Next.js 14.2.35, React 18, SQLx 0.8 with SQLite, FluidAudio/`fluidaudio-rs` 0.14.1 for diarization only, existing Silero VAD and Parakeet/Whisper transcription, Apple Foundation Models through a bundled Swift helper, `security-framework` as a direct macOS Keychain dependency, installed Tauri dialog and filesystem plugins

**Storage**: SQLite is principal structured state; each Session folder owns immutable source media and mixed playback; Keychain owns provider secrets; generated exports are transient until explicit user save or share

**Testing**: Rust unit/integration tests with temporary SQLite and Session folders; Swift helper tests; Bun tests at frontend public seams; TypeScript typecheck; Next production build; deterministic synthetic audio corpus; native macOS UAT; network and log observation

**Target Platform**: Apple Silicon macOS. The main app keeps its existing deployment boundary; the Foundation Models helper checks framework and model availability at runtime and returns typed unavailability when the OS, device, locale, Apple Intelligence setting, or model readiness does not permit generation.

**Project Type**: Local desktop application with a Tauri/Rust core, Next.js renderer, and bundled local helpers

**Performance Goals**: Preserve the accepted 100 ms seek tolerance; record real-time factor, peak memory, energy, thermal behavior, startup, shutdown, and long-duration stability; keep UI review responsive for the accepted 10,000-block Session

**Constraints**: Local by default; no silent fallback; no video transfer; no secret or Session content in logs; source media immutable; jobs transactional and retry-safe; Finder save or macOS share is explicit; fixed Meeting, Interview, and Content schemas only

**Scale/Scope**: One private user, one target Mac, one local library, Sessions up to 10,000 transcript blocks, seven remaining B-86 tickets

## Constitution Check

### Before research

- **Authority stays visible — PASS**: B-86 is intake. `spec.md`, accepted ADRs, and this plan govern execution. The stale FluidAudio wording in B-86 must be reconciled with accepted ADR-0013 before Ticket 5 closes.
- **Smallest fitting path — PASS**: Reuse the current Tauri, SQLite, summary status, transcript identity, playback, dialog, HTTP, and UI components. Add only the missing canonical record, native helper, Keychain boundary, export serializer, and durable job/provenance state.
- **Observable value — PASS**: Each story has a public seam and a target-Mac receipt. Existing code earns closure through current evidence, not historical commit subjects.
- **Parallelize only independence — PASS**: Ticket 9 and Ticket 10 are serialized because both change Session actions, Tauri command registration, and the canonical snapshot consumer contract. No ticket-level parallel frontier remains.
- **Separate production, assurance, and external effects — PASS**: Production uses isolated sibling worktrees and one logical commit per task. The External Candidate follows Specify → Clarify → Plan → Tasks → Analyze → Implement → Converge, ordinary independent review, separate Security Review, and the fail-closed Release Gate. The tracked profile authorizes no external effect; repository creation, push, invitation, and package publication remain separate operator decisions.

### Twelve-Factor gate

ADR-0012 is the approved desktop exception for local durable state, no port, local process components, macOS logging, and in-release migrations. The remaining factors stay active. Wave 1 must pin unresolved branch dependencies and prove build/release/run separation. Wave 2 must make processing restart-safe. Wave 6 must move secrets from SQLite to Keychain and retain replaceable provider adapters. Wave 7 must trace each locally built package to a commit and fixed inputs, record the observed output hashes, and verify rollback. Cross-build byte equality is not an acceptance condition.

### After design

- **PASS**: One `VerifiableRecord` read model supplies generation, export, and provider preview. There is no second principal transcript or dual-write export store.
- **PASS**: Apple guided generation constrains shape; Rust still validates record type, transcript revision, evidence IDs, timestamps, and playable ranges before persistence.
- **PASS**: Markdown and JSON are pure views of one in-memory snapshot. Provider preview and send use a digest of that snapshot and cannot accept renderer-supplied payloads.
- **PASS**: Private corpus audio and secrets remain outside Git. Versioned manifests, hashes, thresholds, and non-sensitive measurements are durable.
- **PASS**: `.external-assurance.json` is the single Distribution Intent profile. It separates private source collaboration from local artifact handling and activates only universal controls.

## Project Structure

### Documentation

```text
.external-assurance.json
specs/002-complete-v1/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── agent-handoff.md
│   ├── native-generation.md
│   ├── provider-transfer.md
│   └── qualification.md
└── tasks.md
```

### Source Code

```text
frontend/
├── src/
│   ├── app/meeting-details/page-content.tsx
│   ├── app/settings/page.tsx
│   ├── components/MeetingDetails/
│   ├── hooks/meeting-details/
│   └── types/
├── tests/lib/
└── src-tauri/
    ├── migrations/
    └── src/
        ├── audio/
        ├── database/
        ├── verifiable_record/
        ├── agent_handoff.rs
        └── providers/

foundation-helper/
├── Package.swift
├── Sources/
└── Tests/

scripts/
├── bootstrap-dev.sh
├── prepare-sidecar.sh
├── qualify-audio.sh
└── release.sh
```

**Structure Decision**: Keep the current desktop application. Add one narrow Swift helper because Foundation Models is a native Swift framework. Keep all trust validation and durable state in Rust. Do not extend the legacy Python backend or the V2 llama sidecar. Keep distribution intent in the one root profile; do not add a release orchestrator.

## Complexity Tracking

No unapproved constitution violation.
