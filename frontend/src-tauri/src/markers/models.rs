use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A moment someone flagged while recording, to come back to later.
///
/// `offset_ms` is measured against the active recording clock — the same one
/// `audio_start_time` uses on transcripts. Wall-clock would drift away from the
/// audio every time the recording is paused.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMarker {
    pub id: String,
    pub meeting_id: String,
    pub offset_ms: i64,
    pub label: Option<String>,
    pub created_at: String,
}

/// A marker captured before the Session exists in the database.
///
/// Recording starts long before `api_save_transcript` creates the meeting row,
/// so markers are buffered in memory and drained once there is an id to attach
/// them to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingMarker {
    pub offset_ms: i64,
    pub label: Option<String>,
}

#[derive(Debug, Error)]
pub enum MarkerError {
    #[error("Session not found")]
    NotFound,
    #[error("marker storage failed")]
    Storage,
    #[error("no recording is active")]
    NotRecording,
}

impl serde::Serialize for MarkerError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            MarkerError::NotFound => "not_found",
            MarkerError::Storage => "storage",
            MarkerError::NotRecording => "not_recording",
        };
        let mut state = serializer.serialize_struct("MarkerError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// Trims and normalises a user-supplied label; blank becomes None so the UI
/// does not have to distinguish "" from "never labelled".
pub fn normalise_label(label: Option<String>) -> Option<String> {
    label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_labels_collapse_to_none() {
        assert_eq!(normalise_label(None), None);
        assert_eq!(normalise_label(Some("   ".into())), None);
        assert_eq!(normalise_label(Some("".into())), None);
    }

    #[test]
    fn labels_are_trimmed_but_kept() {
        assert_eq!(
            normalise_label(Some("  decisão importante \n".into())),
            Some("decisão importante".to_string())
        );
    }
}
