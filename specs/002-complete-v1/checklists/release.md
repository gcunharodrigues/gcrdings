# Release Requirements Checklist: Complete Audio-First V1

**Purpose**: Review whether the Wave 7 release contract is complete, measurable, fail-closed, and auditable before target-Mac execution
**Created**: 2026-08-15
**Feature**: [spec.md](../spec.md)

## Measurement contract

- [x] CHK001 Does T027 define the 30-minute capture and 30-minute import workloads, including start/end boundaries and the evidence retained for each? [Completeness, Spec §User Story 7, FR-017]
- [x] CHK002 Are startup (≤15 s), graceful shutdown (≤5 s), and 10,000-block initial render (≤5 s) defined from an observable user action to an observable ready/closed/rendered state? [Clarity, Plan §Technical Context]
- [x] CHK003 Is the 10,000-block interaction requirement fixed at p95 ≤200 ms with a declared action set, sample count, and percentile calculation before T028 runs? [Measurability, Plan §Performance Goals]
- [x] CHK004 Are RSS growth (≤512 MiB), existing imported-audio peak (≤1970 MB), and thermal state (never serious or critical) explicitly distinguished and measured over the declared workload? [Consistency, Spec FR-004, FR-017]
- [x] CHK005 Is the no-admin energy proxy fixed to a native, reproducible sampling command, cadence, aggregation, and numeric acceptance ceiling before T028 captures results? [Gap, Plan §Performance Goals]
- [x] CHK006 Does every performance row fail closed when a sample, duration boundary, target identity, or required metric is missing? [Coverage, Spec SC-007]

## Package and dependency safety

- [x] CHK007 Does the release contract require production FFmpeg discovery to accept only the verified bundled binary and forbid PATH, home-directory, current-directory, network-download, and shell-profile fallback? [Safety, Spec FR-001, FR-018]
- [x] CHK008 Are builder-path remapping, fixed inputs, exact provenance, closed verification, and observed output hashes required for each clean build without unsupported cross-build byte equality? [Completeness, Spec FR-001, FR-022]
- [x] CHK009 Is package membership governed by a closed allowlist that rejects undeclared executables, models, media, credentials, private paths, source maps, debug artifacts, and V2 controls? [Coverage, Spec Edge Cases, FR-018]
- [x] CHK010 Are `LICENSE.md`, `THIRD_PARTY_NOTICES.md`, `PROVENANCE.md`, and the source-offer artifact required inside the distributed package and bound to the exact dependency/binary manifests? [Traceability, Spec User Story 1]
- [x] CHK011 Do strict format, lint, test, build, `cargo audit`, and `pnpm audit` requirements fail the same way in local release and CI, without interactive setup or unreviewed suppressions? [Consistency, Spec User Story 1, FR-019]

## Distribution and rollback

- [x] CHK012 Is `local_adhoc` the only satisfiable V1 distribution mode, including its exact signing, package, target-Mac, and no-publication boundaries? [Scope, Spec SC-009]
- [x] CHK013 Is public notarized distribution represented as a separate conditional row that remains unsatisfied and cannot be inferred as passed from ad-hoc code-sign verification? [Ambiguity, Plan §Twelve-Factor gate]
- [x] CHK014 Does the rollback contract snapshot application data before install, restore it deterministically after the candidate run, verify hashes/SQLite integrity, and leave the original snapshot untouched? [Completeness, Spec FR-017, SC-009]
- [x] CHK015 Are install cancellation, launch failure, migration failure, rollback interruption, insufficient disk, and already-installed-version cases assigned explicit safe states and retry rules? [Coverage, Spec §Edge Cases]
- [x] CHK016 Does the harness prove cleanup of mounted images, temporary homes/Keychains, listeners, processes, and derived artifacts without requiring administrator privileges? [Privacy, Spec FR-018]

## Ordering and evidence

- [x] CHK017 Is T028 blocked until T027 and every convergence task T031–T036 has one distinct verified commit and all required release-safety gates are green? [Dependency, Spec FR-019, FR-023]
- [x] CHK018 Does the final receipt bind the exact production commit, installed application, installer, target identity, case table, notices, rollback snapshot, and observed hashes from disk rather than trusted strings? [Traceability, Spec §Release Receipt]
- [x] CHK019 Are T029 user acceptance and T030 backlog closure downstream of the immutable T028 receipt, with neither allowed to repair or reinterpret failed evidence? [Ordering, Spec SC-007–SC-010]

## Distribution intent

- [x] CHK020 Does one tracked profile distinguish private source collaboration from local artifact access and activate no public or package-distribution marker? [Authority, Spec FR-020–FR-021]
- [x] CHK021 Are repository creation, push, collaborator invitation, package upload, and publication separate effects that require separate authority? [Authority, Spec FR-024]
- [x] CHK022 Does every External Candidate require the complete Spec Kit sequence, ordinary review, separate Security Review, and fail-closed Release Gate? [Governance, Spec FR-023]

## Notes

- Check items only after the governing requirement is explicit; implementation success alone does not satisfy this checklist.
- `local_adhoc` is the accepted V1 distribution boundary. Public notarized distribution remains a separate unsatisfied conditional until separately authorized and implemented.
- CHK008 is closed by the revised requirement: each clean build proves fixed inputs, package closure, provenance, scans, signing, and exact observed hashes. Cross-build byte equality is outside the accepted private-source/local-artifact boundary.
- CHK014–CHK016 closed after exact-package clean install, existing-data install, rollback, and killed-installer recovery all restored hashes, SQLite integrity, Keychain configuration, and zero cleanup delta.
- T032–T036 and T028–T030 are complete. Ordinary review, Security Review, Release Gate, operator acceptance, and B-86 closure passed in serial order.
