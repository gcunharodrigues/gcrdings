# Complete V1 release qualification

This directory owns the closed Wave 7 release contract. T027 fixes the case IDs, actions, expected codes,
target, thresholds, distribution boundary, evidence bindings, and validator before T028 records any result.

## Current state

Wave 7 is historical local qualification data. The public source repository intentionally omits the
machine-specific receipt, package manifest, install receipts, rollback receipts, and workload evidence.
Maintainers keep those files in the private release workspace and provide them to the local release gate
when qualifying a package. They never authorize this source-only GitHub publication.

The focused contract validates synthetic receipt shapes and proves that forged identity, ancestry, target,
artifacts, locks, prior receipts, evidence files, nested fields, case rows, private data, network
observations, and success states fail closed when the private release workspace is present. A clean public
clone runs the source-only contract and reports local release qualification as not executed.

Run the focused contract check with:

```bash
bun test frontend/tests/lib/v1-qualification.test.ts
```

Validate a private-workspace T028 receipt directly with:

```bash
bun qualification/v1/validate.ts /path/to/private-release-workspace/qualification/v1/target-mac.json
```

The CLI exits zero only for a closed receipt and prints `V1 release receipt valid`. Missing, unreadable,
unexecuted, failed, conflicting, or forged evidence exits nonzero with sanitized identifiers only.

## Fixed release boundary

- Release identity: `gcrdings-0.4.0-aarch64-local-adhoc`.
- Distribution mode: `local_adhoc`.
- Target: Mac15,12 / Apple M3 / 24 GiB / arm64 / macOS 26.5.2 (25F84).
- Public notarized distribution is a separate conditional case whose only accepted T027 state is
  `unsatisfied`. Ad-hoc signing, installation, or code-sign verification can never pass that case.
- The Wave 7 base must be an ancestor of the production candidate, and the candidate must be an ancestor
  of the evidence commit. Every prior Wave receipt candidate must also be an ancestor of the candidate.

The receipt records SHA-256 values for the exact application executable and DMG. Validation hashes those
files at their fixed paths under `CARGO_TARGET_DIR` (or the repository `target` directory), checks the
embedded candidate marker, hashes the final `Cargo.lock` and `frontend/pnpm-lock.yaml`, and hashes every
prior Wave receipt from the repository. Receipt strings never substitute for the files on disk.

## Pre-measurement thresholds

The fixed thresholds in `cases.json` are acceptance limits, not values derived from T028 observations:

| Measurement | Required result |
|---|---:|
| Live capture | at least 1800 seconds |
| Imported audio | at least 1800 seconds |
| Cold startup | at most 15000 ms |
| Graceful shutdown | at most 5000 ms |
| Initial render of 10,000 blocks | at most 5000 ms |
| 10,000-block interaction latency | p95 at most 200 ms |
| RSS growth for capture and import | at most 512 MiB each |
| Existing imported-audio peak | at most 1970 MB |
| Evidence seek error | at most 100 ms |
| Thermal state | never `serious` or `critical` |
| Unapproved network requests | zero |
| Diagnostic private matches | zero |
| Safety failures | zero |

The no-admin energy proxy is the `POWER` column from:

```text
/usr/bin/top -l 1 -pid <app-pid> -stats pid,cpu,mem,power
```

T027 verified the command's availability and output shape without running a workload. T028 samples every
5 seconds during each 1800-second capture/import workload: 360 samples are expected and at least 342 are
required. Aggregation is arithmetic mean plus nearest-rank p95. The fixed conservative ceilings are mean
`POWER` at most 500 and p95 `POWER` at most 1000. Missing, malformed, or insufficient samples fail closed.

## Cargo audit warning policy

`cargo audit` blocks every vulnerability in the active macOS graph. The exact reachable maintenance and
yank warnings reviewed for this candidate are closed in `cargo-audit-reviewed-warnings.json`. A new,
removed, changed, duplicate, malformed, or expired warning review blocks the strict gate. The review
expires on 2026-09-30 and cannot accept a vulnerability.

## Closed case table

`cases.json` is authoritative. Each row fixes one ID, category, action, expected code, and required status.
The receipt must contain each row exactly once with the same action and code. All rows require `passed`
except `public_notarized`, which must remain `unsatisfied`. Unknown rows and unknown nested fields are
rejected. A top-level `passed` outcome is incompatible with an expected, missing, or failed required row.

Case evidence and global observations record only counts and typed results. They must not contain Session
content, transcript, participant names, prompts, credentials, provider payloads, private paths, email
addresses, or unapproved network activity.

## Deterministic local installation and rollback

T035 qualifies the `local_adhoc` package without administrator access through
`scripts/qualify-local-adhoc-install.sh`. The harness accepts only the exact DMG and 40-hex candidate,
installs under the user's `Applications` directory, and always restores the prior application and
`~/Library/Application Support/com.gcrdings.app` state. It rejects symlinks before copying, records
closed SHA-256 tree digests, runs SQLite `PRAGMA quick_check`, stages every replacement beside its final
destination, and leaves a prefix-scoped pending journal so an interrupted rollback is retried first.

The original Keychain default and search list are captured without reading any credential. During the
candidate run, a new private temporary Keychain is the only user Keychain in scope. Cleanup stops the
recorded candidate process group, checks listeners with `lsof`, detaches the read-only DMG, restores the original
Keychain configuration exactly, deletes the temporary Keychain, restores application data, and removes
the pending transaction. Any launch, migration, disk, integrity, alias, cleanup, or restore failure stays
failed and keeps the immutable snapshot available for the next retry.

Run the no-system-write fixture suite with:

```bash
scripts/qualify-local-adhoc-install.sh --self-test
```

The fixture suite covers clean install, existing data, failed launch, migration failure, insufficient
disk, interrupted rollback retry, an already-installed application, alias rejection, exact package
binding, SQLite quiescence, a busy writer, cross-process interruption, abrupt process death, recovery
candidate binding, symlink ancestors, and measured cleanup. T028 invokes
the four receipt modes (`install-clean`, `install-existing`, `rollback`, and `rollback-interrupted`) with
`--application`, `--dmg`, `--package-manifest`, `--candidate`, and `--manifest`. The harness verifies
the closed package manifest before snapshotting, checkpoints each SQLite database after quiescing the
existing application, fsyncs staged replacements, and retains displaced state until post-swap hashes
and integrity pass. Its durable journal records the original Keychain configuration and temporary
Keychain identity so a later process restores both after an abrupt stop. A pending receipt and its closed
transaction journal must bind the same candidate before recovery changes any state. Cleanup evidence is populated
from observed mount, process, helper, listener, Keychain, temporary-artifact, and journal counts rather
than assumed constants. Each successful manifest must validate against
`qualification/v1/rollback-manifest.schema.json`; public notarized distribution remains `unsatisfied`.
