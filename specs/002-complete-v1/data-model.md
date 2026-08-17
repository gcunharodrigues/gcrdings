# Data Model: Complete Audio-First V1

## Delivery governance entity

### Distribution Intent Profile

One tracked root `.external-assurance.json` classifies delivery effects. It is governance data, not application runtime state.

- `source_access`: `private_collaborators`
- `artifact_access`: `local`
- `build_authority`: `collaborator`
- `channels`: `github_private`
- `platforms`: `macos`
- `update_mechanism`: `none`
- `markers`: empty; no public repository, open-source, distributed-artifact, hosted-service, registry, or store claim

Rule: a source repository signal can propose this profile but cannot authorize an effect. A DMG hash or local qualification receipt cannot authorize artifact publication.

## Existing principal entities

### Session (`meetings`)

- `id`: stable Session identifier
- `title`, `created_at`, `updated_at`, `folder_path`
- `principal_transcript_revision`: optimistic concurrency revision
- New `record_type`: `meeting`, `interview`, or `content`

### Participant (`participants`)

- `id`, `meeting_id`, `display_name`
- `speaker_cluster_id`: immutable discovery provenance when applicable

### Transcript Block (`transcripts`)

- `id`, `meeting_id`, `participant_id`, principal `transcript`
- `source_origin`, `speaker_cluster_id`, `ambiguity_group_id`, `alignment_decision`
- `audio_start_time`, `audio_end_time`, `duration`
- New `language`: optional BCP-47 tag from the engine when available

Validation: identity is stable; start is finite and non-negative; end is not before start; participant belongs to the Session; source and alignment provenance are never editable through principal correction.

## New durable entities

### Processing Job

One current job per Session and job kind.

- `id`, `meeting_id`, `kind`, `status`, `stage`, `attempt`
- `input_manifest_hash`: digest of immutable origins and selected engine/model configuration
- `started_at`, `updated_at`, `completed_at`
- `error_code`: typed, non-sensitive failure; no transcript or path content

States: `pending → running → completed`; `running → failed|interrupted`; `failed|interrupted → running` with incremented attempt. Transcript replacement occurs in one transaction after validated completion. A restart converts orphan `running` to `interrupted`.

### Record Generation

One principal local generation per Session.

- `meeting_id`, `schema_version`, `record_type`
- `status`: `pending`, `processing`, `completed`, `unavailable`, `failed`, `stale`
- `input_revision`: principal transcript revision used by the helper
- `result_json`: validated type-specific summary and findings
- `error_code`, `model_version`, `prompt_version`, `created_at`, `updated_at`

Rule: a completed result is current only when `input_revision == meetings.principal_transcript_revision`. Any later transcript save makes it stale/pending for presentation and export.

### Evidence Reference

Stored inside validated generation JSON and materialized by the canonical read model.

- `transcript_block_id`
- `start_seconds`
- optional `end_seconds`

Rule: the block belongs to the Session; times match or fall within the block playable interval; invalid evidence rejects the complete generated result.

### Provider Task Authorization

- `provider`, `task`, `enabled`, `credential_present`, `updated_at`
- The secret value exists only under a stable Keychain service/account tuple.

### Provider Transfer

- `id`, `meeting_id`, `provider`, `task`, `snapshot_revision`
- `preview_digest`, `data_types`, `purpose`
- `status`: `confirmed`, `sending`, `completed`, `failed`
- `result_json`: schema-valid external result stored separately from local generation
- `error_code`, `created_at`, `completed_at`

Rule: send recomputes the server-side preview digest and consumes one confirmed transfer. A retry requires a new confirmation and never changes provider automatically.

## Derived entities

### Verifiable Record

Versioned immutable read model assembled per request:

- Session metadata and record type
- participants
- current principal transcript revision and complete transcript
- local generation state, summary, findings, and resolved evidence
- optional external results with provider/task provenance

It contains no source paths, media bytes, credential fields, prompts, or private diagnostic metadata.

### Agent Handoff

Two pure projections of one Verifiable Record snapshot:

- Markdown presentation
- JSON structured contract

No export artifact is principal or cached.
