# Bug Assessment: T028 evidence is not bound to the release receipt

- **Slug**: t028-evidence-binding
- **Created**: 2026-08-17
- **Source**: pasted independent-review finding
- **Verdict**: valid
- **Severity**: high

## Report

The final T028 receipt records generic pass codes but does not bind the two long-workload sample files or the four install and rollback manifests. The validator still passes if those files disappear or change.

## Symptom

`qualification/v1/target-mac.json` validates without cryptographic or semantic links to the six measured evidence files. CHK018 requires those observed results to remain bound to the final receipt.

## Reproduction

1. Validate `qualification/v1/target-mac.json`; it passes.
2. Remove or change any `/private/tmp/gcrdings-t028-*-a067c8e.json` or long-workload sample file.
3. Validate the receipt again; it still passes.

## Suspected Code Paths

- `qualification/v1/validate.ts` — validates artifact, lock, and prior-receipt hash rows, but has no T028 evidence rows or semantic checks.
- `qualification/v1/cases.json` — declares no persisted T028 evidence files.
- `qualification/v1/target-mac.json` — records only generic case evidence.
- `frontend/tests/lib/v1-qualification.test.ts` — has no regression for missing, changed, or semantically mismatched T028 evidence.

## Root Cause Hypothesis

Confidence: high. The closed receipt schema was designed before T028 execution and modeled only generic per-case safety counts. It omitted a top-level evidence inventory. The measured files remained in `/private/tmp`, outside the immutable evidence commit.

## Proposed Remediation

**Preferred**: Persist sanitized copies of the two long-workload sample files and four install/rollback manifests under `qualification/v1/evidence/`. Declare the six paths in `cases.json`. Add closed `evidence_files` hash rows to the receipt. Make the validator verify file hashes and semantics: workload metrics must match the receipt; install manifests must bind the package candidate, result code, restored snapshot, database integrity, and zero cleanup delta.

The long workload files currently contain only aggregate/process samples and no private paths or content. Preserve the exact bytes. Record the runtime candidate separately in the contract. Do not repeat the workloads or builds.

**Files likely to change**:

- `qualification/v1/evidence/*.json`
- `qualification/v1/cases.json`
- `qualification/v1/target-mac.json`
- `qualification/v1/validate.ts`
- `frontend/tests/lib/v1-qualification.test.ts`
- release documentation that cites the capture sample count

**Tests to add or update**:

- Reject missing, changed, duplicate, or unknown evidence hash rows.
- Reject workload metric drift from the persisted samples.
- Reject install candidate, package, result, restoration, database, or cleanup drift.

## Risks & Considerations

- Do not store private paths, transcript content, credentials, or raw user data.
- Keep package source candidate `a067c8e` distinct from runtime measurement candidate `4c76cd6`.
- The machine file is authoritative: correct any stale copied metric before reissuing the receipt.

## Open Questions

- None.
