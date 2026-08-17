# Feature Specification: Complete Audio-First V1

**Feature Branch**: `codex/v1-wave7-release`

**Created**: 2026-08-14

**Status**: Accepted for revised planning

**Input**: User description: "Use the current system and Code Lane. Put the remaining tickets into waves and execute them by wave."

**Source objective**: B-86 in the private agent-governance repository, remaining Tickets 1, 5, 6, 8, 9, 10, and 11. Ticket 7 is accepted and remains the principal review workspace.

## Clarifications

### Session 2026-08-16

- Coverage scan found no unresolved material ambiguity after the operator decisions.
- Source access is private collaboration through GitHub. Artifact access remains local.
- The collaborator builds the local ad-hoc DMG from source. No prebuilt DMG is shared in this feature.
- Reproducibility means a verified clean build from fixed inputs with exact provenance and output hashes. It does not require byte-identical executables across builds.
- Any repository creation, push, invitation, package upload, or later publication remains a separate effect and requires separate authority.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reproduce the private local baseline (Priority: P1)

As the maintainer, I can reproduce, inspect, and release the rebranded application from its recorded upstream baseline without importing proprietary code or weakening later privacy guarantees.

**Why this priority**: Every remaining workflow depends on a known, licensed, reproducible application baseline.

**Independent Test**: A clean checkout on the target Mac can follow the recorded setup, build, test, package, and release commands and can resolve every shipped code or model artifact to its recorded source and license.

**Acceptance Scenarios**:

1. **Given** a clean checkout and the documented prerequisites, **When** the baseline commands run, **Then** dependencies install and the declared build, test, lint, packaging, and release gates complete with recorded results.
2. **Given** all user-facing surfaces and distributed artifacts, **When** branding and license inventories are inspected, **Then** only gcrdings branding appears, required attribution remains, and no Meetily Pro artifact is present.

---

### User Story 2 - Qualify imported-audio transcription (Priority: P2)

As the user, I can process a completed imported Recording locally into a timestamped multilingual transcript with speaker and overlap information, while the original remains intact and failures remain retry-safe.

**Why this priority**: This establishes the one accepted audio-intelligence path used by both imports and live capture.

**Independent Test**: The versioned private corpus exercises Portuguese, English, code-switching, noise, overlap, long audio, malformed output, and model failure; the resulting receipt records quality, performance, provenance, and retry behavior without network traffic.

**Acceptance Scenarios**:

1. **Given** a supported completed import, **When** local processing finishes, **Then** each transcript block has a stable identity, valid time range, source origin, speaker cluster, optional language metadata, and visible overlap state.
2. **Given** the qualification corpus, **When** the pinned configuration runs on the target Mac, **Then** the measured quality and resource results meet documented thresholds.
3. **Given** model failure or malformed output, **When** processing stops, **Then** the Recording remains intact and the user can retry without duplicate or corrupt transcript state.

---

### User Story 3 - Align captured origins (Priority: P3)

As the user, I can process microphone and system-audio origins independently and review one ordered timeline that preserves provenance and seeks the mixed playback track accurately.

**Why this priority**: Live capture must reach the same verifiable record state as imported audio before generated findings can be trusted.

**Independent Test**: A captured two-origin fixture covers speech, silence, overlap, duplicate or ambiguous passages, retry, and exact seeking while byte checks prove that processing did not modify source origins.

**Acceptance Scenarios**:

1. **Given** completed microphone and system origins, **When** processing finishes, **Then** both origins remain identifiable in one ordered timeline and no ambiguous passage disappears without decision metadata.
2. **Given** individual-microphone mode, **When** its blocks enter the timeline, **Then** they use the configured local participant without unnecessary speaker discovery.
3. **Given** any valid timeline block, **When** the user activates its timestamp, **Then** mixed playback seeks within the accepted alignment tolerance.

---

### User Story 4 - Generate grounded local findings (Priority: P4)

As the user, I can select Meeting, Interview, or Content and generate a purpose-specific local record whose grounded items resolve to current transcript evidence, while model unavailability leaves the Session usable and pending.

**Why this priority**: Evidence-linked structured findings are the principal product differentiation and the source for both export paths.

**Independent Test**: Each fixed record type accepts the same versioned Session input and returns schema-valid grounded output, explicit unavailability, or a typed failure; every evidence reference resolves and seeks, and network observation remains empty.

**Acceptance Scenarios**:

1. **Given** a corrected Session, **When** the user generates a fixed record type, **Then** only fields valid for that type appear and every grounded item links to current playable evidence.
2. **Given** an unavailable or failed local model, **When** generation returns, **Then** summary remains pending, the rest of the Session remains usable, and no external request occurs.
3. **Given** stale, missing, or invalid evidence references, **When** output validation runs, **Then** the result is rejected as a typed failure and is not presented as grounded.

---

### User Story 5 - Export the current Agent Handoff (Priority: P5)

As the user, I can explicitly save or share one self-contained Markdown handoff and an equivalent JSON record built from the current corrected Session.

**Why this priority**: The handoff is the portable output consumed by another assistant or program.

**Independent Test**: Contract fixtures for all three record types prove semantic equivalence, complete corrected transcript inclusion, current-state rebuilding, explicit save or share, field exclusion, and cleanup after failure.

**Acceptance Scenarios**:

1. **Given** a corrected Session with current findings, **When** both formats are generated, **Then** they contain equivalent Session facts, participants, findings, evidence, and complete corrected transcript.
2. **Given** older export artifacts, **When** the user exports again, **Then** both files are rebuilt from current principal state and do not reuse stale content.
3. **Given** cancellation or write failure, **When** export stops, **Then** the Session is unchanged, nothing leaves the library, and no incomplete temporary artifact remains.

---

### User Story 6 - Authorize an optional provider (Priority: P6)

As the user, I can store, test, enable, replace, or remove a provider credential and explicitly confirm each named transfer after seeing the provider, Session, purpose, and exact data type.

**Why this priority**: Optional cloud work must remain deliberate, minimal, and separate from the default local path. It follows the canonical handoff contract so preview and send use one fixed Session projection.

**Independent Test**: A provider test double proves credential lifecycle, per-task enablement, exact confirmation, cancellation, payload minimization, provenance, retry behavior, and the absence of fallback or duplicate transmission.

**Acceptance Scenarios**:

1. **Given** a saved credential, **When** Settings reopens, **Then** the secret is not displayed in full and is absent from logs and exports.
2. **Given** a disabled task or cancelled confirmation, **When** the user attempts a transfer, **Then** no request is sent.
3. **Given** an enabled task and accepted confirmation, **When** the request runs, **Then** it sends only the displayed data, never video, and records provider and task provenance separately from local output.

---

### User Story 7 - Qualify and accept the complete V1 (Priority: P7)

As the user, I can install and operate the complete audio-first workflow on the target Apple Silicon Mac without Terminal use, silent network access, private log content, or exposed V2 controls.

**Why this priority**: Release follows only after the integrated normal, boundary, failure, concurrency, privacy, performance, and rollback cases pass.

**Independent Test**: The final release receipt traces one candidate commit through live capture and import, correction, evidence playback, local findings, optional-provider controls, both exports, packaging, install, rollback, and explicit user acceptance.

**Acceptance Scenarios**:

1. **Given** the target Mac and a release candidate, **When** normal live-capture and import seams run, **Then** both reach a corrected, grounded, exportable record without Terminal use.
2. **Given** each declared boundary and failure case, **When** it occurs, **Then** the specified safe state is visible, retryable where applicable, and data remains intact.
3. **Given** the complete local workflow, **When** network and diagnostic observations are inspected, **Then** no unapproved transfer or Session content appears.
4. **Given** an authorized private collaborator, **When** source access is granted, **Then** the collaborator can build a local ad-hoc DMG from the exact source commit and locked inputs without receiving a prebuilt DMG.

### Edge Cases

- The local model is unavailable, returns malformed data, times out, or returns evidence for a superseded transcript revision.
- One captured origin is silent, missing, lost during capture, duplicated in alignment, or ambiguous against another origin.
- Processing is started twice, retried after interruption, or competes with a save, generation, or export operation.
- The disk becomes full or unwritable during processing, generation, or export.
- A provider credential is missing, replaced during a request, rejected, or removed after task enablement.
- The user cancels before provider transmission, Finder save, macOS share, or overwrite confirmation.
- The Session contains zero blocks, long-form audio, overlapping speech, code-switching, unsafe paths, or stale generated artifacts.
- Logs, errors, crash reports, temporary files, and package contents are inspected for private Session data and secrets.
- A source repository exists while no package is published, or a later request expands one channel without authorizing another.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The product MUST retain one recorded, reproducible, licensed upstream baseline and one verified command set for later changes.
- **FR-002**: The product MUST process completed imported audio locally through one qualified audio-intelligence path without network access.
- **FR-003**: Every transcript block MUST retain stable identity, valid timing, source provenance, speaker identity or cluster, and explicit overlap information when applicable.
- **FR-004**: Qualification MUST record Portuguese, English, code-switching, noise, overlap, long-audio, quality, runtime, memory, repeatability, and failure results on the target Mac.
- **FR-005**: Processing failure or retry MUST preserve source Recordings and MUST NOT create duplicate or corrupt principal transcript state.
- **FR-006**: Captured origins MUST be processed independently before alignment into one ordered provenance-preserving timeline.
- **FR-007**: Timeline timestamps MUST seek the mixed playback track within the measured and accepted tolerance.
- **FR-008**: The product MUST generate exactly Meeting, Interview, and Content local record types with purpose-specific structured fields.
- **FR-009**: Every item presented as grounded MUST resolve to the current principal transcript and a playable timestamp.
- **FR-010**: Local model unavailability or failure MUST leave summary pending, leave the Session usable, and MUST NOT trigger an external request.
- **FR-011**: Optional-provider credentials MUST remain protected, masked after saving, removable, and absent from logs and exports.
- **FR-013**: Provider cancellation MUST send nothing; provider failure MUST remain visible and retryable without fallback, substitution, or duplicate transmission.
- **FR-014**: Markdown and JSON exports MUST represent equivalent current Session facts, findings, evidence, and complete corrected transcript.
- **FR-015**: Export MUST occur only after explicit Finder save or macOS share action and MUST exclude media, credentials, video, and undeclared fields.
- **FR-016**: Export failure MUST preserve the Session and remove incomplete temporary artifacts.
- **FR-017**: The final release MUST pass recorded normal, boundary, failure, concurrency, privacy, performance, packaging, install, and rollback cases on the target Mac.
- **FR-018**: The default local workflow MUST make no external request and operational diagnostics MUST omit Session content and secrets.
- **FR-019**: Each production task MUST be independently checked, committed once, integrated by dependency order, reviewed read-only at the exact candidate, and recorded in execution evidence.
- **FR-020**: The tracked Distribution Intent profile MUST classify this feature as private source collaboration, local artifact access, collaborator build authority, private GitHub source channel, macOS platform, and no update mechanism.
- **FR-021**: An absent or unconfirmed package-publication channel MUST keep every DMG local and MUST NOT infer artifact publication from source collaboration.
- **FR-022**: The local ad-hoc build MUST use the exact source commit, locked dependencies, pinned toolchain inputs, a closed package allowlist, legal and provenance artifacts, private-path scans, and observed output hashes; it MUST NOT require byte-identical executables across independent machines or builds.
- **FR-023**: Every External Candidate MUST complete Specify, Clarify, Plan, Tasks, Analyze, Implement, Converge, ordinary independent review, separate Security Review, and the fail-closed Release Gate before any external effect.
- **FR-024**: Repository creation, source push, collaborator invitation, package upload, and publication MUST each require separate operator authority and channel-specific evidence.

### Key Entities

- **Recording**: An immutable completed input with one or more synchronized audio origins and a mixed playback track.
- **Source Origin**: A microphone, system-audio, or imported-media audio source with provenance and timing.
- **Transcript Block**: A stable timed passage with principal text, participant assignment, source provenance, language metadata, and overlap state.
- **Session**: The aggregate that owns the Recording, principal transcript, selected record type, generation state, findings, and export state.
- **Structured Finding**: A type-specific item with current evidence references and provenance.
- **Evidence Reference**: A link from a finding to a current transcript block and playable source time.
- **Provider Authorization**: A named provider, protected credential reference, enabled task, confirmation facts, and transfer provenance.
- **Agent Handoff**: Equivalent Markdown and JSON representations rebuilt from current Session state.
- **Release Receipt**: The exact candidate, dependencies, model artifacts, checks, observed results, notices, package, and rollback evidence.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A clean target-Mac setup completes every recorded baseline command with zero undocumented manual repair steps.
- **SC-002**: All required corpus categories have versioned results, declared thresholds, and a passing selected configuration before captured-origin processing is accepted.
- **SC-003**: 100% of accepted transcript blocks have valid identity, timing, provenance, speaker assignment, and applicable overlap state.
- **SC-004**: 100% of grounded findings resolve to the current transcript and seek playable evidence; unresolved references are never presented as grounded.
- **SC-005**: Model unavailability and every provider cancellation produce zero external requests while leaving the Session usable.
- **SC-006**: Markdown and JSON contract checks report zero semantic differences for Meeting, Interview, and Content fixtures.
- **SC-007**: Every declared normal, boundary, failure, and concurrency row has an observed result and no unresolved release-blocking failure.
- **SC-008**: Network observation finds zero unapproved requests, and privacy scans find zero Session content, credentials, prompts, or provider payloads in operational diagnostics.
- **SC-009**: The packaged release installs, opens, completes both primary workflows, and follows its verified rollback path on the target Mac.
- **SC-010**: The user accepts the final workflow without Terminal use and without incomplete V2 controls.
- **SC-011**: The Distribution Intent profile selects exactly the universal private-collaboration controls and no inactive public, registry, store, hosted-service, or distributed-artifact controls.
- **SC-012**: Two clean local builds may have different package hashes, but each build passes the closed verifier and records its exact source, toolchain, dependency, package-member, legal, provenance, and output identities.

## Assumptions

- The accepted B-86 product brief, architecture decisions, Ticket 2 prototype, and Ticket 7 workspace remain governing context.
- Existing Ticket 1, 5, and 6 code is evidence to inspect, not proof of acceptance. Work already satisfying a criterion will receive a reproducible receipt instead of a rewrite.
- Apple Silicon macOS is the only V1 release platform.
- Private corpus media and model artifacts remain outside Git; versioned manifests and non-sensitive measurements may be committed.
- Apple Foundation Models is the only local V1 summary engine. External providers remain optional and inactive by default.
- Ticket 9 and Ticket 10 may share one execution wave only after path, state, credential, migration, and external-effect independence is proven in `tasks.md`; otherwise they execute serially.
- The intended source repository is `gcunharodrigues/gcrdings`, but this feature does not authorize its creation, push, collaborator invitation, or any package publication.
