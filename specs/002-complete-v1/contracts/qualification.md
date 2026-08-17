# Qualification Contract

Every receipt names:

- exact candidate commit and release identity
- target Mac hardware and OS
- dependency, helper, engine, model, prompt, and corpus manifest versions
- command or UI action
- expected and observed result
- duration, peak memory, and applicable quality or seek measurement
- network and diagnostic observation result
- artifact hashes when applicable
- exact source commit, locked dependency identities, pinned toolchain inputs, closed package membership, and legal/provenance identities for each local build

Private audio, transcript content, participant names, credentials, prompts, and provider payloads are excluded.

The final case table covers normal, boundary, failure, concurrency, cancellation, restart, full-disk risk, long duration, source loss, unavailable model, invalid output, export cleanup, provider failure, privacy, packaging, install, and rollback.

The `local_adhoc` package contract verifies each build independently. It records exact output hashes but does not require byte-identical executables across builds. Private GitHub source collaboration does not publish or transfer a DMG.
