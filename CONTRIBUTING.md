# Contributing to gcrdings

gcrdings is a public MIT-licensed source repository. All changes enter `main` through a Pull Request.
Direct pushes, force-pushes, and local release artifacts are not part of the contribution workflow.

## Start safely

1. Fork `gcunharodrigues/gcrdings` on GitHub and clone your fork.
2. Add the Meetily source repository as a fetch-only remote only when an upstream comparison is needed:

   ```bash
   git remote add upstream https://github.com/Zackriya-Solutions/meetily.git
   git remote set-url --push upstream disabled://fetch-only
   ```

3. Create a branch from the current public `main`:

   ```bash
   git switch main
   git pull --ff-only origin main
   git switch -c fix/short-description
   ```

Do not copy local `.env` files, recordings, transcripts, models, DMGs, signing inputs, logs, or package
outputs into the repository.

## Development lane

Use the repository's Code Lane. Non-trivial changes follow the pinned Spec Kit sequence and record the
requirements, clarification result, plan, tasks, analysis, implementation, convergence, and review in
`specs/<feature>/`. External publication or artifact changes require a separate Security Review and
fail-closed External Release Gate.

## Before opening a Pull Request

- Run the focused checks for the changed area.
- Run `git diff --check`.
- Scan the diff for credentials, private paths, personal data, generated output, and unlicensed assets.
- Keep actions pinned to full commit SHAs.
- Never add a real credential to an example file. Use blank values in `*.env.example`.

## Pull Request process

1. Open a Pull Request from your fork to `main`.
2. Describe the behavior, security impact, tests, and documentation impact.
3. Wait for required CI and Code Owner review.
4. Address review comments with new commits.
5. Only the maintainer merges the approved Pull Request.

The maintainer may request a new External Candidate when the audience, channel, platform, artifact, or
privilege boundary changes. A source-only approval does not authorize a binary release.

## License

By contributing, you agree that the contribution is provided under the MIT License in `LICENSE.md`.
Third-party code remains under its own license; update `THIRD_PARTY_NOTICES.md` when dependency or asset
provenance changes.
