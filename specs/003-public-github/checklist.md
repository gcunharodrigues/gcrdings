# Publication Checklist

## Candidate hygiene

- [ ] No real credential in the current tree or reachable `main` history.
- [ ] No private path, PII, recording, model, database, log, DMG, package, or generated artifact.
- [ ] No unreferenced binary or image with personal metadata.
- [ ] Blank environment templates only; real environment files ignored.
- [ ] MIT, third-party notices, source offer, and provenance agree.

## Governance and CI

- [ ] PR-only contribution policy and CODEOWNERS are committed.
- [ ] Actions use full commit SHAs and least-privilege permissions.
- [ ] Pull Request checks are fast; macOS release checks are manual and separate.
- [ ] Dependabot and CodeQL cover supported dependency and code surfaces.

## Assurance and publication

- [ ] Exact clean candidate clone verified.
- [ ] Ordinary independent Code Review passed.
- [ ] Separate Security Review passed.
- [ ] Public source profile controls and evaluate passed.
- [ ] Empty public repository and branch rules configured.
- [ ] Only `HEAD:refs/heads/main` pushed.
- [ ] Live SHA, visibility, protection, CI, and secret scanning verified.
