use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Runtime};

use super::models::{normalise_range, Clip, ClipError, ClipStatus};
use super::repository::ClipRepository;
use crate::audio::ffmpeg::find_bundled_ffmpeg_path;
use crate::state::AppState;

/// Cuts from the preserved original when there is one, so a clip of an imported
/// video stays video. Recordings made here are audio only, and fall back to the
/// mixed track. Nothing inspects streams: whatever the source had, the cut has.
fn resolve_source(folder: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem == "original")
            {
                return Some(path);
            }
        }
    }

    let mixed = folder.join("audio.mp4");
    mixed.is_file().then_some(mixed)
}

fn clips_folder(folder: &Path) -> PathBuf {
    folder.join("clips")
}

/// Re-encodes rather than stream-copying. A stream copy snaps to the nearest
/// keyframe, which on a long recording can move the start by seconds — and the
/// whole point of a clip is that someone chose exactly where it begins.
async fn render(
    source: &Path,
    destination: &Path,
    start_ms: i64,
    end_ms: i64,
) -> Result<(), ClipError> {
    // Held for the whole run: dropping it removes the staging directory the
    // verified binary lives in, and the spawn would lose its executable.
    let ffmpeg = find_bundled_ffmpeg_path().map_err(|_| ClipError::RenderFailed)?;
    let command = ffmpeg.command().map_err(|_| ClipError::RenderFailed)?;

    let start = format!("{:.3}", start_ms as f64 / 1000.0);
    let duration = format!("{:.3}", (end_ms - start_ms) as f64 / 1000.0);

    let output = tokio::process::Command::from(command)
        .args(["-nostdin", "-y", "-ss", &start, "-i"])
        .arg(source)
        .args([
            "-t",
            &duration,
            // Applied only if the source carries a video stream; ffmpeg ignores
            // them for audio-only input.
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
        ])
        .arg(destination)
        .output()
        .await
        .map_err(|_| ClipError::RenderFailed)?;

    drop(ffmpeg);

    if !output.status.success() {
        // A partial file would look like a finished clip that plays nothing.
        let _ = std::fs::remove_file(destination);
        return Err(ClipError::RenderFailed);
    }
    Ok(())
}

#[tauri::command]
pub async fn api_list_clips<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<Clip>, ClipError> {
    ClipRepository::list(state.db_manager.pool(), &meeting_id).await
}

/// Creates the clip row, then cuts in the background.
///
/// The row exists before the file does so the UI can show it as rendering
/// rather than appearing to do nothing while ffmpeg runs.
#[tauri::command]
pub async fn api_create_clip<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    start_ms: i64,
    end_ms: i64,
    title: Option<String>,
) -> Result<Clip, ClipError> {
    let (start_ms, end_ms) = normalise_range(start_ms, end_ms)?;
    let pool = state.db_manager.pool().clone();

    let folder = ClipRepository::meeting_folder(&pool, &meeting_id).await?;
    let folder = PathBuf::from(folder);
    let source = resolve_source(&folder).ok_or(ClipError::SourceUnavailable)?;

    let clip = ClipRepository::create(
        &pool,
        &meeting_id,
        title.as_deref().map(str::trim).filter(|t| !t.is_empty()),
        start_ms,
        end_ms,
    )
    .await?;

    let clip_id = clip.id.clone();
    tauri::async_runtime::spawn(async move {
        let _ =
            ClipRepository::set_status(&pool, &clip_id, ClipStatus::Rendering, None, None).await;
        let _ = app.emit("clip-status-changed", &clip_id);

        let destination_folder = clips_folder(&folder);
        if std::fs::create_dir_all(&destination_folder).is_err() {
            let _ = ClipRepository::set_status(
                &pool,
                &clip_id,
                ClipStatus::Failed,
                None,
                Some("storage"),
            )
            .await;
            let _ = app.emit("clip-status-changed", &clip_id);
            return;
        }

        let extension = source.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
        let destination = destination_folder.join(format!("{clip_id}.{extension}"));

        match render(&source, &destination, start_ms, end_ms).await {
            Ok(()) => {
                let _ = ClipRepository::set_status(
                    &pool,
                    &clip_id,
                    ClipStatus::Ready,
                    Some(&destination.to_string_lossy()),
                    None,
                )
                .await;
            }
            Err(_) => {
                let _ = ClipRepository::set_status(
                    &pool,
                    &clip_id,
                    ClipStatus::Failed,
                    None,
                    Some("render_failed"),
                )
                .await;
            }
        }
        let _ = app.emit("clip-status-changed", &clip_id);
    });

    Ok(clip)
}

#[tauri::command]
pub async fn api_rename_clip<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    clip_id: String,
    title: Option<String>,
) -> Result<(), ClipError> {
    ClipRepository::rename(
        state.db_manager.pool(),
        &clip_id,
        title.as_deref().map(str::trim).filter(|t| !t.is_empty()),
    )
    .await
}

/// Deletes the row and its file. The Session's own media is never touched.
#[tauri::command]
pub async fn api_delete_clip<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    clip_id: String,
) -> Result<(), ClipError> {
    let file_path = ClipRepository::delete(state.db_manager.pool(), &clip_id).await?;
    if let Some(path) = file_path {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[tauri::command]
pub async fn api_reveal_clip<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    clip_id: String,
) -> Result<(), ClipError> {
    let clip = ClipRepository::get(state.db_manager.pool(), &clip_id).await?;
    let path = clip.file_path.ok_or(ClipError::NotFound)?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .args(["-R", &path])
            .spawn()
            .map_err(|_| ClipError::Storage)?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = path;

    Ok(())
}
