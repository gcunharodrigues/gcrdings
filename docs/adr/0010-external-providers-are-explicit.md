# Keep external providers available but inactive by default

V1 retains supported external AI providers as optional capabilities, while FluidAudio and Apple Foundation Models remain the defaults. A provider becomes usable only after the user stores its credential in Keychain, tests it, enables it for a named task, and confirms each transfer with the Session, purpose, and data type shown. No external request or fallback occurs silently, and V1 never sends video.
