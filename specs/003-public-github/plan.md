# Implementation Plan: Safe Public GitHub Source Publication

## Technical context

gcrdings is a Rust/Tauri and TypeScript application with macOS build inputs and local qualification artifacts. The public candidate must keep those local boundaries while retaining source, licenses, provenance, build instructions, and review policy.

## Constitution check

- Principle I: authority is the accepted operator plan and this feature contract.
- Principle II: use the existing Code Lane, Spec Kit, GitHub, and standard scanners; add no runner.
- Principle III: each requirement has a measurable scan, live setting, or document check.
- Principle IV: file groups can be edited independently, but integration and external effects are serialized.
- Principle V: isolated worktree, ordinary review, Security Review, Release Gate, then separate GitHub publication.

## Delivery sequence

1. Record the accepted specification, zero-question Clarify result, plan, checklist, and Analyze result.
2. Remove public leakage and generated/unreferenced assets. Preserve synthetic negative-test markers.
3. Update public documentation, contribution governance, profile, and security contact.
4. Split fast CI from the manual long release gate. Pin every action. Add Dependabot and CodeQL for the supported Rust and TypeScript surfaces.
5. Run static and focused checks, then review the exact integrated candidate.
6. Create an empty public repository, configure server-side rules, publish only `HEAD:main`, and verify live state.

## Deliberate scope cuts

- No local Git hook is treated as an authority. Hooks do not clone through Git and are optional developer defense.
- No DMG, model, package, signing input, or release automation is published.
- The public seed uses one sanitized orphan commit. This prevents historical local-machine references and synthetic test markers from becoming reachable on GitHub. No real credential was found; a real credential would still block publication and require rotation and a separate history decision.
