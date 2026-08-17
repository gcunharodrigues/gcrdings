# Feature Specification: Safe Public GitHub Source Publication

**Feature Branch**: `codex/public-github`
**Created**: 2026-08-17
**Status**: Accepted for implementation
**Input**: Operator-approved plan to publish gcrdings as an independent public MIT source repository.

## Clarifications

### Session 2026-08-17

- The repository is independent. It is not a GitHub network fork of Meetily.
- The first release contains source only. DMGs, models, recordings, logs, signing inputs, and packages remain local.
- The public channel is GitHub. A future package or platform channel requires a new intent profile, review, and gate.
- `main` accepts changes only through Pull Requests. The maintainer approves and merges them.
- Meetily provenance remains in `PROVENANCE.md`; an optional fetch-only `upstream` remote is not a publication channel.
- Coverage scan found no unresolved material ambiguity. Clarify therefore records zero questions.

## User Scenarios & Testing

### User Story 1 - Publish a clean source candidate (Priority: P1)

As the maintainer, I can publish the source code in a public repository without exposing secrets, personal data, private paths, generated artifacts, or unreviewed binaries.

**Independent Test**: A clean clone and the candidate's reachable history pass secret, path, PII, binary, dependency, license, workflow, and artifact scans.

**Acceptance Scenarios**:

1. Given a candidate checkout, when tracked files and reachable history are scanned, then no real credential, private path, PII, recording, model, package, DMG, or generated release artifact is found.
2. Given the source-only profile, when the assurance controls run, then `open_source` and `public_repository` resolve to the GitHub public source channel and no artifact is declared.
3. Given an accidental scan hit, unavailable or ambiguous GitHub response, unpinned action, incomplete license, or failed control, when the gate evaluates, then publication is blocked.

### User Story 2 - Collaborate through reviewed Pull Requests (Priority: P1)

As the maintainer, I can require every contributor to work from a fork and submit a Pull Request that passes CI and CODEOWNER review before `main` changes.

**Independent Test**: The live repository reports public visibility, protected `main`, required maintainer and CODEOWNER approval, required checks, resolved conversations, stale-approval dismissal, and no force-push or deletion.

**Acceptance Scenarios**:

1. Given a contributor branch, when it targets `main`, then direct push and direct merge are unavailable.
2. Given a Pull Request, when required CI or conversation resolution is missing, then GitHub prevents merge.
3. Given an approved Pull Request, when the maintainer merges it, then the published `main` SHA is the reviewed candidate SHA.

### User Story 3 - Preserve provenance and future channel boundaries (Priority: P2)

As a future maintainer, I can distinguish source publication from binary or package distribution and trace the imported baseline without implying a fork relationship.

**Independent Test**: README, provenance, build, contribution, security, profile, and license documents agree on source-only scope, MIT licensing, third-party notices, and the next-gate requirement for any new channel.

**Acceptance Scenarios**:

1. Given a request to publish a DMG, package, model, or platform build, when the current profile is inspected, then it blocks until a new channel-specific profile and review are accepted.
2. Given upstream changes, when they are fetched, then the fetch-only remote cannot publish them to the public repository without a reviewed Pull Request.

## Requirements

- **FR-001**: The candidate MUST publish source only under MIT and MUST retain `LICENSE.md`, `THIRD_PARTY_NOTICES.md`, `SOURCE_OFFER.md`, and provenance.
- **FR-002**: The candidate MUST remove tracked credentials, environment files, real personal paths, PII, recordings, models, databases, logs, DMGs, packages, generated build outputs, and unreferenced binary or image assets.
- **FR-003**: Committed environment templates MUST contain blank credential values and `.gitignore` MUST exclude real environment files and common secret/database extensions.
- **FR-004**: The profile MUST use markers `open_source` and `public_repository`, channel `github_public`, source access `public_open_source`, local artifact access, and no artifact paths.
- **FR-005**: Public-facing documentation MUST state independent-repository provenance, source-only scope, MIT obligations, contribution rules, security reporting, and future channel boundaries.
- **FR-006**: Every workflow action MUST use an immutable commit SHA. Workflow permissions MUST be least privilege, and Pull Request workflows MUST use no repository secrets.
- **FR-007**: Fast Pull Request checks MUST be separate from the manual long macOS release-gate workflow.
- **FR-008**: The repository MUST define CODEOWNERS, Dependabot, and CodeQL for the supported Rust and TypeScript surfaces.
- **FR-009**: `main` MUST require Pull Request review by `@gcunharodrigues`, CODEOWNER review, required CI, resolved conversations, stale-approval dismissal, and no force-push or deletion.
- **FR-010**: Publication MUST push only `HEAD:refs/heads/main`; it MUST NOT use `--mirror`, `--all`, or publish work branches.
- **FR-011**: Secret/path/PII/license/workflow/artifact scans, clone verification, ordinary review, Security Review, and fail-closed Release Gate MUST pass before the first source push.
- **FR-012**: A future binary, package, platform, or release channel MUST use a new intent profile, review, and gate; this source publication result MUST NOT be reused.

## Edge Cases

- A placeholder looks like a credential but is synthetic policy data. It remains only when a test or allowlist requires it and is documented as non-secret.
- A secret exists only in reachable history. Publication blocks; the credential is revoked or rotated before any history decision.
- A GitHub request returns 404, 503, timeout, or an ambiguous permission response. The gate blocks; it never treats unavailability as success.
- A local branch, tag, unreachable object, or dirty worktree exists. Only the exact clean `HEAD` is eligible for the push.
- A contributor tries a direct push, force-push, unresolved conversation, stale approval, or unpinned workflow action. Server-side policy or CI blocks merge.
- An image or binary contains metadata or an unreferenced private capture. It is removed or independently verified before publication.

## Success Criteria

- **SC-001**: The exact reviewed candidate is the only source commit pushed to `main`.
- **SC-002**: The live repository is public and its `main` protection and required checks match FR-009.
- **SC-003**: The public clone contains no real secret, private path, PII, local artifact, or unreviewed binary finding.
- **SC-004**: A fresh contributor can follow `CONTRIBUTING.md`, work in a fork, and open a Pull Request without receiving local release artifacts.
- **SC-005**: A future distribution request is visibly blocked by the source-only profile until separately authorized.
