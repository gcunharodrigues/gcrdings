# Native Generation Contract

## Request

One UTF-8 NDJSON object on stdin:

- `schema_version`
- `request_id`
- `record_type`: `meeting`, `interview`, or `content`
- `principal_transcript_revision`
- participants and ordered transcript blocks with stable IDs and playable times
- versioned prompt instructions

No media, file path, credential, provider config, or external URL is allowed.

## Response

One UTF-8 NDJSON object on stdout:

- `completed`: schema-valid type-specific summary and findings with evidence block IDs
- `unavailable`: typed reason such as unsupported OS, ineligible device, Apple Intelligence disabled, unsupported locale, or model not ready
- `failed`: typed generation, context-size, decoding, timeout, or helper failure

Rust treats malformed JSON, extra protocol messages, revision mismatch, unknown fields, unresolved evidence, and non-playable time as failure. No response causes cloud fallback.

## Concurrency

One helper process handles one request. A Session has at most one active local generation. Cancellation or helper death leaves principal transcript and prior accepted result unchanged.

