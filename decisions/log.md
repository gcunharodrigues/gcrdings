# Decision log

Append-only, **newest first**. One entry per notable decision or operational action:
`## YYYY-MM-DD — title` then **Decision / Why / Alternatives / Provenance**. Hard-to-reverse decisions
are promoted to an ADR (`NNNN-slug.md` — see [README](./README.md)).

## 2026-08-27 — Public GitHub source-only boundary

**Decision**

Current behavior remains source-only on the public GitHub repository. DMGs, signing inputs, models,
recordings, logs, package outputs, and release receipts stay outside Git and are not published. A binary
release is Aspirational: it requires a new distribution-intent profile, ordinary review, independent
Security Review, a fail-closed External Release Gate, and explicit authorization.

**Why**

The current `README.md`, `docs/BUILDING.md`, `docs/BASELINE.md`, and `AGENTS.md` separate public source
from local build inputs and historical qualification evidence. Keeping the future binary path conditional
preserves the current audience, channel, and artifact boundary.

**Alternatives**

Public binary publication, public artifacts, or reuse of historical local qualification for a new channel
would expand the current contract without its required assurance.

**Provenance**

Current public state: `README.md`, `docs/BUILDING.md`, `docs/BASELINE.md`, and `AGENTS.md`; source-only
candidate recorded at base commit `a30e44ac7cf0c15d260e0079c74b3200eb533328`. Historical Wave 7 local
qualification remains evidence only in `docs/BASELINE.md`.
