---
type: ADR
title: Public GitHub source-only boundary
description: Current public distribution exposes source only; binary publication remains aspirational and requires new assurance.
generated: {by: human:guilherme, at: 2026-08-27T15:12:33-03:00}
---

# Public GitHub source-only boundary

## Current

The public GitHub repository contains source only. DMGs, signing inputs, models, recordings, logs, package
outputs, and release receipts remain outside Git and are not published. The current boundary is documented
by `README.md`, `docs/BUILDING.md`, `docs/BASELINE.md`, and `AGENTS.md`.

## Aspirational

A future binary release is permitted only as a proposal pending a new distribution-intent profile, ordinary
review, independent Security Review, a fail-closed External Release Gate, and explicit authorization. This
proposal does not change the current source-only profile.

## Historical

The Wave 7 local qualification recorded in `docs/BASELINE.md` is historical local-build evidence only. It
does not authorize public GitHub artifacts, notarization, or reuse of its assurance for a new channel.

## Verification

Run `okf verify decisions/` and `okf index --check decisions/`; the repository audit is `okf audit --json .`.
