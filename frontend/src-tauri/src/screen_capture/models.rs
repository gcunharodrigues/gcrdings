use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What a screen recording is pointed at.
///
/// A whole display or a single window, chosen before recording starts. There is
/// no "everything" option: capturing more than was asked for is the failure
/// mode that matters here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum CaptureTarget {
    Display { id: u32, width: i64, height: i64 },
    Window { id: u32, title: String, app: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureTargets {
    pub displays: Vec<CaptureTarget>,
    pub windows: Vec<CaptureTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenRecordingState {
    pub is_recording: bool,
    pub target: Option<CaptureTarget>,
    pub output_path: Option<String>,
    /// Milliseconds into the audio recording at which the screen started.
    ///
    /// The two capture paths start moments apart, and a viewer that assumes
    /// they share a zero would drift for the whole Session.
    pub started_at_offset_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum ScreenCaptureError {
    #[error("screen recording permission has not been granted")]
    PermissionDenied,
    #[error("the selected window or display is no longer available")]
    TargetUnavailable,
    #[error("a screen recording is already running")]
    AlreadyRecording,
    #[error("no screen recording is running")]
    NotRecording,
    #[error("this macOS version cannot record to a file")]
    Unsupported,
    #[error("the screen recording failed")]
    Failed,
}

impl serde::Serialize for ScreenCaptureError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            ScreenCaptureError::PermissionDenied => "permission_denied",
            ScreenCaptureError::TargetUnavailable => "target_unavailable",
            ScreenCaptureError::AlreadyRecording => "already_recording",
            ScreenCaptureError::NotRecording => "not_recording",
            ScreenCaptureError::Unsupported => "unsupported",
            ScreenCaptureError::Failed => "failed",
        };
        let mut state = serializer.serialize_struct("ScreenCaptureError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// Windows worth offering in a picker.
///
/// ScreenCaptureKit reports every layer the window server knows about, most of
/// which are menu shadows, tooltips and zero-sized helpers. Showing those makes
/// the picker unusable, so only ordinary on-screen windows with a title survive.
pub fn is_offerable_window(title: Option<&str>, layer: i64, width: i64, height: i64) -> bool {
    let has_title = title.map(|t| !t.trim().is_empty()).unwrap_or(false);
    has_title && layer == 0 && width >= 120 && height >= 120
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_and_helpers_are_filtered_out() {
        assert!(!is_offerable_window(None, 0, 800, 600), "untitled");
        assert!(!is_offerable_window(Some("  "), 0, 800, 600), "blank title");
        assert!(
            !is_offerable_window(Some("Menu"), 25, 800, 600),
            "overlay layer"
        );
        assert!(
            !is_offerable_window(Some("Tooltip"), 0, 40, 20),
            "too small"
        );
    }

    #[test]
    fn ordinary_windows_are_offered() {
        assert!(is_offerable_window(Some("Zoom Meeting"), 0, 1280, 720));
    }
}
