# Adapt Twelve-Factor to a local desktop application

gcrdings will preserve the Twelve-Factor invariants that improve reproducibility, replaceability, release discipline, failure recovery, and operational clarity, but it has an approved exception where the standard assumes a horizontally deployed network service.

The V1 desktop architecture therefore uses these adaptations:

- Durable user data belongs to the local Session library instead of an external backing service.
- The application does not expose or require a network port.
- Capture and processing run as explicit local components for fault isolation and resource control, not as horizontally scalable process types.
- Sanitized structured diagnostics use the macOS logging system rather than relying exclusively on standard streams.
- Versioned, transactional library migrations run from the immutable application release rather than through a separate platform one-off process.

The remaining factors stay in force: one version-controlled codebase, complete and pinned dependencies, secrets protected by Keychain, replaceable provider adapters, immutable identifiable builds, restart-safe jobs, reproducible development and release environments, and tested graceful and abrupt termination.

Revisit this exception if gcrdings introduces a server, shared library, cloud synchronization, collaboration, or remotely executed processing.
