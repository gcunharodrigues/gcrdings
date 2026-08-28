use std::sync::Mutex;

use tauri::{AppHandle, Runtime};

use super::models::{normalise_label, MarkerError, PendingMarker, SessionMarker};
use super::repository::MarkerRepository;
use crate::state::AppState;

/// Markers captured before the Session row exists.
///
/// Recording starts long before api_save_transcript creates the meeting, so
/// there is no id to attach a marker to at the moment it is dropped. Buffering
/// here rather than in the frontend means a window reload does not lose them.
static PENDING_MARKERS: Mutex<Vec<PendingMarker>> = Mutex::new(Vec::new());

fn take_pending() -> Vec<PendingMarker> {
    PENDING_MARKERS
        .lock()
        .map(|mut buffer| std::mem::take(&mut *buffer))
        .unwrap_or_default()
}

/// Drains the buffer into the Session that was just created.
///
/// Called from the save path. A failure here must not fail the save: losing a
/// marker is an annoyance, losing the transcript is not.
pub async fn flush_pending_markers(pool: &sqlx::Pool<sqlx::Sqlite>, meeting_id: &str) -> u64 {
    let pending = take_pending();
    MarkerRepository::insert_many(pool, meeting_id, &pending)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn api_add_pending_marker<R: Runtime>(
    _app: AppHandle<R>,
    offset_ms: i64,
    label: Option<String>,
) -> Result<usize, MarkerError> {
    if offset_ms < 0 {
        return Err(MarkerError::NotRecording);
    }

    let mut buffer = PENDING_MARKERS.lock().map_err(|_| MarkerError::Storage)?;
    buffer.push(PendingMarker {
        offset_ms,
        label: normalise_label(label),
    });
    Ok(buffer.len())
}

#[tauri::command]
pub async fn api_get_pending_markers<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<Vec<PendingMarker>, MarkerError> {
    PENDING_MARKERS
        .lock()
        .map(|buffer| buffer.clone())
        .map_err(|_| MarkerError::Storage)
}

/// Used when a recording is discarded rather than saved.
#[tauri::command]
pub async fn api_clear_pending_markers<R: Runtime>(_app: AppHandle<R>) -> Result<(), MarkerError> {
    take_pending();
    Ok(())
}

#[tauri::command]
pub async fn api_get_session_markers<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<SessionMarker>, MarkerError> {
    MarkerRepository::list(state.db_manager.pool(), &meeting_id).await
}

#[tauri::command]
pub async fn api_add_session_marker<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    offset_ms: i64,
    label: Option<String>,
) -> Result<SessionMarker, MarkerError> {
    MarkerRepository::insert(
        state.db_manager.pool(),
        &meeting_id,
        &PendingMarker {
            offset_ms: offset_ms.max(0),
            label: normalise_label(label),
        },
    )
    .await
}

#[tauri::command]
pub async fn api_update_session_marker<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    marker_id: String,
    label: Option<String>,
) -> Result<(), MarkerError> {
    MarkerRepository::update_label(
        state.db_manager.pool(),
        &marker_id,
        normalise_label(label).as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn api_delete_session_marker<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    marker_id: String,
) -> Result<(), MarkerError> {
    MarkerRepository::delete(state.db_manager.pool(), &marker_id).await
}
