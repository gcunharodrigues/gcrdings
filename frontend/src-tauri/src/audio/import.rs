//! Transactional, audio-first media import.
//!
//! V1 preserves the selected source, derives one playable audio track, and then
//! starts the same local batch transcription used after recording. It does not
//! expose video or create a database row until all media work succeeds.

#[cfg(test)]
use crate::audio::decoder::decode_audio_file;
use crate::state::AppState;
use anyhow::{anyhow, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use super::audio_processing::sanitize_filename;
use super::constants::AUDIO_EXTENSIONS;
use super::ffmpeg::find_ffmpeg_path;
use super::recording_preferences::get_default_recordings_folder;

static IMPORT_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
static IMPORT_CANCELLED: AtomicBool = AtomicBool::new(false);
const MAX_FILE_SIZE_BYTES: u64 = 20 * 1024 * 1024 * 1024;
const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

pub(crate) struct ImportGuard;

impl ImportGuard {
    pub(crate) fn acquire() -> Result<Self, String> {
        IMPORT_IN_PROGRESS
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self)
            .map_err(|_| "Import already in progress".to_string())
    }
}

impl Drop for ImportGuard {
    fn drop(&mut self) {
        IMPORT_IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct ImportPaths {
    staging_folder: PathBuf,
    final_folder: PathBuf,
    original_path: PathBuf,
    playback_path: PathBuf,
}

/// Removes unfinished media work on cancellation or any recoverable failure.
struct StagingImport {
    path: Option<PathBuf>,
}

impl StagingImport {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn commit(mut self, destination: &Path) -> Result<FinalizedImport> {
        let source = self.path.as_ref().expect("staging path exists");
        std::fs::rename(source, destination)
            .map_err(|error| anyhow!("Could not finalize the imported Session: {error}"))?;
        self.path.take();
        Ok(FinalizedImport {
            path: Some(destination.to_path_buf()),
        })
    }
}

/// Keeps a finalized folder reversible until its database row commits.
struct FinalizedImport {
    path: Option<PathBuf>,
}

impl FinalizedImport {
    fn keep(mut self) {
        self.path.take();
    }
}

impl Drop for FinalizedImport {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

impl Drop for StagingImport {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFileInfo {
    pub path: String,
    pub filename: String,
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgress {
    pub stage: String,
    pub progress_percentage: u32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub meeting_id: String,
    pub title: String,
    pub segments_count: usize,
    pub duration_seconds: f64,
    pub transcription_completed: bool,
    pub diarization_completed: bool,
    #[serde(skip)]
    pub folder_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportError {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStarted {
    pub message: String,
}

pub fn is_import_in_progress() -> bool {
    IMPORT_IN_PROGRESS.load(Ordering::SeqCst)
}

pub fn cancel_import() {
    IMPORT_CANCELLED.store(true, Ordering::SeqCst);
    super::retranscription::cancel_retranscription();
}

pub fn validate_audio_file(path: &Path) -> Result<AudioFileInfo> {
    if !path.is_file() {
        return Err(anyhow!(
            "The selected media file does not exist or is unavailable"
        ));
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !AUDIO_EXTENSIONS.contains(&extension.as_str()) {
        return Err(anyhow!(
            "Unsupported format .{}. Choose one of: {}",
            extension,
            AUDIO_EXTENSIONS.join(", ")
        ));
    }

    let size_bytes = std::fs::metadata(path)
        .map_err(|_| anyhow!("The selected media file could not be read"))?
        .len();
    if size_bytes == 0 {
        return Err(anyhow!("The selected media file is empty"));
    }
    if size_bytes > MAX_FILE_SIZE_BYTES {
        return Err(anyhow!("The selected media file exceeds the 20 GB limit"));
    }

    let mut header = [0_u8; 12];
    let bytes_read = File::open(path)
        .and_then(|mut file| file.read(&mut header))
        .map_err(|_| anyhow!("The selected media file could not be read"))?;
    if !signature_matches_extension(&header[..bytes_read], &extension) {
        return Err(anyhow!(
            "The file contents do not match the .{} extension",
            extension
        ));
    }

    let duration_seconds = extract_duration(path)?;
    let filename = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported media")
        .to_string();

    Ok(AudioFileInfo {
        path: path.to_string_lossy().to_string(),
        filename,
        duration_seconds,
        size_bytes,
        format: extension.to_uppercase(),
    })
}

fn signature_matches_extension(header: &[u8], extension: &str) -> bool {
    match extension {
        "wav" => header.starts_with(b"RIFF") && header.get(8..12) == Some(b"WAVE"),
        "flac" => header.starts_with(b"fLaC"),
        "ogg" => header.starts_with(b"OggS"),
        "mp3" => {
            header.starts_with(b"ID3")
                || header
                    .get(..2)
                    .is_some_and(|bytes| bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0)
        }
        "aac" => header
            .get(..2)
            .is_some_and(|bytes| bytes[0] == 0xff && bytes[1] & 0xf6 == 0xf0),
        "mp4" | "mov" | "m4v" | "m4a" => header.get(4..8) == Some(b"ftyp"),
        _ => false,
    }
}

fn extract_duration(path: &Path) -> Result<f64> {
    use symphonia::core::codecs::CODEC_TYPE_NULL;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = File::open(path)?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|_| anyhow!("The media file is damaged or unsupported"))?;
    let track = probed
        .format
        .tracks()
        .iter()
        .find(|track| {
            track.codec_params.codec != CODEC_TYPE_NULL
                && (track.codec_params.sample_rate.is_some()
                    || track.codec_params.channels.is_some())
        })
        .ok_or_else(|| anyhow!("No usable audio track was found in the media file"))?;
    if let (Some(sample_rate), Some(frames)) =
        (track.codec_params.sample_rate, track.codec_params.n_frames)
    {
        if sample_rate > 0 {
            return Ok(frames as f64 / sample_rate as f64);
        }
    }
    Ok(0.0)
}

fn create_import_paths(root: &Path, title: &str, extension: &str) -> Result<ImportPaths> {
    std::fs::create_dir_all(root)?;
    let import_id = Uuid::new_v4();
    let safe_title = sanitize_filename(title);
    let display_title = if safe_title.is_empty() {
        "Imported media"
    } else {
        &safe_title
    };
    let final_folder = root.join(format!(
        "{}_{}_{}",
        display_title,
        chrono::Utc::now().format("%Y-%m-%d_%H-%M"),
        &import_id.simple().to_string()[..8]
    ));
    let staging_folder = root.join(format!(".import-{import_id}.tmp"));
    std::fs::create_dir(&staging_folder)?;

    Ok(ImportPaths {
        original_path: staging_folder.join(format!("original.{extension}")),
        playback_path: staging_folder.join("audio.mp4"),
        staging_folder,
        final_folder,
    })
}

fn extract_playable_audio(source: &Path, destination: &Path) -> Result<()> {
    let ffmpeg = find_ffmpeg_path().map_err(|error| anyhow!(error.code()))?;
    let mut command = ffmpeg.command().map_err(|error| anyhow!(error.code()))?;
    let mut child = command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
        .arg(source)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
        ])
        .arg(destination)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| anyhow!("Could not start audio extraction"))?;

    let started = Instant::now();
    loop {
        if IMPORT_CANCELLED.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(destination);
            return Err(anyhow!("Import cancelled"));
        }
        if started.elapsed() >= EXTRACTION_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(destination);
            return Err(anyhow!("Audio extraction exceeded the two-hour limit"));
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => {
                let _ = std::fs::remove_file(destination);
                return Err(anyhow!(
                    "No usable audio track was found, or the media file is damaged"
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(destination);
                return Err(anyhow!("Audio extraction could not be completed"));
            }
        }
    }
    if std::fs::metadata(destination).map_or(true, |metadata| metadata.len() == 0) {
        return Err(anyhow!("The extracted audio track is empty"));
    }
    Ok(())
}

fn copy_original(source: &Path, destination: &Path) -> Result<()> {
    let mut input =
        File::open(source).map_err(|_| anyhow!("The original media could not be read"))?;
    let mut output =
        File::create(destination).map_err(|_| anyhow!("The original media could not be copied"))?;
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        fail_if_cancelled()?;
        let bytes_read = input
            .read(&mut buffer)
            .map_err(|_| anyhow!("The original media could not be read"))?;
        if bytes_read == 0 {
            break;
        }
        output
            .write_all(&buffer[..bytes_read])
            .map_err(|_| anyhow!("The original media could not be copied"))?;
    }
    output
        .sync_all()
        .map_err(|_| anyhow!("The original media copy could not be finalized"))
}

pub(crate) async fn start_import<R: Runtime>(
    app: AppHandle<R>,
    source_path: String,
    title: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<ImportResult> {
    let mut imported = match run_import(app.clone(), PathBuf::from(source_path), title).await {
        Ok(imported) => imported,
        Err(import_error) => {
            let _ = app.emit(
                "import-error",
                ImportError {
                    error: import_error.to_string(),
                },
            );
            return Err(import_error);
        }
    };

    if IMPORT_CANCELLED.load(Ordering::SeqCst) {
        let _ = app.emit("import-complete", &imported);
        return Ok(imported);
    }

    emit_progress(&app, "transcribing", 90, "Transcribing imported audio...");
    match super::retranscription::start_retranscription(
        app.clone(),
        imported.meeting_id.clone(),
        imported.folder_path.clone(),
        language,
        model,
        provider,
    )
    .await
    {
        Ok(result) => {
            imported.segments_count = result.segments_count;
            imported.transcription_completed = true;
            imported.diarization_completed = result.diarization_completed;
        }
        Err(error) => {
            // The imported media is already durable. Keep it available for a retry.
            log::warn!("Automatic transcription after import failed: {error}");
        }
    }

    emit_progress(&app, "complete", 100, "Import complete");
    let _ = app.emit("import-complete", &imported);
    Ok(imported)
}

async fn run_import<R: Runtime>(
    app: AppHandle<R>,
    source: PathBuf,
    title: String,
) -> Result<ImportResult> {
    let source_for_validation = source.clone();
    tokio::task::spawn_blocking(move || validate_audio_file(&source_for_validation))
        .await
        .map_err(|_| anyhow!("Media validation stopped unexpectedly"))??;
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| anyhow!("The selected media file has no supported extension"))?;

    emit_progress(&app, "copying", 10, "Preserving original media...");
    let paths = create_import_paths(&get_default_recordings_folder(), &title, &extension)?;
    let staging = StagingImport::new(paths.staging_folder.clone());
    let source_for_copy = source.clone();
    let original_for_copy = paths.original_path.clone();
    tokio::task::spawn_blocking(move || copy_original(&source_for_copy, &original_for_copy))
        .await
        .map_err(|_| anyhow!("The original media copy stopped unexpectedly"))??;

    emit_progress(&app, "extracting", 45, "Extracting playable audio...");
    let original_for_extraction = paths.original_path.clone();
    let playback_for_extraction = paths.playback_path.clone();
    tokio::task::spawn_blocking(move || {
        extract_playable_audio(&original_for_extraction, &playback_for_extraction)
    })
    .await
    .map_err(|_| anyhow!("Audio extraction stopped unexpectedly"))??;
    let duration_seconds = extract_duration(&paths.playback_path)?;
    fail_if_cancelled()?;

    let meeting_id = format!("meeting-{}", Uuid::new_v4());
    write_import_metadata(
        &paths.staging_folder,
        &meeting_id,
        &title,
        duration_seconds,
        &format!("original.{extension}"),
        matches!(extension.as_str(), "mp4" | "mov" | "m4v"),
    )?;

    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| anyhow!("App state not available"))?;
    emit_progress(&app, "saving", 85, "Creating Session...");
    let finalized = staging.commit(&paths.final_folder)?;

    let folder_path = paths.final_folder.to_string_lossy().to_string();
    create_imported_meeting(
        app_state.db_manager.pool(),
        &meeting_id,
        &title,
        &folder_path,
    )
    .await?;
    finalized.keep();

    Ok(ImportResult {
        meeting_id,
        title,
        segments_count: 0,
        duration_seconds,
        transcription_completed: false,
        diarization_completed: false,
        folder_path,
    })
}

fn fail_if_cancelled() -> Result<()> {
    if IMPORT_CANCELLED.load(Ordering::SeqCst) {
        Err(anyhow!("Import cancelled"))
    } else {
        Ok(())
    }
}

fn emit_progress<R: Runtime>(app: &AppHandle<R>, stage: &str, progress: u32, message: &str) {
    let _ = app.emit(
        "import-progress",
        ImportProgress {
            stage: stage.to_string(),
            progress_percentage: progress,
            message: message.to_string(),
        },
    );
}

async fn create_imported_meeting(
    pool: &sqlx::SqlitePool,
    meeting_id: &str,
    title: &str,
    folder_path: &str,
) -> Result<()> {
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at, folder_path)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(meeting_id)
    .bind(title)
    .bind(now)
    .bind(now)
    .bind(folder_path)
    .execute(pool)
    .await
    .map_err(|error| anyhow!("Could not create the imported Session: {error}"))?;
    Ok(())
}

fn write_import_metadata(
    folder: &Path,
    meeting_id: &str,
    title: &str,
    duration_seconds: f64,
    original_filename: &str,
    original_has_video: bool,
) -> Result<()> {
    let metadata_path = folder.join("metadata.json");
    let temporary_path = folder.join(".metadata.json.tmp");
    let now = chrono::Utc::now().to_rfc3339();
    let document = serde_json::json!({
        "version": "1.0",
        "meeting_id": meeting_id,
        "meeting_name": title,
        "created_at": now,
        "completed_at": now,
        "duration_seconds": duration_seconds,
        "audio_file": "audio.mp4",
        "original_file": original_filename,
        "original_has_video": original_has_video,
        "status": "completed",
        "source": "import"
    });
    std::fs::write(&temporary_path, serde_json::to_vec_pretty(&document)?)?;
    std::fs::rename(temporary_path, metadata_path)?;
    Ok(())
}

#[tauri::command]
pub async fn select_and_validate_audio_command<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<AudioFileInfo>, String> {
    info!("Opening file dialog for media import");
    let app_clone = app.clone();
    let selected = tokio::task::spawn_blocking(move || {
        app_clone
            .dialog()
            .file()
            .add_filter("Audio and Video", AUDIO_EXTENSIONS)
            .blocking_pick_file()
    })
    .await
    .map_err(|_| "The file picker could not be opened".to_string())?;

    let Some(path) = selected else {
        return Ok(None);
    };
    let path = PathBuf::from(path.to_string());
    tokio::task::spawn_blocking(move || validate_audio_file(&path))
        .await
        .map_err(|_| "Media validation stopped unexpectedly".to_string())?
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn validate_audio_file_command(path: String) -> Result<AudioFileInfo, String> {
    tokio::task::spawn_blocking(move || validate_audio_file(Path::new(&path)))
        .await
        .map_err(|_| "Media validation stopped unexpectedly".to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_import_audio_command<R: Runtime>(
    app: AppHandle<R>,
    source_path: String,
    title: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<ImportStarted, String> {
    let guard = ImportGuard::acquire()?;
    IMPORT_CANCELLED.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn(async move {
        // Held for the whole import; dropping it releases the single-import lock.
        let _guard = guard;
        if let Err(import_error) =
            start_import(app, source_path, title, language, model, provider).await
        {
            error!("Media import failed: {import_error}");
        }
    });
    Ok(ImportStarted {
        message: "Import started".to_string(),
    })
}

#[tauri::command]
pub async fn cancel_import_command() -> Result<(), String> {
    if !is_import_in_progress() {
        return Err("No import in progress".to_string());
    }
    cancel_import();
    Ok(())
}

#[tauri::command]
pub async fn is_import_in_progress_command() -> bool {
    is_import_in_progress()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_extensions_match_v1_contract() {
        assert_eq!(
            AUDIO_EXTENSIONS,
            &["mp4", "mov", "m4v", "m4a", "wav", "mp3", "flac", "ogg", "aac"]
        );
    }

    #[test]
    fn media_signature_must_match_extension_family() {
        assert!(signature_matches_extension(b"RIFF\0\0\0\0WAVE", "wav"));
        assert!(!signature_matches_extension(b"RIFF\0\0\0\0WAVE", "mp3"));
        assert!(signature_matches_extension(b"\0\0\0\x18ftypisom", "mov"));
        assert!(signature_matches_extension(b"fLaC", "flac"));
    }

    #[test]
    fn invalid_media_is_rejected_without_exposing_its_path() {
        let missing = validate_audio_file(Path::new("/private/customer/interview.wav"))
            .unwrap_err()
            .to_string();
        assert!(missing.contains("does not exist"));
        assert!(!missing.contains("customer"));

        let root = tempfile::tempdir().unwrap();
        let mislabeled = root.path().join("recording.mp3");
        std::fs::write(&mislabeled, b"RIFF\0\0\0\0WAVE").unwrap();
        assert!(validate_audio_file(&mislabeled)
            .unwrap_err()
            .to_string()
            .contains("do not match"));
    }

    #[test]
    fn repeated_imports_get_independent_destinations() {
        let root = tempfile::tempdir().unwrap();
        let first = create_import_paths(root.path(), "Weekly sync", "mov").unwrap();
        let second = create_import_paths(root.path(), "Weekly sync", "mov").unwrap();
        assert_ne!(first.final_folder, second.final_folder);
        assert_eq!(first.original_path.file_name().unwrap(), "original.mov");
        assert_eq!(first.playback_path.file_name().unwrap(), "audio.mp4");
    }

    #[test]
    fn failed_import_removes_staging_folder() {
        let root = tempfile::tempdir().unwrap();
        let paths = create_import_paths(root.path(), "Broken", "wav").unwrap();
        {
            let _staging = StagingImport::new(paths.staging_folder.clone());
            std::fs::write(&paths.original_path, b"broken").unwrap();
        }
        assert!(!paths.staging_folder.exists());
        assert!(!paths.final_folder.exists());
    }

    #[test]
    fn failed_finalization_removes_staging_folder() {
        let root = tempfile::tempdir().unwrap();
        let staging_path = root.path().join(".import.tmp");
        let destination = root.path().join("existing");
        std::fs::create_dir(&staging_path).unwrap();
        std::fs::write(staging_path.join("private-original.wav"), b"private").unwrap();
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("keep"), b"occupied").unwrap();

        let result = StagingImport::new(staging_path.clone()).commit(&destination);

        assert!(result.is_err());
        assert!(!staging_path.exists());
        assert!(destination.join("keep").is_file());
    }

    #[test]
    fn import_is_reserved_before_work_is_scheduled() {
        IMPORT_IN_PROGRESS.store(false, Ordering::SeqCst);
        let guard = ImportGuard::acquire().unwrap();
        assert!(ImportGuard::acquire().is_err());
        assert!(is_import_in_progress());
        drop(guard);
        assert!(!is_import_in_progress());
    }

    #[test]
    fn finalized_folder_is_removed_until_database_commit() {
        let root = tempfile::tempdir().unwrap();
        let paths = create_import_paths(root.path(), "Pending", "wav").unwrap();
        let staging = StagingImport::new(paths.staging_folder.clone());
        let finalized = staging.commit(&paths.final_folder).unwrap();
        assert!(paths.final_folder.exists());
        drop(finalized);
        assert!(!paths.final_folder.exists());
    }

    #[test]
    fn audio_import_preserves_original_and_creates_playable_audio() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.wav");
        write_test_wav(&source);
        let paths = create_import_paths(root.path(), "Audio", "wav").unwrap();
        let staging = StagingImport::new(paths.staging_folder.clone());

        std::fs::copy(&source, &paths.original_path).unwrap();
        extract_playable_audio(&paths.original_path, &paths.playback_path).unwrap();
        assert_eq!(
            std::fs::read(&source).unwrap(),
            std::fs::read(&paths.original_path).unwrap()
        );
        assert!(
            decode_audio_file(&paths.playback_path)
                .unwrap()
                .duration_seconds
                > 0.9
        );

        staging.commit(&paths.final_folder).unwrap().keep();
        assert!(paths.final_folder.join("original.wav").is_file());
        assert!(paths.final_folder.join("audio.mp4").is_file());
    }

    #[test]
    fn video_import_uses_only_its_audio_track() {
        let root = tempfile::tempdir().unwrap();
        let source_audio = root.path().join("source.wav");
        let source_video = root.path().join("source.mov");
        write_test_wav(&source_audio);
        create_test_video(&source_audio, &source_video);

        let info = validate_audio_file(&source_video).unwrap();
        assert_eq!(info.format, "MOV");
        let derived = root.path().join("derived.mp4");
        extract_playable_audio(&source_video, &derived).unwrap();
        assert!(decode_audio_file(&derived).unwrap().duration_seconds > 0.9);
    }

    #[test]
    fn audio_less_video_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let source_video = root.path().join("silent.m4v");
        let ffmpeg = find_ffmpeg_path().unwrap();
        let status = ffmpeg
            .command()
            .unwrap()
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=black:s=16x16:d=1",
                "-an",
                "-c:v",
                "mpeg4",
            ])
            .arg(&source_video)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(validate_audio_file(&source_video)
            .unwrap_err()
            .to_string()
            .contains("No usable audio track"));
    }

    #[test]
    fn metadata_marks_video_as_preserved_but_audio_first() {
        let root = tempfile::tempdir().unwrap();
        write_import_metadata(
            root.path(),
            "meeting-123",
            "Imported video",
            42.0,
            "original.mov",
            true,
        )
        .unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.path().join("metadata.json")).unwrap())
                .unwrap();
        assert_eq!(document["audio_file"], "audio.mp4");
        assert_eq!(document["original_file"], "original.mov");
        assert_eq!(document["original_has_video"], true);
        assert!(document.get("video_file").is_none());
    }

    fn write_test_wav(path: &Path) {
        let sample_rate = 16_000_u32;
        let samples: Vec<i16> = (0..sample_rate)
            .map(|index| {
                let phase = index as f32 * 440.0 * std::f32::consts::TAU / sample_rate as f32;
                (phase.sin() * i16::MAX as f32 * 0.1) as i16
            })
            .collect();
        let data_size = (samples.len() * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_size as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_size.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn create_test_video(audio: &Path, destination: &Path) {
        let ffmpeg = find_ffmpeg_path().unwrap();
        let status = ffmpeg
            .command()
            .unwrap()
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=black:s=16x16:d=1",
                "-i",
            ])
            .arg(audio)
            .args(["-shortest", "-c:v", "mpeg4", "-c:a", "aac"])
            .arg(destination)
            .status()
            .unwrap();
        assert!(status.success());
    }
}
