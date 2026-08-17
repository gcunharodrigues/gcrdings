# Agent Handoff Contract

Markdown and JSON are generated from the same immutable `VerifiableRecord` snapshot.

Both include:

- schema version and Session metadata
- fixed record type
- participants
- generation status, current summary, and current structured findings
- each evidence reference with block ID and timestamp
- the complete corrected principal transcript
- local or external result provenance where present

Both exclude:

- audio or video bytes and source paths
- credentials or Keychain identifiers
- prompts, model private state, logs, or temporary paths
- stale generated results presented as current

JSON is canonical for semantic comparison. Markdown section order may differ only as presentation. Tests parse the Markdown projection into canonical facts and compare them with JSON for all three record types.

Destination selection occurs inside the native command. Cancellation returns without writing. Failure removes the temporary sibling file and leaves the Session unchanged.

