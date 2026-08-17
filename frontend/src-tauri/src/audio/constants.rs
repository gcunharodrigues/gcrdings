/// Supported audio file extensions for import and retranscription.
///
/// V1 audio-first imports: common audio plus ISO media containers whose audio
/// track can be extracted without exposing video features.
pub const MIXED_TRACK_SEEK_TOLERANCE_SECONDS: f64 = 0.1;

pub const AUDIO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "m4a", "wav", "mp3", "flac", "ogg", "aac",
];
