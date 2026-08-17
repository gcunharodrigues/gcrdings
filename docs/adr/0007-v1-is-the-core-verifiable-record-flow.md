# Keep V1 to the core verifiable-record flow

V1 will deliver the complete audio path from microphone/system capture or audio import through proven local transcription followed by FluidAudio diarization, participant naming, basic transcript correction, timestamp playback, Apple Foundation Models summaries with evidence, and Markdown/JSON agent handoff. It relies on local storage plus FileVault and retains essential recording integrity and redaction safeguards.

V2 owns video, application-level library encryption and recovery keys, embedded `llama.cpp`, complete revision history, semantic search and organization, calendar automation, multi-application capture, pre-roll, advanced overlap processing, external-library and backup workflows, detailed audit, automated provider integrations, and advanced export formats. These capabilities must not delay validation of the primary workflow.

V1 transcript correction is deliberately limited to editing block text, changing or renaming the assigned participant, and undoing or redoing changes within the current editing session. Persistent revision trees, block splitting and merging, manual timestamp editing, and cross-engine comparison belong to V2.

V1 provides three fixed record types: Meeting, Interview, and Content. Each has a purpose-built summary structure, but users cannot edit templates or create custom record types until V2.

Every V1 Session can export one self-contained Markdown document and one structurally equivalent JSON document. Both contain metadata, participants, the current summary and structured findings, timestamp evidence, and the complete corrected transcript. They are rebuilt from Session state and leave the library only through an explicit Finder save or macOS share action; partial presets, ZIP, media, and subtitle exports belong to V2.
