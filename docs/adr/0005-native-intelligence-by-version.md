# Use Apple Foundation Models in V1 and defer embedded llama.cpp to V2

V1 will generate local summaries and structured findings through a bundled Swift helper that uses Apple Foundation Models on macOS. It will not require Ollama or ship a separate local model runtime. V2 will add a pinned `llama.cpp` model accelerated by Metal as an embedded fallback, alongside the video generation boundary already assigned to V2.

When Apple Foundation Models is unavailable, capture, transcription, diarization, editing, playback, and export remain usable. Summary and structured extraction stay pending until local intelligence becomes available or the user explicitly authorizes an external provider; gcrdings never falls back to cloud processing silently.
