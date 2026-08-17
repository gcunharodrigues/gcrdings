# Research: Complete Audio-First V1

## Decision 1: Treat ADR-0013 as the current Ticket 5 engine contract

**Decision**: Keep Silero VAD and Parakeet/Whisper transcription. Use FluidAudio only for offline diarization. Update the stale B-86 Ticket 5 wording before closure.

**Rationale**: ADR-0013 explicitly supersedes ADR-0006 and requires a measured gain before replacing proven transcription or VAD engines. The current code and baseline already implement this split.

**Alternatives considered**: Rebuild VAD and transcription on FluidAudio solely to match stale intake text. Rejected because it contradicts the later accepted ADR and adds unmeasured risk.

## Decision 2: Audit existing Tickets 1, 5, and 6 before writing replacement code

**Decision**: Start each foundation Wave with current checks and gap tests. Preserve working code. Add code only for a failed acceptance seam.

**Rationale**: `main` already contains the rebrand, provenance docs, local post-processing, diarization, per-origin processing, ambiguity metadata, transactional transcript replacement, source preservation, and alignment tests. Historical commits do not prove current acceptance.

**Known gaps**: Packaging was declared but not run; lint and security gates were recorded red; dependency/model pinning has open items; no complete versioned corpus receipt exists; FluidAudio first-run model installation is not release-ready; transcript language metadata and durable crash-recoverable processing state require proof or implementation.

## Decision 3: Generate a private deterministic corpus locally

**Decision**: A versioned manifest and script generate Portuguese, English, code-switch, noise, overlap, multi-speaker, and long-form samples on the target Mac using local system speech and the already bundled audio toolchain. Git stores text, parameters, expected words/speakers, hashes, thresholds, and measurements, not generated audio.

**Rationale**: This is reproducible, private, license-safe, and enough to measure the actual pipeline. Real capture remains a separate native UAT seam.

**Alternatives considered**: Commit binary fixtures; use private human meetings; download public corpora. Rejected for repository size, privacy, licensing, or network dependency.

## Decision 4: Add durable processing job state

**Decision**: Persist one active transcription/diarization job per Session with stage, attempt, input hashes, status, typed error, and timestamps. Atomically replace principal transcript blocks only after every accepted origin result validates.

**Rationale**: The current global retranscription guard prevents concurrent work in one process but cannot explain or recover an interrupted process after restart.

**Alternatives considered**: Keep only an in-memory flag; write status only to Session metadata files. Rejected because neither is a transactional principal job state.

## Decision 5: Introduce one canonical `VerifiableRecord` snapshot

**Decision**: Rust assembles a versioned snapshot from the Session, participants, current principal transcript revision, generation result, and evidence. Generation, export, and provider preview consume this snapshot.

**Rationale**: One read model prevents stale exports, renderer-supplied payloads, and divergence between local findings, JSON, Markdown, and external transfers.

**Alternatives considered**: Let each feature query its own fields; use the current generic frontend `Summary` type. Rejected because the existing summary has no evidence or principal revision contract.

## Decision 6: Use a bundled Swift helper for Apple Foundation Models

**Decision**: Build a small helper that exchanges one NDJSON request and one NDJSON response per process invocation. It checks `SystemLanguageModel` availability, uses guided generation for one fixed record schema, and returns `completed`, `unavailable`, or typed `failed`. Rust validates all generated evidence and persists only validated output.

**Rationale**: The Apple framework is Swift-native. A bounded process gives explicit fault isolation and reuses the current bundled-helper lifecycle pattern without exposing a network port.

**Alternatives considered**: Extend the llama helper; call a cloud model; put evidence validation in Swift or React. Rejected by ADR-0005, the local-default contract, and trust-boundary requirements.

## Decision 7: Remove the V1 llama path from the shipped product

**Decision**: Ticket 8 removes or disables the legacy built-in llama model controls and package path from V1 after the Apple helper passes. Deferred V2 code must not appear as an incomplete V1 control or release artifact.

**Rationale**: ADR-0005 assigns embedded llama.cpp to V2. Keeping both local engines active creates an unrequested fallback and doubles package and model provenance work.

## Decision 8: Export before external providers

**Decision**: Execute Ticket 10 before Ticket 9. The handoff serializers establish the canonical, contract-tested record projection. Provider preview then selects an explicit subset from that projection.

**Rationale**: Tickets 9 and 10 both alter Session actions and Tauri command registration and consume current generated state. Serializing avoids file and contract conflicts.

**Alternatives considered**: Run Tickets 9 and 10 in parallel because both depend only on Ticket 8. Rejected because path, state, and contract independence is false.

## Decision 9: Keep provider secrets only in Keychain

**Decision**: Add a direct macOS Keychain adapter. SQLite retains provider identity, model, task enablement, and non-sensitive transfer provenance. A one-time idempotent application migration moves legacy plaintext credentials to Keychain and clears every plaintext column or embedded JSON secret before provider use.

**Rationale**: Current settings store and return full credentials. This violates B-86, ADR-0010, and ADR-0012.

**Alternatives considered**: Encrypt SQLite fields with an app key; invoke `/usr/bin/security`; keep masked values in React. Rejected because native Keychain already supplies the correct trust boundary and no renderer needs full secrets.

## Decision 10: Build exports atomically and never cache them

**Decision**: Serialize both formats from one snapshot in memory. Write through a temporary sibling file, flush, rename, and remove the temporary file on cancellation or failure. The Rust command obtains the destination through native UI.

**Rationale**: Rebuilding is shorter and safer than cache invalidation. Native destination selection prevents untrusted renderer paths.

## Decision 11: Apply the approved desktop Twelve-Factor exception only

**Decision**: Keep ADR-0012 adaptations. Preserve pinned dependencies, immutable release identity, Keychain config, adapter boundaries, restart-safe jobs, structured redacted diagnostics, and rollback.

**Rationale**: The app is deliberately local and portless. No new exception is necessary.

## Decision 12: Share private source and keep packages local

**Decision**: Classify the current External Candidate as private GitHub source collaboration with local artifacts. A collaborator builds the local ad-hoc DMG from the exact source commit, lockfiles, pinned toolchain inputs, and closed package contract. Each build records its own output hashes. Cross-build byte equality is not required.

**Rationale**: Source access and artifact distribution are different effects. The current goal is reviewable collaboration, not package publication. Fixed inputs, provenance, package closure, scans, signing checks, and observed hashes give a truthful build record without blocking on compiler or linker nondeterminism that does not change the verified source boundary.

**Alternatives considered**: Publish the DMG with the source; keep byte-identical main executables as a release blocker; remove clean-build verification. Rejected because the first expands authority, the second exceeds the accepted collaboration boundary, and the third weakens traceability.
