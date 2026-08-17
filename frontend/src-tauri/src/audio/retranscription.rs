// Retranscription module - allows re-processing stored audio with different settings

use super::common::{create_transcript_segments, split_segment_at_silence, write_transcripts_json};
use super::constants::AUDIO_EXTENSIONS;
use crate::audio::decoder::decode_audio_file_for_transcription;
use crate::audio::diarization::{diarize_file, speaker_label_for_interval, SpeakerSegment};
use crate::audio::vad::get_speech_chunks_with_progress;
use crate::config::{DEFAULT_PARAKEET_MODEL, DEFAULT_WHISPER_MODEL};
use crate::database::models::ProcessingJob;
use crate::database::repositories::processing_job::{
    ProcessingJobError, ProcessingJobWarning, ProcessingJobsRepository, StartProcessingJob,
};
use crate::parakeet_engine::ParakeetEngine;
use crate::state::AppState;
use crate::whisper_engine::WhisperEngine;
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use unicode_normalization::UnicodeNormalization;

const RETRANSCRIPTION_IDLE: u8 = 0;
const RETRANSCRIPTION_PROCESSING: u8 = 1;
const RETRANSCRIPTION_CANCEL_REQUESTED: u8 = 2;
const RETRANSCRIPTION_SAVING: u8 = 3;
static RETRANSCRIPTION_STATE: AtomicU8 = AtomicU8::new(RETRANSCRIPTION_IDLE);

/// RAII guard for the retranscription state.
/// Ensures flag is cleared even if retranscription panics or returns early
struct RetranscriptionGuard;

impl RetranscriptionGuard {
    /// Create guard and set flag atomically
    fn acquire() -> Result<Self, String> {
        if RETRANSCRIPTION_STATE
            .compare_exchange(
                RETRANSCRIPTION_IDLE,
                RETRANSCRIPTION_PROCESSING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            return Err("Retranscription already in progress".to_string());
        }
        Ok(RetranscriptionGuard)
    }
}

impl Drop for RetranscriptionGuard {
    fn drop(&mut self) {
        RETRANSCRIPTION_STATE.store(RETRANSCRIPTION_IDLE, Ordering::SeqCst);
    }
}

/// VAD redemption time in milliseconds - bridges natural pauses in speech
/// Batch processing needs longer redemption (2000ms) than live pipeline (400ms)
/// because the entire file is processed at once by VAD, and 400ms fragments
/// speech at every natural sentence/topic pause (500ms-2s)
const VAD_REDEMPTION_TIME_MS: u32 = 2000;

/// Progress update emitted during retranscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionProgress {
    pub meeting_id: String,
    pub stage: String, // "decoding", "transcribing", "saving"
    pub progress_percentage: u32,
    pub message: String,
}

/// Result of retranscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionResult {
    pub meeting_id: String,
    pub segments_count: usize,
    pub duration_seconds: f64,
    pub language: Option<String>,
    pub diarization_completed: bool,
    pub failed_origins: Vec<String>,
}

/// Error during retranscription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionError {
    pub meeting_id: String,
    pub code: String,
    pub error: String,
}

/// Check if retranscription is currently in progress
pub fn is_retranscription_in_progress() -> bool {
    RETRANSCRIPTION_STATE.load(Ordering::SeqCst) != RETRANSCRIPTION_IDLE
}

/// Cancel ongoing retranscription
pub fn cancel_retranscription() {
    let _ = RETRANSCRIPTION_STATE.compare_exchange(
        RETRANSCRIPTION_PROCESSING,
        RETRANSCRIPTION_CANCEL_REQUESTED,
        Ordering::SeqCst,
        Ordering::SeqCst,
    );
}

/// Start retranscription of a meeting's audio
pub async fn start_retranscription<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
    _meeting_folder_path: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionResult> {
    let (job, folder, origins) = reserve_processing_job(
        &app,
        &meeting_id,
        language.as_ref(),
        model.as_ref(),
        provider.as_ref(),
    )
    .await
    .map_err(|error| anyhow!(error))?;
    execute_reserved_retranscription(app, job, folder, origins, language, model, provider).await
}

async fn execute_reserved_retranscription<R: Runtime>(
    app: AppHandle<R>,
    job: ProcessingJob,
    folder: PathBuf,
    origins: Vec<AudioOrigin>,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionResult> {
    let _guard = match RetranscriptionGuard::acquire() {
        Ok(guard) => guard,
        Err(_) => {
            if let Some(state) = app.try_state::<AppState>() {
                let _ = ProcessingJobsRepository::fail(
                    state.db_manager.pool(),
                    &job.id,
                    ProcessingJobError::Duplicate,
                )
                .await;
            }
            return Err(anyhow!(ProcessingJobError::Duplicate));
        }
    };

    let use_parakeet = provider.as_deref() == Some("parakeet");
    let meeting_id = job.meeting_id.clone();
    let result = run_retranscription(
        app.clone(),
        job.clone(),
        folder,
        origins,
        language,
        model,
        provider,
    )
    .await;

    // Unload the engine after the batch job (success, failure, or cancellation)
    super::common::unload_transcription_engine(use_parakeet).await;

    match &result {
        Ok(res) => {
            let _ = app.emit(
                "retranscription-complete",
                serde_json::json!({
                    "meeting_id": res.meeting_id,
                    "segments_count": res.segments_count,
                    "duration_seconds": res.duration_seconds,
                    "language": res.language,
                    "diarization_completed": res.diarization_completed,
                    "failed_origins": res.failed_origins
                }),
            );
        }
        Err(e) => {
            let public_error = classify_processing_error(e);
            if let Some(state) = app.try_state::<AppState>() {
                let _ = ProcessingJobsRepository::fail(
                    state.db_manager.pool(),
                    &job.id,
                    public_error.clone(),
                )
                .await;
            }
            let _ = app.emit(
                "retranscription-error",
                RetranscriptionError {
                    meeting_id: meeting_id.clone(),
                    code: public_error.code().to_string(),
                    error: public_error.to_string(),
                },
            );
        }
    }

    result
}

fn classify_processing_error(error: &anyhow::Error) -> ProcessingJobError {
    error
        .downcast_ref::<ProcessingJobError>()
        .cloned()
        .unwrap_or(ProcessingJobError::ProcessingFailed)
}

/// Find audio file in meeting folder
/// Tries common names first, then scans for any file with an audio extension
fn find_audio_file(folder: &Path) -> Result<PathBuf> {
    let candidates = [
        "audio.mp4",
        "audio.m4a",
        "audio.wav",
        "audio.mp3",
        "audio.flac",
        "audio.ogg",
        "recording.mp4",
        "audio.mkv",
        "audio.webm",
        "audio.wma",
    ];

    for name in candidates {
        let path = folder.join(name);
        if path.exists() {
            return trusted_media_path(folder, &path);
        }
    }

    // Fallback: scan folder for any file with an audio extension
    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_lowercase();
                if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                    return trusted_media_path(folder, &path);
                }
            }
        }
    }

    Err(anyhow!("No audio file found in: {}", folder.display()))
}

fn trusted_media_path(folder: &Path, path: &Path) -> Result<PathBuf> {
    let canonical_folder = folder.canonicalize()?;
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&canonical_folder) || !canonical_path.is_file() {
        return Err(anyhow!("Audio source is outside the recording folder"));
    }
    Ok(path.to_path_buf())
}

fn validate_recording_folder(root: &Path, folder: &Path) -> Result<PathBuf> {
    let canonical_root = root.canonicalize()?;
    let canonical_folder = folder.canonicalize()?;
    if !canonical_folder.starts_with(&canonical_root) || !canonical_folder.is_dir() {
        return Err(anyhow!("Recording folder is outside the managed library"));
    }
    Ok(canonical_folder)
}

async fn resolve_meeting_folder<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
) -> Result<PathBuf> {
    let state = app
        .try_state::<AppState>()
        .ok_or_else(|| anyhow!("App state not available"))?;
    let stored_path: Option<String> =
        sqlx::query_scalar("SELECT folder_path FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(state.db_manager.pool())
            .await?
            .flatten();
    let stored_path = stored_path.ok_or_else(|| anyhow!("Recording folder is unavailable"))?;
    validate_recording_folder(
        &super::recording_preferences::get_default_recordings_folder(),
        Path::new(&stored_path),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioOrigin {
    kind: &'static str,
    path: PathBuf,
}

struct AudioOriginDiscovery {
    available: Vec<AudioOrigin>,
    missing: Vec<String>,
    manifest_hash: String,
}

fn input_manifest_hash(origins: &[AudioOrigin]) -> Result<String> {
    let expected = origins.iter().map(|origin| origin.kind).collect::<Vec<_>>();
    audio_origin_manifest_hash(&expected, origins)
}

fn audio_origin_manifest_hash(
    expected: &[&'static str],
    origins: &[AudioOrigin],
) -> Result<String> {
    let mut ordered = expected.to_vec();
    ordered.sort_unstable();
    ordered.dedup();
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    for kind in ordered {
        hasher.update((kind.len() as u64).to_le_bytes());
        hasher.update(kind.as_bytes());
        let Some(origin) = origins.iter().find(|origin| origin.kind == kind) else {
            hasher.update([0]);
            continue;
        };
        hasher.update([1]);
        let mut file = std::fs::File::open(&origin.path)?;
        hasher.update(file.metadata()?.len().to_le_bytes());
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Clone, Copy)]
struct DeclaredCapturedOrigin {
    kind: &'static str,
    stored: bool,
}

fn declared_captured_origins(folder: &Path) -> Result<Vec<DeclaredCapturedOrigin>> {
    let metadata_path = folder.join("metadata.json");
    if !metadata_path.exists() {
        return Ok(Vec::new());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(metadata_path)?)?;
    let Some(origins_value) = metadata.get("origins") else {
        return Ok(Vec::new());
    };
    let origins = origins_value
        .as_array()
        .ok_or_else(|| anyhow!("Recording origins must be an array"))?;
    let mut declared = Vec::with_capacity(origins.len());
    for origin in origins {
        let object = origin
            .as_object()
            .ok_or_else(|| anyhow!("Recording origin must be an object"))?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "kind" | "file" | "status"))
        {
            return Err(anyhow!("Recording origin has an unknown field"));
        }
        let (kind, expected_file) = match object.get("kind").and_then(serde_json::Value::as_str) {
            Some("microphone") => ("microphone", "microphone.mp4"),
            Some("system-audio") => ("system-audio", "system-audio.mp4"),
            _ => return Err(anyhow!("Recording origin kind is unsupported")),
        };
        if object.get("file").and_then(serde_json::Value::as_str) != Some(expected_file) {
            return Err(anyhow!("Recording origin file does not match its kind"));
        }
        let stored = match object.get("status").and_then(serde_json::Value::as_str) {
            Some("stored") => true,
            Some("partial" | "unavailable") => false,
            _ => return Err(anyhow!("Recording origin status is unsupported")),
        };
        if declared
            .iter()
            .any(|existing: &DeclaredCapturedOrigin| existing.kind == kind)
        {
            return Err(anyhow!("Recording origin kind is duplicated"));
        }
        declared.push(DeclaredCapturedOrigin { kind, stored });
    }
    declared.sort_unstable_by_key(|origin| origin.kind);
    Ok(declared)
}

fn discover_audio_origins(folder: &Path) -> Result<AudioOriginDiscovery> {
    let declared = declared_captured_origins(folder)?;
    let detected = [
        ("microphone", folder.join("microphone.mp4")),
        ("system-audio", folder.join("system-audio.mp4")),
    ]
    .into_iter()
    .filter(|(kind, path)| {
        path.exists()
            && (declared.is_empty()
                || declared
                    .iter()
                    .any(|origin| origin.kind == *kind && origin.stored))
    })
    .map(|(kind, path)| trusted_media_path(folder, &path).map(|path| AudioOrigin { kind, path }))
    .collect::<Result<Vec<_>>>()?;

    let expected = if declared.is_empty() {
        detected
            .iter()
            .map(|origin| origin.kind)
            .collect::<Vec<_>>()
    } else {
        declared.iter().map(|origin| origin.kind).collect()
    };
    if !expected.is_empty() {
        let missing = expected
            .iter()
            .filter(|kind| !detected.iter().any(|origin| origin.kind == **kind))
            .map(|kind| (*kind).to_string())
            .collect();
        let manifest_hash = audio_origin_manifest_hash(&expected, &detected)?;
        return Ok(AudioOriginDiscovery {
            available: detected,
            missing,
            manifest_hash,
        });
    }

    let imported = AudioOrigin {
        kind: "imported",
        path: find_audio_file(folder)?,
    };
    let manifest_hash = input_manifest_hash(std::slice::from_ref(&imported))?;
    Ok(AudioOriginDiscovery {
        available: vec![imported],
        missing: Vec::new(),
        manifest_hash,
    })
}

async fn reserve_processing_job<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
    language: Option<&String>,
    model: Option<&String>,
    provider: Option<&String>,
) -> Result<(ProcessingJob, PathBuf, Vec<AudioOrigin>), ProcessingJobError> {
    let folder = resolve_meeting_folder(app, meeting_id)
        .await
        .map_err(|_| ProcessingJobError::InvalidInput)?;
    let discovery =
        discover_audio_origins(&folder).map_err(|_| ProcessingJobError::InvalidInput)?;
    if discovery.available.is_empty() {
        return Err(ProcessingJobError::InvalidInput);
    }
    let origins = discovery.available;
    let provider = provider.map(String::as_str).unwrap_or("whisper");
    let model = model
        .map(String::as_str)
        .unwrap_or(if provider == "parakeet" {
            DEFAULT_PARAKEET_MODEL
        } else {
            DEFAULT_WHISPER_MODEL
        });
    let state = app
        .try_state::<AppState>()
        .ok_or(ProcessingJobError::Storage)?;
    let job = ProcessingJobsRepository::start(
        state.db_manager.pool(),
        StartProcessingJob {
            meeting_id: meeting_id.to_string(),
            language: language.cloned(),
            provider: provider.to_string(),
            model: model.to_string(),
            input_manifest_hash: discovery.manifest_hash,
            source_origins: origins
                .iter()
                .map(|origin| origin.kind.to_string())
                .chain(discovery.missing)
                .collect(),
        },
    )
    .await?;
    Ok((job, folder, origins))
}

fn load_or_snapshot_microphone_participants(
    folder: &Path,
    preferences: &super::recording_preferences::RecordingPreferences,
) -> Result<super::recording_preferences::MicrophoneParticipantSnapshot> {
    let metadata_path = folder.join("metadata.json");
    let mut metadata = if metadata_path.exists() {
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&metadata_path)?)?
    } else {
        serde_json::json!({})
    };
    let object = metadata
        .as_object_mut()
        .ok_or_else(|| anyhow!("Recording metadata must be a JSON object"))?;

    if let Some(snapshot) = object.get("microphone_participants") {
        return Ok(serde_json::from_value(snapshot.clone())?);
    }

    let snapshot =
        super::recording_preferences::MicrophoneParticipantSnapshot::from_preferences(preferences);
    object.insert(
        "microphone_participants".to_string(),
        serde_json::to_value(&snapshot)?,
    );
    let temporary_path = folder.join(format!(
        ".metadata.participants.{}.tmp",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&temporary_path, serde_json::to_vec_pretty(&metadata)?)?;
    std::fs::rename(&temporary_path, &metadata_path)?;
    Ok(snapshot)
}

#[cfg(test)]
fn find_audio_origins(folder: &Path) -> Result<Vec<AudioOrigin>> {
    let discovery = discover_audio_origins(folder)?;
    if discovery.available.is_empty() {
        Err(anyhow!("No declared audio origin is available"))
    } else {
        Ok(discovery.available)
    }
}

fn merge_origin_timelines(
    timelines: Vec<Vec<crate::api::TranscriptSegment>>,
) -> Vec<crate::api::TranscriptSegment> {
    let mut segments = timelines.into_iter().flatten().collect::<Vec<_>>();
    segments.sort_by(|left, right| {
        left.audio_start_time
            .unwrap_or(f64::MAX)
            .total_cmp(&right.audio_start_time.unwrap_or(f64::MAX))
            .then_with(|| {
                left.audio_end_time
                    .unwrap_or(f64::MAX)
                    .total_cmp(&right.audio_end_time.unwrap_or(f64::MAX))
            })
            .then_with(|| left.source_origin.cmp(&right.source_origin))
    });
    mark_ambiguous_duplicates(&mut segments);
    segments
}

fn mark_ambiguous_duplicates(segments: &mut [crate::api::TranscriptSegment]) {
    // ponytail: O(n²) exact-text comparison is adequate for two V1 origins; use an
    // interval index if transcript blocks become numerous enough to measure a slowdown.
    for left_index in 0..segments.len() {
        for right_index in (left_index + 1)..segments.len() {
            let (left, right) = {
                let (head, tail) = segments.split_at_mut(right_index);
                (&mut head[left_index], &mut tail[0])
            };
            if left.source_origin == right.source_origin
                || normalized_text(&left.text) != normalized_text(&right.text)
                || !time_ranges_overlap(left, right)
            {
                continue;
            }

            let key = format!(
                "{}|{:.3}|{:.3}",
                normalized_text(&left.text),
                left.audio_start_time
                    .unwrap_or_default()
                    .min(right.audio_start_time.unwrap_or_default()),
                left.audio_end_time
                    .unwrap_or_default()
                    .max(right.audio_end_time.unwrap_or_default())
            );
            let stable_hash = key
                .as_bytes()
                .iter()
                .fold(0xcbf29ce484222325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                });
            let group_id = format!("ambiguity-{stable_hash:016x}");
            left.ambiguity_group_id = Some(group_id.clone());
            right.ambiguity_group_id = Some(group_id);
            left.alignment_decision = Some("preserved-ambiguous-duplicate".to_string());
            right.alignment_decision = Some("preserved-ambiguous-duplicate".to_string());
        }
    }
}

fn normalized_text(text: &str) -> String {
    text.nfc()
        .flat_map(char::to_lowercase)
        .nfc()
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn time_ranges_overlap(
    left: &crate::api::TranscriptSegment,
    right: &crate::api::TranscriptSegment,
) -> bool {
    match (
        left.audio_start_time,
        left.audio_end_time,
        right.audio_start_time,
        right.audio_end_time,
    ) {
        (Some(left_start), Some(left_end), Some(right_start), Some(right_end)) => {
            left_start <= right_end && right_start <= left_end
        }
        _ => false,
    }
}

fn assign_microphone_speaker(segments: &mut [crate::api::TranscriptSegment], speaker_name: &str) {
    for segment in segments {
        segment.speaker = Some(speaker_name.to_string());
        segment.speaker_cluster_id = Some("microphone:local".to_string());
    }
}

fn assign_origin_speaker_clusters(
    segments: &mut [crate::api::TranscriptSegment],
    origin: &AudioOrigin,
) {
    for segment in segments {
        if let Some(speaker) = &segment.speaker {
            segment.speaker_cluster_id = Some(format!("{}:{speaker}", origin.kind));
        }
    }
}

struct ProcessedOrigin {
    segments: Vec<crate::api::TranscriptSegment>,
    duration_seconds: f64,
    speaker_labels_completed: bool,
}

fn combine_processed_origins(
    processed: Vec<ProcessedOrigin>,
    had_failures: bool,
) -> Result<ProcessedOrigin> {
    if processed.is_empty() {
        return Err(anyhow!("No audio origin could be transcribed"));
    }

    let duration_seconds = processed
        .iter()
        .map(|origin| origin.duration_seconds)
        .fold(0.0_f64, f64::max);
    let speaker_labels_completed = !had_failures
        && processed
            .iter()
            .all(|origin| origin.speaker_labels_completed);
    let segments = merge_origin_timelines(
        processed
            .into_iter()
            .map(|origin| origin.segments)
            .collect(),
    );

    Ok(ProcessedOrigin {
        segments,
        duration_seconds,
        speaker_labels_completed,
    })
}

fn must_preserve_existing_transcript(failed_origins: &[String], existing_count: i64) -> bool {
    !failed_origins.is_empty() && existing_count > 0
}

/// Internal function to run retranscription
async fn run_retranscription<R: Runtime>(
    app: AppHandle<R>,
    job: ProcessingJob,
    folder_path: PathBuf,
    origins: Vec<AudioOrigin>,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionResult> {
    let meeting_id = job.meeting_id.clone();
    let recording_preferences = super::recording_preferences::load_recording_preferences(&app)
        .await
        .unwrap_or_default();
    let microphone_speaker_name = if origins.iter().any(|origin| origin.kind == "microphone") {
        load_or_snapshot_microphone_participants(&folder_path, &recording_preferences)?
            .individual_speaker_name()
    } else {
        None
    };

    // Determine which provider to use (default to whisper)
    let use_parakeet = provider.as_deref() == Some("parakeet");

    info!(
        "Starting retranscription for meeting {} with language {:?}, model {:?}, provider {:?}",
        meeting_id, language, model, provider
    );

    let mut processed_origins = Vec::with_capacity(origins.len());
    let mut failed_origins = job
        .source_origins
        .iter()
        .filter(|kind| !origins.iter().any(|origin| origin.kind == kind.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    for (origin_index, origin) in origins.iter().enumerate() {
        let result = process_audio_origin(
            &app,
            &meeting_id,
            origin,
            language.clone(),
            model.as_deref(),
            use_parakeet,
            origin_index,
            origins.len(),
            microphone_speaker_name.as_deref(),
        )
        .await;

        match result {
            Ok(processed) => processed_origins.push(processed),
            Err(error)
                if RETRANSCRIPTION_STATE.load(Ordering::SeqCst)
                    == RETRANSCRIPTION_CANCEL_REQUESTED =>
            {
                return Err(error)
            }
            Err(error) => {
                failed_origins.push(origin.kind.to_string());
                warn!(
                    "Skipping unavailable {} audio origin while preserving the others: {error}",
                    origin.kind
                );
            }
        }
    }

    let combined = combine_processed_origins(processed_origins, !failed_origins.is_empty())?;
    let segments = combined.segments;
    let duration_seconds = combined.duration_seconds;
    let diarization_completed = combined.speaker_labels_completed;

    emit_progress(&app, &meeting_id, "saving", 85, "Saving transcripts...");

    // Save to database
    let app_state = app
        .try_state::<AppState>()
        .ok_or_else(|| anyhow!("App state not available"))?;

    // Wrap delete+insert+update in a transaction to prevent data loss
    let pool = app_state.db_manager.pool();
    if !failed_origins.is_empty() {
        let existing_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM transcripts WHERE meeting_id = ?")
                .bind(&meeting_id)
                .fetch_one(pool)
                .await?;
        if must_preserve_existing_transcript(&failed_origins, existing_count) {
            return Err(anyhow!(
                "Preserved the existing transcript because these audio origins failed: {}",
                failed_origins.join(", ")
            ));
        }
    }
    if discover_audio_origins(&folder_path)?.manifest_hash != job.input_manifest_hash {
        return Err(anyhow!(ProcessingJobError::SourceChanged));
    }
    begin_non_cancellable_save()?;
    let warning = if !failed_origins.is_empty() {
        Some(ProcessingJobWarning::PartialOriginUnavailable)
    } else if !diarization_completed {
        Some(ProcessingJobWarning::SpeakerLabelsUnavailable)
    } else {
        None
    };
    if let Some(warning) = warning {
        ProcessingJobsRepository::warn(pool, &job.id, warning)
            .await
            .map_err(|error| anyhow!(error))?;
    }
    ProcessingJobsRepository::complete_with_transcript(pool, &job.id, &segments)
        .await
        .map_err(|error| anyhow!(error))?;

    info!(
        "Updated {} transcripts for meeting {} in transaction",
        segments.len(),
        meeting_id
    );

    // Write updated transcripts.json and metadata.json to the meeting folder
    emit_progress(
        &app,
        &meeting_id,
        "saving",
        92,
        "Writing transcript files...",
    );

    if let Err(e) = write_transcripts_json(&folder_path, &segments) {
        warn!("Failed to write transcripts.json: {}", e);
    }

    // Find audio filename for metadata
    let audio_filename = find_audio_file(&folder_path)
        .unwrap_or_else(|_| origins[0].path.clone())
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.mp4")
        .to_string();

    if let Err(e) = write_retranscription_metadata(
        &folder_path,
        &meeting_id,
        duration_seconds,
        &audio_filename,
        &failed_origins,
    ) {
        warn!("Failed to update metadata.json: {}", e);
    }

    emit_progress(
        &app,
        &meeting_id,
        "complete",
        100,
        "Retranscription complete",
    );

    Ok(RetranscriptionResult {
        meeting_id,
        segments_count: segments.len(),
        duration_seconds,
        language,
        diarization_completed,
        failed_origins,
    })
}

// Per-origin processing keeps the existing cancellation, language, and identity inputs explicit.
#[allow(clippy::too_many_arguments)]
async fn process_audio_origin<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
    origin: &AudioOrigin,
    language: Option<String>,
    model: Option<&str>,
    use_parakeet: bool,
    origin_index: usize,
    origin_count: usize,
    microphone_speaker_name: Option<&str>,
) -> Result<ProcessedOrigin> {
    ensure_not_cancelled()?;
    emit_origin_progress(
        app,
        meeting_id,
        "decoding",
        5,
        origin,
        origin_index,
        origin_count,
        "Decoding audio...",
    );

    let path = origin.path.clone();
    let decoded = tokio::task::spawn_blocking(move || decode_audio_file_for_transcription(&path))
        .await
        .map_err(|error| anyhow!("Decode task panicked: {error}"))??;
    let duration_seconds = decoded.duration_seconds;

    ensure_not_cancelled()?;
    let audio_samples = tokio::task::spawn_blocking(move || decoded.to_whisper_format())
        .await
        .map_err(|error| anyhow!("Resample task panicked: {error}"))?;

    emit_origin_progress(
        app,
        meeting_id,
        "vad",
        20,
        origin,
        origin_index,
        origin_count,
        "Detecting speech...",
    );
    ensure_not_cancelled()?;
    let app_for_vad = app.clone();
    let meeting_id_for_vad = meeting_id.to_string();
    let origin_for_vad = origin.clone();
    let speech_segments = tokio::task::spawn_blocking(move || {
        get_speech_chunks_with_progress(
            &audio_samples,
            VAD_REDEMPTION_TIME_MS,
            |progress, segments_found| {
                emit_origin_progress(
                    &app_for_vad,
                    &meeting_id_for_vad,
                    "vad",
                    20 + progress * 5 / 100,
                    &origin_for_vad,
                    origin_index,
                    origin_count,
                    &format!("Detecting speech... {segments_found} segments"),
                );
                RETRANSCRIPTION_STATE.load(Ordering::SeqCst) == RETRANSCRIPTION_PROCESSING
            },
        )
    })
    .await
    .map_err(|error| anyhow!("VAD task panicked: {error}"))??;
    if speech_segments.is_empty() {
        return Err(anyhow!("No speech detected in {} audio", origin.kind));
    }

    const MAX_SEGMENT_SAMPLES: usize = 25 * 16_000;
    let processable_segments = speech_segments
        .iter()
        .flat_map(|segment| {
            if segment.samples.len() > MAX_SEGMENT_SAMPLES {
                split_segment_at_silence(segment, MAX_SEGMENT_SAMPLES)
            } else {
                vec![segment.clone()]
            }
        })
        .collect::<Vec<_>>();

    emit_origin_progress(
        app,
        meeting_id,
        "transcribing",
        25,
        origin,
        origin_index,
        origin_count,
        "Loading transcription engine...",
    );
    let whisper_engine = if use_parakeet {
        None
    } else {
        Some(get_or_init_whisper(app, model).await?)
    };
    let parakeet_engine = if use_parakeet {
        Some(get_or_init_parakeet(app, model).await?)
    } else {
        None
    };

    let mut transcripts = Vec::new();
    for (index, segment) in processable_segments.iter().enumerate() {
        ensure_not_cancelled()?;
        if segment.samples.len() < 1_600 {
            continue;
        }
        let local_progress =
            25 + ((index as f32 / processable_segments.len() as f32) * 50.0) as u32;
        emit_origin_progress(
            app,
            meeting_id,
            "transcribing",
            local_progress,
            origin,
            origin_index,
            origin_count,
            &format!(
                "Transcribing segment {} of {}...",
                index + 1,
                processable_segments.len()
            ),
        );

        let text = if let Some(engine) = &parakeet_engine {
            engine
                .transcribe_audio(segment.samples.clone())
                .await
                .map_err(|error| anyhow!("Parakeet transcription failed: {error}"))?
        } else {
            whisper_engine
                .as_ref()
                .ok_or_else(|| anyhow!("Whisper engine is unavailable"))?
                .transcribe_audio_with_confidence(segment.samples.clone(), language.clone())
                .await
                .map_err(|error| anyhow!("Whisper transcription failed: {error}"))?
                .0
        };
        if !text.trim().is_empty() {
            transcripts.push((text, segment.start_timestamp_ms, segment.end_timestamp_ms));
        }
    }

    let mut segments = create_transcript_segments(&transcripts);
    for segment in &mut segments {
        segment.source_origin = Some(origin.kind.to_string());
    }
    if segments.is_empty() {
        return Err(anyhow!(
            "Transcription returned no text for {} audio",
            origin.kind
        ));
    }

    let needs_diarization = !(origin.kind == "microphone" && microphone_speaker_name.is_some());
    if needs_diarization {
        super::common::unload_transcription_engine(use_parakeet).await;
        release_unused_allocator_pages();
    }

    let speaker_labels_completed =
        if let ("microphone", Some(name)) = (origin.kind, microphone_speaker_name) {
            assign_microphone_speaker(&mut segments, name);
            true
        } else {
            emit_origin_progress(
                app,
                meeting_id,
                "diarizing",
                80,
                origin,
                origin_index,
                origin_count,
                "Identifying speakers...",
            );
            let completed = match apply_diarization_result(
                &mut segments,
                diarize_file(origin.path.clone()).await,
            ) {
                Ok(_) => true,
                Err(error) => {
                    warn!(
                        "Speaker diarization unavailable for {}: {error}",
                        origin.kind
                    );
                    false
                }
            };
            assign_origin_speaker_clusters(&mut segments, origin);
            completed
        };

    // ponytail: FluidAudio cannot stop mid-inference; cancellation still prevents persistence.
    ensure_not_cancelled()?;
    Ok(ProcessedOrigin {
        segments,
        duration_seconds,
        speaker_labels_completed,
    })
}

#[cfg(target_os = "macos")]
fn release_unused_allocator_pages() {
    unsafe extern "C" {
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }

    let released = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
    debug!("Released {released} bytes of unused allocator pages before diarization");
}

#[cfg(not(target_os = "macos"))]
fn release_unused_allocator_pages() {}

fn ensure_not_cancelled() -> Result<()> {
    if RETRANSCRIPTION_STATE.load(Ordering::SeqCst) == RETRANSCRIPTION_CANCEL_REQUESTED {
        Err(anyhow!(ProcessingJobError::Cancelled))
    } else {
        Ok(())
    }
}

fn begin_non_cancellable_save() -> Result<()> {
    RETRANSCRIPTION_STATE
        .compare_exchange(
            RETRANSCRIPTION_PROCESSING,
            RETRANSCRIPTION_SAVING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .map(|_| ())
        .map_err(|_| anyhow!(ProcessingJobError::Cancelled))
}

#[allow(clippy::too_many_arguments)]
fn emit_origin_progress<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
    stage: &str,
    local_progress: u32,
    origin: &AudioOrigin,
    origin_index: usize,
    origin_count: usize,
    message: &str,
) {
    let completed = origin_index as u32 * 100 + local_progress.min(100);
    let total = origin_count.max(1) as u32 * 100;
    let overall_progress = 5 + completed * 80 / total;
    emit_progress(
        app,
        meeting_id,
        stage,
        overall_progress,
        &format!("{}: {message}", origin.kind),
    );
}

fn apply_diarization_result(
    segments: &mut [crate::api::TranscriptSegment],
    result: Result<Vec<SpeakerSegment>>,
) -> Result<usize> {
    let speaker_segments = result?;
    if speaker_segments.is_empty() {
        return Err(anyhow!("Speaker diarization returned no speakers"));
    }
    if speaker_segments.iter().any(|segment| {
        segment.speaker_id.trim().is_empty()
            || !segment.start_time.is_finite()
            || !segment.end_time.is_finite()
            || segment.start_time < 0.0
            || segment.end_time <= segment.start_time
    }) {
        return Err(anyhow!(ProcessingJobError::MalformedDiarization));
    }

    // ponytail: Preserve every speaker in a coarse transcript block. Split at speaker
    // boundaries only when word-level timestamps make that reliable.
    let assignments = segments
        .iter()
        .map(|segment| {
            let (Some(start_time), Some(end_time)) =
                (segment.audio_start_time, segment.audio_end_time)
            else {
                return None;
            };
            speaker_label_for_interval(&speaker_segments, start_time, end_time)
        })
        .collect::<Vec<_>>();

    if assignments.is_empty() || assignments.iter().any(Option::is_none) {
        return Err(anyhow!(
            "Speaker diarization did not align with the transcript"
        ));
    }
    for (segment, speaker) in segments.iter_mut().zip(assignments) {
        segment.speaker = speaker;
    }

    Ok(speaker_segments.len())
}

/// Emit progress event
fn emit_progress<R: Runtime>(
    app: &AppHandle<R>,
    meeting_id: &str,
    stage: &str,
    progress: u32,
    message: &str,
) {
    let _ = app.emit(
        "retranscription-progress",
        RetranscriptionProgress {
            meeting_id: meeting_id.to_string(),
            stage: stage.to_string(),
            progress_percentage: progress,
            message: message.to_string(),
        },
    );
}

/// Get or initialize the Whisper engine, auto-loading the model if needed
/// If `requested_model` is provided, ensures that specific model is loaded
async fn get_or_init_whisper<R: Runtime>(
    app: &AppHandle<R>,
    requested_model: Option<&str>,
) -> Result<Arc<WhisperEngine>> {
    use crate::whisper_engine::commands::WHISPER_ENGINE;

    let engine = {
        let guard = WHISPER_ENGINE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().cloned()
    };

    match engine {
        Some(e) => {
            // Determine which model to use
            let target_model = match requested_model {
                Some(model) => model.to_string(),
                None => get_configured_whisper_model(app).await?,
            };

            // Check if the correct model is already loaded
            let current_model = e.get_current_model().await;
            let needs_load = match &current_model {
                Some(loaded) => loaded != &target_model,
                None => true,
            };

            if needs_load {
                info!(
                    "Loading Whisper model '{}' (current: {:?})",
                    target_model, current_model
                );

                // Discover available models first (populates the internal cache)
                info!("Discovering available Whisper models...");
                if let Err(discover_err) = e.discover_models().await {
                    warn!(
                        "Error during model discovery (continuing anyway): {}",
                        discover_err
                    );
                }

                match e.load_model(&target_model).await {
                    Ok(_) => {
                        info!("Whisper model '{}' loaded successfully", target_model);
                        Ok(e)
                    }
                    Err(load_err) => {
                        error!(
                            "Failed to load Whisper model '{}': {}",
                            target_model, load_err
                        );
                        Err(anyhow!(
                            "Failed to load Whisper model '{}': {}",
                            target_model,
                            load_err
                        ))
                    }
                }
            } else {
                info!("Whisper model '{}' already loaded", target_model);
                Ok(e)
            }
        }
        None => Err(anyhow!("Whisper engine not initialized")),
    }
}

/// Get the configured Whisper model name from the database
async fn get_configured_whisper_model<R: Runtime>(app: &AppHandle<R>) -> Result<String> {
    debug!("Getting configured Whisper model from database...");

    let app_state = app.try_state::<AppState>().ok_or_else(|| {
        error!("App state not available");
        anyhow!("App state not available")
    })?;

    debug!("Querying transcript_settings table...");

    // Query the transcript settings from the database - get both provider and model
    let result: Option<(String, String)> =
        sqlx::query_as("SELECT provider, model FROM transcript_settings WHERE id = '1'")
            .fetch_optional(app_state.db_manager.pool())
            .await
            .map_err(|e| {
                error!("Failed to query transcript config: {}", e);
                anyhow!("Failed to query transcript config: {}", e)
            })?;

    match result {
        Some((provider, model)) => {
            info!(
                "Found transcript config: provider={}, model={}",
                provider, model
            );

            // Check if provider is Whisper-based
            if provider == "localWhisper" || provider == "whisper" {
                Ok(model)
            } else {
                error!(
                    "Retranscription requires Whisper provider, but configured provider is: {}",
                    provider
                );
                Err(anyhow!("Retranscription requires Whisper. Current provider '{}' does not support retranscription with language selection.", provider))
            }
        }
        None => {
            // Default to configured Whisper model if no config exists
            warn!(
                "No transcript config found, using default model '{}'",
                DEFAULT_WHISPER_MODEL
            );
            Ok(DEFAULT_WHISPER_MODEL.to_string())
        }
    }
}

/// Get or initialize the Parakeet engine, auto-loading the model if needed
async fn get_or_init_parakeet<R: Runtime>(
    app: &AppHandle<R>,
    requested_model: Option<&str>,
) -> Result<Arc<ParakeetEngine>> {
    use crate::parakeet_engine::commands::PARAKEET_ENGINE;

    let engine = {
        let guard = PARAKEET_ENGINE.lock().unwrap_or_else(|e| e.into_inner());
        guard.as_ref().cloned()
    };

    match engine {
        Some(e) => {
            // Determine which model to use
            let target_model = match requested_model {
                Some(model) => model.to_string(),
                None => get_configured_parakeet_model(app).await?,
            };

            // Check if the correct model is already loaded
            let current_model = e.get_current_model().await;
            let needs_load = match &current_model {
                Some(loaded) => loaded != &target_model,
                None => true,
            };

            if needs_load {
                info!(
                    "Loading Parakeet model '{}' (current: {:?})",
                    target_model, current_model
                );

                // Discover available models first
                info!("Discovering available Parakeet models...");
                if let Err(discover_err) = e.discover_models().await {
                    warn!(
                        "Error during Parakeet model discovery (continuing anyway): {}",
                        discover_err
                    );
                }

                match e.load_model(&target_model).await {
                    Ok(_) => {
                        info!("Parakeet model '{}' loaded successfully", target_model);
                        Ok(e)
                    }
                    Err(load_err) => {
                        error!(
                            "Failed to load Parakeet model '{}': {}",
                            target_model, load_err
                        );
                        Err(anyhow!(
                            "Failed to load Parakeet model '{}': {}",
                            target_model,
                            load_err
                        ))
                    }
                }
            } else {
                info!("Parakeet model '{}' already loaded", target_model);
                Ok(e)
            }
        }
        None => Err(anyhow!("Parakeet engine not initialized")),
    }
}

/// Get the configured Parakeet model name from the database
async fn get_configured_parakeet_model<R: Runtime>(app: &AppHandle<R>) -> Result<String> {
    debug!("Getting configured Parakeet model from database...");

    let app_state = app.try_state::<AppState>().ok_or_else(|| {
        error!("App state not available");
        anyhow!("App state not available")
    })?;

    // Query the transcript settings from the database
    let result: Option<(String, String)> =
        sqlx::query_as("SELECT provider, model FROM transcript_settings WHERE id = '1'")
            .fetch_optional(app_state.db_manager.pool())
            .await
            .map_err(|e| {
                error!("Failed to query transcript config: {}", e);
                anyhow!("Failed to query transcript config: {}", e)
            })?;

    match result {
        Some((provider, model)) => {
            info!(
                "Found transcript config: provider={}, model={}",
                provider, model
            );

            if provider == "parakeet" {
                Ok(model)
            } else {
                // Default to configured Parakeet model
                warn!("Configured provider is not Parakeet, using default model");
                Ok(DEFAULT_PARAKEET_MODEL.to_string())
            }
        }
        None => {
            // Default to configured Parakeet model if no config exists
            warn!("No transcript config found, using default Parakeet model");
            Ok(DEFAULT_PARAKEET_MODEL.to_string())
        }
    }
}

/// Write or update metadata.json for retranscription (preserves existing fields, adds retranscribed_at)
fn write_retranscription_metadata(
    folder: &Path,
    meeting_id: &str,
    duration_seconds: f64,
    audio_filename: &str,
    failed_origins: &[String],
) -> Result<()> {
    let metadata_path = folder.join("metadata.json");
    let temp_path = folder.join(".metadata.json.tmp");
    let now = chrono::Utc::now().to_rfc3339();

    // Try to read existing metadata and update it
    let json = if metadata_path.exists() {
        let existing = std::fs::read_to_string(&metadata_path)?;
        let mut value: serde_json::Value = serde_json::from_str(&existing)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert("retranscribed_at".to_string(), serde_json::json!(now));
            obj.insert(
                "status".to_string(),
                serde_json::json!(if failed_origins.is_empty() {
                    "completed"
                } else {
                    "partial"
                }),
            );
            obj.insert(
                "failed_origins".to_string(),
                serde_json::json!(failed_origins),
            );
            obj.insert(
                "transcript_file".to_string(),
                serde_json::json!("transcripts.json"),
            );
            obj.remove("detected_summary_language");
        }
        value
    } else {
        serde_json::json!({
            "version": "1.0",
            "meeting_id": meeting_id,
            "created_at": now,
            "completed_at": now,
            "retranscribed_at": now,
            "duration_seconds": duration_seconds,
            "audio_file": audio_filename,
            "transcript_file": "transcripts.json",
            "status": if failed_origins.is_empty() { "completed" } else { "partial" },
            "failed_origins": failed_origins,
            "source": "retranscription"
        })
    };

    let json_string = serde_json::to_string_pretty(&json)?;
    std::fs::write(&temp_path, &json_string)?;
    std::fs::rename(&temp_path, &metadata_path)?;

    info!("Wrote metadata.json to {}", metadata_path.display());
    Ok(())
}

// Tauri commands

/// Response when retranscription is started
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetranscriptionStarted {
    pub meeting_id: String,
    pub message: String,
}

// Start retranscription (Beta gated using configContext.betaFeatures)
#[tauri::command]
pub async fn start_retranscription_command<R: Runtime>(
    app: AppHandle<R>,
    meeting_id: String,
    _meeting_folder_path: String,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<RetranscriptionStarted, ProcessingJobError> {
    // Check if retranscription is already in progress (guard will be acquired in start_retranscription)
    if is_retranscription_in_progress() {
        return Err(ProcessingJobError::Duplicate);
    }

    let (job, folder, origins) = reserve_processing_job(
        &app,
        &meeting_id,
        language.as_ref(),
        model.as_ref(),
        provider.as_ref(),
    )
    .await?;

    // Spawn the retranscription in a background task
    tauri::async_runtime::spawn(async move {
        let result =
            execute_reserved_retranscription(app, job, folder, origins, language, model, provider)
                .await;

        // Errors are already emitted as events in start_retranscription
        // so we just log here for debugging
        if let Err(e) = result {
            error!("Retranscription failed: {}", classify_processing_error(&e));
        }
    });

    Ok(RetranscriptionStarted {
        meeting_id,
        message: "Retranscription started".to_string(),
    })
}

#[tauri::command]
pub async fn cancel_retranscription_command() -> Result<(), String> {
    if !is_retranscription_in_progress() {
        return Err("No retranscription in progress".to_string());
    }
    cancel_retranscription();
    Ok(())
}

#[tauri::command]
pub async fn is_retranscription_in_progress_command() -> bool {
    is_retranscription_in_progress()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_transcript_segments_empty() {
        let transcripts: Vec<(String, f64, f64)> = vec![];
        let segments = create_transcript_segments(&transcripts);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_create_transcript_segments_single() {
        let transcripts = vec![
            ("Hello world".to_string(), 0.0, 1500.0), // 0-1.5 seconds
        ];
        let segments = create_transcript_segments(&transcripts);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello world");
        assert_eq!(segments[0].audio_start_time, Some(0.0));
        assert_eq!(segments[0].audio_end_time, Some(1.5));
        assert_eq!(segments[0].duration, Some(1.5));
    }

    #[test]
    fn test_create_transcript_segments_multiple() {
        let transcripts = vec![
            ("First segment".to_string(), 0.0, 2000.0), // 0-2 seconds
            ("Second segment".to_string(), 3000.0, 5000.0), // 3-5 seconds
            ("Third segment".to_string(), 6500.0, 8000.0), // 6.5-8 seconds
        ];
        let segments = create_transcript_segments(&transcripts);

        assert_eq!(segments.len(), 3);

        // First segment
        assert_eq!(segments[0].text, "First segment");
        assert_eq!(segments[0].audio_start_time, Some(0.0));
        assert_eq!(segments[0].audio_end_time, Some(2.0));
        assert_eq!(segments[0].duration, Some(2.0));

        // Second segment
        assert_eq!(segments[1].text, "Second segment");
        assert_eq!(segments[1].audio_start_time, Some(3.0));
        assert_eq!(segments[1].audio_end_time, Some(5.0));
        assert_eq!(segments[1].duration, Some(2.0));

        // Third segment
        assert_eq!(segments[2].text, "Third segment");
        assert_eq!(segments[2].audio_start_time, Some(6.5));
        assert_eq!(segments[2].audio_end_time, Some(8.0));
        assert_eq!(segments[2].duration, Some(1.5));
    }

    #[test]
    fn test_create_transcript_segments_trims_whitespace() {
        let transcripts = vec![("  Hello with spaces  ".to_string(), 0.0, 1000.0)];
        let segments = create_transcript_segments(&transcripts);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Hello with spaces");
    }

    #[test]
    fn test_create_transcript_segments_generates_unique_ids() {
        let transcripts = vec![
            ("Segment one".to_string(), 0.0, 1000.0),
            ("Segment two".to_string(), 1000.0, 2000.0),
        ];
        let segments = create_transcript_segments(&transcripts);

        assert_eq!(segments.len(), 2);
        assert_ne!(segments[0].id, segments[1].id);
        assert!(segments[0].id.starts_with("transcript-"));
        assert!(segments[1].id.starts_with("transcript-"));
    }

    #[test]
    fn test_cancellation_state_stops_processing_but_not_an_active_save() {
        RETRANSCRIPTION_STATE.store(RETRANSCRIPTION_IDLE, Ordering::SeqCst);

        assert!(!is_retranscription_in_progress());

        let guard = RetranscriptionGuard::acquire().unwrap();
        cancel_retranscription();
        assert!(ensure_not_cancelled().is_err());
        drop(guard);

        let guard = RetranscriptionGuard::acquire().unwrap();
        begin_non_cancellable_save().unwrap();
        cancel_retranscription();
        assert!(ensure_not_cancelled().is_ok());
        drop(guard);
    }

    #[test]
    fn test_vad_redemption_time_constant() {
        // Batch processing uses 2000ms to bridge natural pauses in full-file VAD
        assert_eq!(VAD_REDEMPTION_TIME_MS, 2000);
    }

    #[test]
    fn test_find_audio_file_common_candidates() {
        let dir = tempfile::tempdir().unwrap();

        // No audio file → error
        assert!(find_audio_file(dir.path()).is_err());

        // Create audio.mp4 — should be found first
        std::fs::write(dir.path().join("audio.mp4"), b"fake").unwrap();
        let found = find_audio_file(dir.path()).unwrap();
        assert_eq!(found.file_name().unwrap(), "audio.mp4");
    }

    #[test]
    fn test_find_audio_file_non_mp4_extensions() {
        let dir = tempfile::tempdir().unwrap();

        // Create audio.wav (imported as .wav, not .mp4)
        std::fs::write(dir.path().join("audio.wav"), b"fake").unwrap();
        let found = find_audio_file(dir.path()).unwrap();
        assert_eq!(found.file_name().unwrap(), "audio.wav");
    }

    #[test]
    fn test_find_audio_file_fallback_scan() {
        let dir = tempfile::tempdir().unwrap();

        // Create a file with an audio extension but non-standard name
        std::fs::write(dir.path().join("my_recording.flac"), b"fake").unwrap();
        // Also add a non-audio file that should be ignored
        std::fs::write(dir.path().join("notes.txt"), b"text").unwrap();

        let found = find_audio_file(dir.path()).unwrap();
        assert_eq!(found.file_name().unwrap(), "my_recording.flac");
    }

    #[test]
    fn test_find_audio_file_priority_order() {
        let dir = tempfile::tempdir().unwrap();

        // Create both audio.m4a and audio.mp4 — mp4 should win (listed first in candidates)
        std::fs::write(dir.path().join("audio.m4a"), b"fake").unwrap();
        std::fs::write(dir.path().join("audio.mp4"), b"fake").unwrap();
        let found = find_audio_file(dir.path()).unwrap();
        assert_eq!(found.file_name().unwrap(), "audio.mp4");
    }

    #[test]
    fn test_find_audio_file_empty_folder() {
        let dir = tempfile::tempdir().unwrap();
        let result = find_audio_file(dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No audio file found"));
    }

    #[test]
    fn test_find_audio_file_nonexistent_folder() {
        let result = find_audio_file(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err());
    }

    #[test]
    fn test_audio_extensions_constant() {
        // Keep retranscription aligned with the V1 import contract.
        assert!(AUDIO_EXTENSIONS.contains(&"mp4"));
        assert!(AUDIO_EXTENSIONS.contains(&"mov"));
        assert!(AUDIO_EXTENSIONS.contains(&"m4v"));
        assert!(AUDIO_EXTENSIONS.contains(&"m4a"));
        assert!(AUDIO_EXTENSIONS.contains(&"wav"));
        assert!(AUDIO_EXTENSIONS.contains(&"mp3"));
        assert!(AUDIO_EXTENSIONS.contains(&"flac"));
        assert!(AUDIO_EXTENSIONS.contains(&"ogg"));
        assert!(AUDIO_EXTENSIONS.contains(&"aac"));
        assert!(!AUDIO_EXTENSIONS.contains(&"mkv"));
        assert!(!AUDIO_EXTENSIONS.contains(&"webm"));
        assert!(!AUDIO_EXTENSIONS.contains(&"wma"));
        assert!(!AUDIO_EXTENSIONS.contains(&"txt"));
        assert!(!AUDIO_EXTENSIONS.contains(&"pdf"));
    }

    #[test]
    fn diarization_failure_preserves_the_transcript_without_a_speaker() {
        let mut segments =
            create_transcript_segments(&[("Keep this transcript".to_string(), 0.0, 1_000.0)]);

        let result =
            apply_diarization_result(&mut segments, Err(anyhow!("speaker model unavailable")));

        assert!(result.is_err());
        assert_eq!(segments[0].speaker, None);
        assert_eq!(segments[0].text, "Keep this transcript");
    }

    #[test]
    fn empty_or_unaligned_diarization_is_not_reported_as_complete() {
        let mut segments =
            create_transcript_segments(&[("Keep this transcript".to_string(), 0.0, 1_000.0)]);

        assert!(apply_diarization_result(&mut segments, Ok(vec![])).is_err());
        assert!(apply_diarization_result(
            &mut segments,
            Ok(vec![SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: 10.0,
                end_time: 11.0,
            }]),
        )
        .is_err());
        assert_eq!(segments[0].speaker, None);
    }

    #[test]
    fn partially_aligned_diarization_is_atomic_and_not_complete() {
        let mut segments = create_transcript_segments(&[
            ("First".to_string(), 0.0, 1_000.0),
            ("Second".to_string(), 2_000.0, 3_000.0),
        ]);

        assert!(apply_diarization_result(
            &mut segments,
            Ok(vec![SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: 0.0,
                end_time: 1.0,
            }]),
        )
        .is_err());
        assert!(segments.iter().all(|segment| segment.speaker.is_none()));
    }

    #[test]
    fn malformed_diarization_is_rejected_without_changing_the_transcript() {
        let malformed = [
            SpeakerSegment {
                speaker_id: " ".to_string(),
                start_time: 0.0,
                end_time: 1.0,
            },
            SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: f64::NAN,
                end_time: 1.0,
            },
            SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: 0.0,
                end_time: f64::INFINITY,
            },
            SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: -1.0,
                end_time: 1.0,
            },
            SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: 2.0,
                end_time: 1.0,
            },
        ];

        for speaker in malformed {
            let mut segments =
                create_transcript_segments(&[("Keep this transcript".to_string(), 0.0, 1_000.0)]);
            assert!(apply_diarization_result(&mut segments, Ok(vec![speaker])).is_err());
            assert_eq!(segments[0].speaker, None);
            assert_eq!(segments[0].text, "Keep this transcript");
        }
    }

    #[test]
    fn captured_recording_uses_independent_origins_instead_of_the_mix() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("audio.mp4"), b"mix").unwrap();
        std::fs::write(dir.path().join("microphone.mp4"), b"mic").unwrap();
        std::fs::write(dir.path().join("system-audio.mp4"), b"system").unwrap();

        let origins = find_audio_origins(dir.path()).unwrap();

        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].kind, "microphone");
        assert_eq!(origins[0].path, dir.path().join("microphone.mp4"));
        assert_eq!(origins[1].kind, "system-audio");
        assert_eq!(origins[1].path, dir.path().join("system-audio.mp4"));
    }

    #[test]
    fn declared_source_loss_is_visible_and_bound_to_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("metadata.json"),
            serde_json::to_vec(&serde_json::json!({
                "origins": [
                    { "kind": "microphone", "file": "microphone.mp4", "status": "stored" },
                    { "kind": "system-audio", "file": "system-audio.mp4", "status": "stored" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(dir.path().join("microphone.mp4"), b"mic").unwrap();
        std::fs::write(dir.path().join("audio.mp4"), b"mix").unwrap();

        let lost = discover_audio_origins(dir.path()).unwrap();
        assert_eq!(
            lost.available
                .iter()
                .map(|origin| origin.kind)
                .collect::<Vec<_>>(),
            vec!["microphone"]
        );
        assert_eq!(lost.missing, vec!["system-audio"]);

        std::fs::write(dir.path().join("system-audio.mp4"), b"system").unwrap();
        let restored = discover_audio_origins(dir.path()).unwrap();
        assert!(restored.missing.is_empty());
        assert_ne!(lost.manifest_hash, restored.manifest_hash);
    }

    #[test]
    fn partial_declared_origin_is_unavailable_even_when_its_file_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("metadata.json"),
            serde_json::to_vec(&serde_json::json!({
                "origins": [
                    { "kind": "microphone", "file": "microphone.mp4", "status": "stored" },
                    { "kind": "system-audio", "file": "system-audio.mp4", "status": "partial" }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(dir.path().join("microphone.mp4"), b"mic").unwrap();
        std::fs::write(dir.path().join("system-audio.mp4"), b"partial system").unwrap();

        let discovery = discover_audio_origins(dir.path()).unwrap();

        assert_eq!(
            discovery
                .available
                .iter()
                .map(|origin| origin.kind)
                .collect::<Vec<_>>(),
            vec!["microphone"]
        );
        assert_eq!(discovery.missing, vec!["system-audio"]);
    }

    #[test]
    fn captured_origin_metadata_rejects_unknown_kind_status_and_file() {
        for origin in [
            serde_json::json!({ "kind": "unknown", "file": "unknown.mp4", "status": "stored" }),
            serde_json::json!({ "kind": "microphone", "file": "microphone.mp4", "status": "unknown" }),
            serde_json::json!({ "kind": "microphone", "file": "other.mp4", "status": "stored" }),
        ] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(
                dir.path().join("metadata.json"),
                serde_json::to_vec(&serde_json::json!({ "origins": [origin] })).unwrap(),
            )
            .unwrap();
            assert!(discover_audio_origins(dir.path()).is_err());
        }
    }

    #[test]
    fn imported_recording_uses_its_single_audio_origin() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("audio.mp4"), b"imported").unwrap();

        let origins = find_audio_origins(dir.path()).unwrap();

        assert_eq!(origins.len(), 1);
        assert_eq!(origins[0].kind, "imported");
        assert_eq!(origins[0].path, dir.path().join("audio.mp4"));
    }

    #[test]
    fn input_manifest_detects_source_changes_without_using_private_paths() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join("audio.mp4"), b"same audio").unwrap();
        std::fs::write(second.path().join("audio.mp4"), b"same audio").unwrap();
        let first_origins = find_audio_origins(first.path()).unwrap();
        let second_origins = find_audio_origins(second.path()).unwrap();

        let expected = input_manifest_hash(&first_origins).unwrap();
        assert_eq!(expected, input_manifest_hash(&second_origins).unwrap());
        std::fs::write(first.path().join("audio.mp4"), b"changed audio").unwrap();
        assert_ne!(expected, input_manifest_hash(&first_origins).unwrap());
        assert_eq!(expected.len(), 64);
    }

    #[test]
    fn recording_folder_must_remain_inside_the_managed_root() {
        let root = tempfile::tempdir().unwrap();
        let recording = root.path().join("recording");
        std::fs::create_dir(&recording).unwrap();
        let outside = tempfile::tempdir().unwrap();

        assert!(validate_recording_folder(root.path(), &recording).is_ok());
        assert!(validate_recording_folder(root.path(), outside.path()).is_err());
    }

    #[test]
    fn microphone_participants_are_snapshotted_per_recording() {
        let recording = tempfile::tempdir().unwrap();
        std::fs::write(recording.path().join("metadata.json"), "{}").unwrap();
        let initial = super::super::recording_preferences::RecordingPreferences {
            local_speaker_name: "Guilherme".to_string(),
            ..Default::default()
        };
        let changed = super::super::recording_preferences::RecordingPreferences {
            local_speaker_name: "Someone else".to_string(),
            ..Default::default()
        };

        let first = load_or_snapshot_microphone_participants(recording.path(), &initial).unwrap();
        let second = load_or_snapshot_microphone_participants(recording.path(), &changed).unwrap();

        assert_eq!(first, second);
        assert_eq!(second.local_speaker_name, "Guilherme");
    }

    #[cfg(unix)]
    #[test]
    fn audio_origin_symlink_cannot_escape_the_recording_folder() {
        use std::os::unix::fs::symlink;

        let recording = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        symlink(outside.path(), recording.path().join("audio.mp4")).unwrap();

        assert!(find_audio_file(recording.path()).is_err());
    }

    #[test]
    fn origin_timelines_keep_provenance_and_overlapping_passages() {
        let mut microphone =
            create_transcript_segments(&[("Local answer".to_string(), 1_000.0, 2_000.0)]);
        microphone[0].speaker = Some("You".to_string());
        microphone[0].source_origin = Some("microphone".to_string());

        let mut system =
            create_transcript_segments(&[("Remote question".to_string(), 500.0, 1_500.0)]);
        system[0].speaker = Some("Speaker 1".to_string());
        system[0].source_origin = Some("system-audio".to_string());

        let merged = merge_origin_timelines(vec![microphone, system]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "Remote question");
        assert_eq!(merged[0].source_origin.as_deref(), Some("system-audio"));
        assert_eq!(merged[1].text, "Local answer");
        assert_eq!(merged[1].source_origin.as_deref(), Some("microphone"));
    }

    #[test]
    fn timeline_keeps_silence_gaps_and_all_overlapping_passages() {
        let mut microphone = create_transcript_segments(&[
            ("Before silence".to_string(), 0.0, 1_000.0),
            ("After silence".to_string(), 4_000.0, 5_000.0),
        ]);
        for segment in &mut microphone {
            segment.source_origin = Some("microphone".to_string());
        }
        let mut system = create_transcript_segments(&[("Overlapping".to_string(), 500.0, 1_500.0)]);
        system[0].source_origin = Some("system-audio".to_string());

        let merged = merge_origin_timelines(vec![microphone, system]);

        assert_eq!(
            merged
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Before silence", "Overlapping", "After silence"]
        );
        assert_eq!(merged[1].audio_end_time, Some(1.5));
        assert_eq!(merged[2].audio_start_time, Some(4.0));
    }

    #[test]
    fn ambiguous_duplicate_passages_from_different_origins_are_preserved() {
        let mut microphone =
            create_transcript_segments(&[("Ação útil.".to_string(), 1_000.0, 2_000.0)]);
        microphone[0].source_origin = Some("microphone".to_string());
        let mut system = create_transcript_segments(&[(
            "Ac\u{327}a\u{303}o u\u{301}til-".to_string(),
            1_000.0,
            2_000.0,
        )]);
        system[0].source_origin = Some("system-audio".to_string());

        let merged = merge_origin_timelines(vec![system, microphone]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].source_origin.as_deref(), Some("microphone"));
        assert_eq!(merged[1].source_origin.as_deref(), Some("system-audio"));
        assert_eq!(merged[0].ambiguity_group_id, merged[1].ambiguity_group_id);
        assert!(merged[0].ambiguity_group_id.is_some());
        assert_eq!(
            merged[0].alignment_decision.as_deref(),
            Some("preserved-ambiguous-duplicate")
        );
    }

    #[test]
    fn ambiguity_normalization_handles_dotted_i_case_folding() {
        assert_eq!(normalized_text("İtem."), normalized_text("item-"));
    }

    #[test]
    fn individual_microphone_uses_the_local_participant_without_diarization() {
        let mut segments =
            create_transcript_segments(&[("Local speech".to_string(), 0.0, 1_000.0)]);
        segments[0].source_origin = Some("microphone".to_string());

        assign_microphone_speaker(&mut segments, "Guilherme");

        assert_eq!(segments[0].speaker.as_deref(), Some("Guilherme"));
        assert_eq!(segments[0].source_origin.as_deref(), Some("microphone"));
        assert_eq!(
            segments[0].speaker_cluster_id.as_deref(),
            Some("microphone:local")
        );
    }

    #[test]
    fn one_failed_origin_keeps_the_successful_timeline() {
        let mut segments =
            create_transcript_segments(&[("Remote speech".to_string(), 500.0, 1_500.0)]);
        segments[0].source_origin = Some("system-audio".to_string());

        let combined = combine_processed_origins(
            vec![ProcessedOrigin {
                segments,
                duration_seconds: 2.0,
                speaker_labels_completed: true,
            }],
            true,
        )
        .unwrap();

        assert_eq!(combined.segments.len(), 1);
        assert_eq!(combined.duration_seconds, 2.0);
        assert!(!combined.speaker_labels_completed);
    }

    #[test]
    fn all_failed_origins_cannot_replace_the_existing_transcript() {
        assert!(combine_processed_origins(Vec::new(), true).is_err());
    }

    #[test]
    fn partial_retry_preserves_an_existing_transcript() {
        let failed = vec!["microphone".to_string()];
        assert!(must_preserve_existing_transcript(&failed, 3));
        assert!(!must_preserve_existing_transcript(&failed, 0));
    }
}
