# Security policy

## Supported source line

Only the current `main` branch is supported. The public repository publishes source code only. DMGs,
signing inputs, models, recordings, transcripts, logs, and package receipts are not supported disclosure
channels.

## Report a vulnerability

Do not open a public issue for an undisclosed vulnerability. Use a GitHub private vulnerability report or
contact the repository maintainer through the private contact method listed in the repository owner profile.
Include the affected commit, impact, reproduction steps, and a minimal safe proof. Do not include credentials,
personal data, recordings, or full user content.

If a report contains a credential, stop using it and ask the owner to revoke or rotate it immediately. The
maintainer will acknowledge a valid report, reproduce it in an isolated checkout, and publish a fix through a
Pull Request after the disclosure boundary is agreed.

## Public safety rules

- Never put secrets in issues, Pull Requests, CI logs, examples, screenshots, or release artifacts.
- Report accidental exposure privately. Do not attempt to access data that is not yours.
- Security fixes follow the same protected-`main` review process, with an independent Security Review when
  the change crosses the external-assurance boundary.
