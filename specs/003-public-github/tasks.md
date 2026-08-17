# Tasks: Safe Public GitHub Source Publication

## Execution Waves

### Wave 1 — Contract and profile

- [x] T001 Create the accepted Spec Kit artifacts and public source Distribution Intent profile. Independent check: JSON validation and artifact consistency scan. Evidence: `fabdfaa`; `release_gate.py controls` PASS with `open_source` and `public_repository`.

### Wave 2 — Candidate sanitization

- [x] T002 Remove environment files, stale backups, unreferenced binaries/images, and local-only artifacts. Harden ignore rules and blank templates. Independent check: tracked-file, history, path, PII, binary, and metadata scans. Evidence: `98cde86`; current tree has no real credential, private path, database, recording, model, package, or release artifact.

### Wave 3 — Public governance

- [x] T003 Update README, provenance, build, contribution, security, agent, CODEOWNERS, and Pull Request guidance. Independent check: stale-reference and documentation consistency scan. Evidence: `0ef79ab`; provenance, branding, and public-boundary tests PASS.

### Wave 4 — CI and dependency hygiene

- [x] T004 Split fast CI from the manual release gate and add pinned CodeQL and Dependabot configuration. Independent check: workflow parser, SHA, permissions, and secret scan. Evidence: `db7cdd6`, `fc35829`, `2a37d8b`, `7f66eb3`; four workflow files parse and all third-party actions use full commit SHAs.

### Wave 5 — Integrated assurance

- [ ] T005 Run focused repository checks, license/dependency checks, and clean-clone verification. Independent check: command results recorded below.
- [ ] T006 Review the exact integrated candidate with one ordinary reviewer and one separate Security Reviewer. Independent check: signed/read-only verdicts outside the candidate tree.
- [ ] T007 Run public-profile `controls` and `evaluate` with fail-closed evidence. Independent check: `PASS` only with no accepted risk.

### Wave 6 — External effect

- [ ] T008 Create the empty public repository and configure `main` protection, CODEOWNER review, required CI, conversation resolution, stale-approval dismissal, and no force-push/deletion.
- [ ] T009 Push only `HEAD:refs/heads/main`; keep Meetily `upstream` fetch-only and never publish work branches.
- [ ] T010 Verify live visibility, published SHA, branch rules, required checks, CI, and secret scanning. Independent check: live GitHub API evidence.

## Execution Evidence

Record exact commit SHAs, command results, reviewer verdicts, gate output paths, GitHub repository slug, and live verification timestamp here before marking tasks complete.

Current checks: frontend `bun test --timeout=15000 --max-concurrency=4 frontend/tests/lib` = 114 pass, 0 fail; Rust `cargo test --workspace --locked` with the pinned FFmpeg staged beside Cargo test binaries = 328 pass, 0 fail, 2 ignored. Historical commits contain only synthetic placeholders and local-machine references; the public candidate will be seeded as one sanitized orphan commit so those unreachable objects are never sent to GitHub.
