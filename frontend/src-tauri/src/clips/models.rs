use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ClipStatus {
    Pending,
    Rendering,
    Ready,
    Failed,
}

impl ClipStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipStatus::Pending => "pending",
            ClipStatus::Rendering => "rendering",
            ClipStatus::Ready => "ready",
            ClipStatus::Failed => "failed",
        }
    }

    /// Named to avoid shadowing FromStr, which this is not: an unknown value
    /// falls back to Pending rather than failing, because the column is
    /// CHECK-constrained and a surprise there is a bug, not user input.
    pub fn from_column(value: &str) -> Self {
        match value {
            "rendering" => ClipStatus::Rendering,
            "ready" => ClipStatus::Ready,
            "failed" => ClipStatus::Failed,
            _ => ClipStatus::Pending,
        }
    }
}

/// A range of a Session, cut from its original media.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub meeting_id: String,
    pub title: Option<String>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub status: ClipStatus,
    pub file_path: Option<String>,
    pub error_code: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Error)]
pub enum ClipError {
    #[error("Session not found")]
    NotFound,
    #[error("the end must come after the start")]
    EmptyRange,
    #[error("the Session has no media to cut from")]
    SourceUnavailable,
    #[error("the clip could not be cut")]
    RenderFailed,
    #[error("clip storage failed")]
    Storage,
}

impl serde::Serialize for ClipError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            ClipError::NotFound => "not_found",
            ClipError::EmptyRange => "empty_range",
            ClipError::SourceUnavailable => "source_unavailable",
            ClipError::RenderFailed => "render_failed",
            ClipError::Storage => "storage",
        };
        let mut state = serializer.serialize_struct("ClipError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// Orders a pair of marker offsets and rejects a range with no duration.
///
/// Two markers arrive in whatever order they were dropped, and the later one is
/// not necessarily the second one clicked.
pub fn normalise_range(a: i64, b: i64) -> Result<(i64, i64), ClipError> {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    if end <= start.max(0) {
        return Err(ClipError::EmptyRange);
    }
    Ok((start.max(0), end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_dropped_out_of_order_still_make_a_range() {
        assert_eq!(normalise_range(9000, 3000).unwrap(), (3000, 9000));
        assert_eq!(normalise_range(3000, 9000).unwrap(), (3000, 9000));
    }

    #[test]
    fn a_range_with_no_duration_is_rejected() {
        assert!(matches!(
            normalise_range(5000, 5000),
            Err(ClipError::EmptyRange)
        ));
    }

    #[test]
    fn a_negative_start_is_clamped_rather_than_trusted() {
        assert_eq!(normalise_range(-2000, 4000).unwrap(), (0, 4000));
    }
}
