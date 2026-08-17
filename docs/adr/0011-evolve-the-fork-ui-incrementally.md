# Evolve the fork UI incrementally

V1 will reuse Meetily Community navigation and functional components where they fit, while removing all upstream branding and links. The differentiated Session workspace—summary, participants, timestamped transcript, evidence, and agent export—must be prototyped and accepted before implementation. Unrelated settings and model-management surfaces are redesigned only when the working product demonstrates a need.

## Accepted Session workspace

Guilherme accepted the **Evidence desk** prototype on 2026-07-29. The production workspace will keep the timestamped transcript and mixed-track player at the center, summary and participants in the left context column, and evidence in the right verification column. Evidence timestamps select the corresponding transcript block and seek playback. Meeting, Interview, and Content Sessions share this structure, including explicit empty, processing, model-unavailable, failed, and completed states. Agent Handoff remains a Session-level action.
