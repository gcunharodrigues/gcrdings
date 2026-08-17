# Provider Transfer Contract

## Credential boundary

The renderer can request credential status, save, replace, test, or remove. It never receives the saved secret. Keychain stores the secret. SQLite stores only provider configuration and task enablement.

## Preview

Rust builds a preview from the current `VerifiableRecord` and returns:

- provider display name
- Session title
- named purpose and task
- exact data types
- principal transcript revision
- opaque preview digest

The renderer cannot provide or modify the payload.

## Send

After confirmation, the renderer submits only the preview digest. Rust verifies current task enablement, credential presence, unchanged snapshot digest, and an unused confirmation. It then sends exactly the previewed data through the selected adapter.

Cancellation sends nothing. Failure does not retry automatically, substitute a provider, or reuse confirmation. Video and media are forbidden. The stored result includes provider and task provenance and remains separate from local generation.

