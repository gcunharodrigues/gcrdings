use anyhow::Result;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use super::audio_processing::create_meeting_folder;
use super::incremental_saver::IncrementalAudioSaver;
use super::recording_state::AudioChunk;

/// Structured transcript segment for JSON export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub audio_start_time: f64, // Seconds from recording start
    pub audio_end_time: f64,   // Seconds from recording start
    pub duration: f64,         // Segment duration in seconds
    pub display_time: String,  // Formatted time for display like "[02:15]"
    pub confidence: f32,
    pub sequence_id: u64,
}

/// Meeting metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingMetadata {
    pub version: String,
    pub meeting_id: Option<String>,
    pub meeting_name: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub duration_seconds: Option<f64>,
    pub devices: DeviceInfo,
    pub audio_file: String,
    pub transcript_file: String,
    pub sample_rate: u32,
    pub status: String, // "recording", "completed", "error"
    pub origins: Vec<OriginMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microphone_participants:
        Option<super::recording_preferences::MicrophoneParticipantSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub microphone: Option<String>,
    pub system_audio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginMetadata {
    pub kind: String,
    pub file: String,
    pub status: String,
}

pub struct RecordingSaver {
    microphone_saver: Option<Arc<AsyncMutex<IncrementalAudioSaver>>>,
    system_audio_saver: Option<Arc<AsyncMutex<IncrementalAudioSaver>>>,
    accumulation_task: Option<JoinHandle<()>>,
    meeting_folder: Option<PathBuf>,
    meeting_name: Option<String>,
    metadata: Option<MeetingMetadata>,
    transcript_segments: Arc<Mutex<Vec<TranscriptSegment>>>,
    microphone_had_persistence_failure: Arc<AtomicBool>,
    system_audio_had_persistence_failure: Arc<AtomicBool>,
    microphone_participants: Option<super::recording_preferences::MicrophoneParticipantSnapshot>,
}

fn completed_origin_status(stored: bool, persistence_failed: bool) -> &'static str {
    if persistence_failed {
        "partial"
    } else if stored {
        "stored"
    } else {
        "unavailable"
    }
}

impl RecordingSaver {
    pub fn new() -> Self {
        Self {
            microphone_saver: None,
            system_audio_saver: None,
            accumulation_task: None,
            meeting_folder: None,
            meeting_name: None,
            metadata: None,
            transcript_segments: Arc::new(Mutex::new(Vec::new())),
            microphone_had_persistence_failure: Arc::new(AtomicBool::new(false)),
            system_audio_had_persistence_failure: Arc::new(AtomicBool::new(false)),
            microphone_participants: None,
        }
    }

    /// Set the meeting name for this recording session
    pub fn set_meeting_name(&mut self, name: Option<String>) {
        self.meeting_name = name;
    }

    pub(crate) fn set_microphone_participants(
        &mut self,
        snapshot: super::recording_preferences::MicrophoneParticipantSnapshot,
    ) {
        self.microphone_participants = Some(snapshot);
    }

    /// Set device information in metadata
    pub fn set_device_info(&mut self, mic_name: Option<String>, sys_name: Option<String>) {
        if let Some(ref mut metadata) = self.metadata {
            metadata.devices.microphone = mic_name;
            metadata.devices.system_audio = sys_name;

            // Write updated metadata to disk if folder exists
            if let Some(folder) = &self.meeting_folder {
                let metadata_clone = metadata.clone();
                if let Err(e) = self.write_metadata(folder, &metadata_clone) {
                    warn!("Failed to update metadata with device info: {}", e);
                }
            }
        }
    }

    /// Add or update a structured transcript segment (upserts based on sequence_id)
    /// Also saves incrementally to disk
    pub fn add_transcript_segment(&self, segment: TranscriptSegment) {
        if let Ok(mut segments) = self.transcript_segments.lock() {
            // Check if segment with same sequence_id exists (update it)
            if let Some(existing) = segments
                .iter_mut()
                .find(|s| s.sequence_id == segment.sequence_id)
            {
                *existing = segment.clone();
                info!(
                    "Updated transcript segment {} (seq: {}) - total segments: {}",
                    segment.id,
                    segment.sequence_id,
                    segments.len()
                );
            } else {
                // New segment, add it
                segments.push(segment.clone());
                info!(
                    "Added new transcript segment {} (seq: {}) - total segments: {}",
                    segment.id,
                    segment.sequence_id,
                    segments.len()
                );
            }
        } else {
            error!(
                "Failed to lock transcript segments for adding segment {}",
                segment.id
            );
        }

        // NEW: Save incrementally to disk
        if let Some(folder) = &self.meeting_folder {
            if let Err(e) = self.write_transcripts_json(folder) {
                warn!("Failed to write incremental transcript update: {}", e);
            }
        }
    }

    /// Legacy method for backward compatibility - converts text to basic segment
    pub fn add_transcript_chunk(&self, text: String) {
        let segment = TranscriptSegment {
            id: format!("seg_{}", chrono::Utc::now().timestamp_millis()),
            text,
            audio_start_time: 0.0,
            audio_end_time: 0.0,
            duration: 0.0,
            display_time: "[00:00]".to_string(),
            confidence: 1.0,
            sequence_id: 0,
        };
        self.add_transcript_segment(segment);
    }

    /// Start accumulation with optional incremental saving
    ///
    /// # Arguments
    /// * `auto_save` - If true, creates checkpoints and enables saving. If false, audio chunks are discarded.
    pub fn start_accumulation(
        &mut self,
        auto_save: bool,
        capture_microphone: bool,
        capture_system_audio: bool,
        state: Arc<super::recording_state::RecordingState>,
    ) -> (
        mpsc::UnboundedSender<AudioChunk>,
        oneshot::Receiver<super::recording_state::DeviceType>,
    ) {
        if auto_save {
            info!("Initializing incremental audio saver for recording (auto-save ENABLED)");
        } else {
            info!(
                "Starting recording without audio saving (auto-save DISABLED - transcripts only)"
            );
        }

        // Create channel for receiving audio chunks
        let (sender, mut receiver) = mpsc::unbounded_channel::<AudioChunk>();
        let (ready_sender, ready_receiver) = oneshot::channel();

        // Initialize meeting folder and incremental saver ONLY if auto_save is enabled
        if auto_save {
            if let Some(name) = self.meeting_name.clone() {
                match self.initialize_meeting_folder(
                    &name,
                    true,
                    capture_microphone,
                    capture_system_audio,
                ) {
                    Ok(()) => info!("Successfully initialized meeting folder with checkpoints"),
                    Err(e) => {
                        error!("Failed to initialize meeting folder: {}", e);
                        // Continue anyway - will use fallback flat structure
                    }
                }
            }
        } else {
            // When auto_save is false, still create meeting folder for transcripts/metadata
            // but skip .checkpoints directory
            if let Some(name) = self.meeting_name.clone() {
                match self.initialize_meeting_folder(&name, false, false, false) {
                    Ok(()) => info!("Successfully initialized meeting folder (transcripts only)"),
                    Err(e) => {
                        error!("Failed to initialize meeting folder: {}", e);
                    }
                }
            }
        }

        // Start accumulation task
        let microphone_saver = self.microphone_saver.clone();
        let system_audio_saver = self.system_audio_saver.clone();
        let microphone_failed = self.microphone_had_persistence_failure.clone();
        let system_audio_failed = self.system_audio_had_persistence_failure.clone();
        let save_audio = auto_save;

        self.accumulation_task = Some(tokio::spawn(async move {
            let mut ready_sender = Some(ready_sender);
            info!(
                "Recording saver accumulation task started (save_audio: {})",
                save_audio
            );

            while let Some(chunk) = receiver.recv().await {
                if !save_audio {
                    continue;
                }

                let saver = match chunk.device_type {
                    super::recording_state::DeviceType::Microphone => &microphone_saver,
                    super::recording_state::DeviceType::System => &system_audio_saver,
                };
                if let Some(saver) = saver {
                    let device_type = chunk.device_type.clone();
                    match saver.lock().await.add_chunk(chunk) {
                        Err(error) => {
                            match &device_type {
                                super::recording_state::DeviceType::Microphone => {
                                    microphone_failed.store(true, Ordering::Relaxed)
                                }
                                super::recording_state::DeviceType::System => {
                                    system_audio_failed.store(true, Ordering::Relaxed)
                                }
                            }
                            state.mark_origin_failed(device_type);
                            error!("Failed to persist audio origin chunk: {}", error);
                        }
                        Ok(true) => {
                            state.mark_origin_persisted(device_type.clone());
                            if let Some(sender) = ready_sender.take() {
                                let _ = sender.send(device_type);
                            }
                        }
                        Ok(false) => {}
                    }
                }
            }

            info!("Recording saver accumulation task ended");
        }));
        (sender, ready_receiver)
    }

    /// Initialize meeting folder structure and metadata
    ///
    /// # Arguments
    /// * `meeting_name` - Name of the meeting
    /// * `create_checkpoints` - Whether to create .checkpoints/ directory and IncrementalAudioSaver
    fn initialize_meeting_folder(
        &mut self,
        meeting_name: &str,
        create_checkpoints: bool,
        capture_microphone: bool,
        capture_system_audio: bool,
    ) -> Result<()> {
        // Load preferences to get base recordings folder
        let base_folder = super::recording_preferences::get_default_recordings_folder();

        // Create meeting folder structure (with or without .checkpoints/ subdirectory)
        let meeting_folder = create_meeting_folder(&base_folder, meeting_name, create_checkpoints)?;

        // Only initialize incremental saver if checkpoints are needed (auto_save is true)
        if create_checkpoints {
            if capture_microphone {
                self.microphone_saver = Some(Arc::new(AsyncMutex::new(
                    IncrementalAudioSaver::new_for_track(
                        meeting_folder.clone(),
                        48000,
                        "microphone",
                    )?,
                )));
            }
            if capture_system_audio {
                self.system_audio_saver = Some(Arc::new(AsyncMutex::new(
                    IncrementalAudioSaver::new_for_track(
                        meeting_folder.clone(),
                        48000,
                        "system-audio",
                    )?,
                )));
            }
            info!("Incremental origin savers initialized for recording");
        } else {
            info!("⚠️  Skipped incremental audio saver (auto-save disabled)");
        }

        // Create initial metadata
        let metadata = MeetingMetadata {
            version: "1.0".to_string(),
            meeting_id: None, // Will be set by backend
            meeting_name: Some(meeting_name.to_string()),
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            duration_seconds: None,
            devices: DeviceInfo {
                microphone: None, // Could be enhanced to store actual device names
                system_audio: None,
            },
            audio_file: if create_checkpoints {
                "audio.mp4".to_string()
            } else {
                "".to_string()
            },
            transcript_file: "transcripts.json".to_string(),
            sample_rate: 48000,
            status: "starting".to_string(),
            origins: [
                capture_microphone.then(|| OriginMetadata {
                    kind: "microphone".to_string(),
                    file: "microphone.mp4".to_string(),
                    status: "waiting-for-samples".to_string(),
                }),
                capture_system_audio.then(|| OriginMetadata {
                    kind: "system-audio".to_string(),
                    file: "system-audio.mp4".to_string(),
                    status: "waiting-for-samples".to_string(),
                }),
            ]
            .into_iter()
            .flatten()
            .collect(),
            microphone_participants: capture_microphone
                .then(|| self.microphone_participants.clone())
                .flatten(),
        };

        // Write initial metadata.json
        self.write_metadata(&meeting_folder, &metadata)?;

        self.meeting_folder = Some(meeting_folder);
        self.metadata = Some(metadata);

        Ok(())
    }

    pub fn set_capture_status(&mut self, status: &str) -> Result<()> {
        let folder = self
            .meeting_folder
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Recording folder is unavailable"))?;
        let metadata = self
            .metadata
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Recording metadata is unavailable"))?;
        metadata.status = status.to_string();
        let snapshot = metadata.clone();
        self.write_metadata(folder, &snapshot)
    }

    /// Write metadata.json to disk (atomic write with temp file)
    fn write_metadata(&self, folder: &Path, metadata: &MeetingMetadata) -> Result<()> {
        let metadata_path = folder.join("metadata.json");
        let temp_path = folder.join(".metadata.json.tmp");

        let json_string = serde_json::to_string_pretty(metadata)?;
        std::fs::write(&temp_path, json_string)?;
        std::fs::rename(&temp_path, &metadata_path)?; // Atomic

        Ok(())
    }

    /// Write transcripts.json to disk (atomic write with temp file and validation)
    fn write_transcripts_json(&self, folder: &Path) -> Result<()> {
        // Clone segments to avoid holding lock during I/O
        let segments_clone = if let Ok(segments) = self.transcript_segments.lock() {
            segments.clone()
        } else {
            error!("Failed to lock transcript segments for writing");
            return Err(anyhow::anyhow!("Failed to lock transcript segments"));
        };

        info!(
            "Writing {} transcript segments to JSON",
            segments_clone.len()
        );

        let transcript_path = folder.join("transcripts.json");
        let temp_path = folder.join(".transcripts.json.tmp");

        // Create JSON structure
        let json = serde_json::json!({
            "version": "1.0",
            "segments": segments_clone,
            "last_updated": chrono::Utc::now().to_rfc3339(),
            "total_segments": segments_clone.len()
        });

        // Serialize to pretty JSON string
        let json_string = serde_json::to_string_pretty(&json).map_err(|e| {
            error!("Failed to serialize transcripts to JSON: {}", e);
            anyhow::anyhow!("JSON serialization failed: {}", e)
        })?;

        // Write to temp file with error handling
        std::fs::write(&temp_path, &json_string).map_err(|e| {
            error!("Failed to write transcript temp file: {}", e);
            anyhow::anyhow!("Failed to write temp file: {}", e)
        })?;

        // Verify temp file was written correctly
        if !temp_path.exists() {
            error!("Temp transcript file does not exist after write");
            return Err(anyhow::anyhow!("Temp file verification failed"));
        }

        // Atomic rename
        std::fs::rename(&temp_path, &transcript_path).map_err(|e| {
            error!("Failed to install transcript file: {}", e);
            anyhow::anyhow!("Failed to rename transcript file: {}", e)
        })?;

        info!(
            "✅ Successfully wrote transcripts.json with {} segments",
            segments_clone.len()
        );
        Ok(())
    }

    // in frontend/src-tauri/src/audio/recording_saver.rs
    pub fn get_stats(&self) -> (usize, u32) {
        if let Some(ref saver) = self.microphone_saver {
            if let Ok(guard) = saver.try_lock() {
                (guard.get_checkpoint_count() as usize, 48000)
            } else {
                (0, 48000)
            }
        } else {
            (0, 48000)
        }
    }

    /// Stop and save using incremental saving approach
    ///
    /// # Arguments
    /// * `app` - Tauri app handle for emitting events
    /// * `recording_duration` - Actual recording duration in seconds (from RecordingState)
    pub async fn stop_and_save<R: Runtime>(
        &mut self,
        app: &AppHandle<R>,
        recording_duration: Option<f64>,
    ) -> Result<Option<String>, String> {
        info!("Stopping recording saver");

        if let Some(task) = self.accumulation_task.take() {
            task.await
                .map_err(|error| format!("Audio persistence task failed: {error}"))?;
        }

        if self.microphone_saver.is_none() && self.system_audio_saver.is_none() {
            info!("⚠️  No audio saver initialized (auto-save was disabled) - skipping audio finalization");
            info!("✅ Transcripts and metadata already saved incrementally");
            return Ok(None);
        }

        let (microphone_path, microphone_error) =
            Self::finalize_origin(&self.microphone_saver, "microphone").await;
        let (system_audio_path, system_audio_error) =
            Self::finalize_origin(&self.system_audio_saver, "system audio").await;
        let microphone_persistence_failed = self
            .microphone_had_persistence_failure
            .load(Ordering::Relaxed);
        let system_audio_persistence_failed = self
            .system_audio_had_persistence_failure
            .load(Ordering::Relaxed);
        let had_persistence_failure =
            microphone_persistence_failed || system_audio_persistence_failed;
        let final_audio_path = self
            .create_mixed_track(microphone_path.as_ref(), system_audio_path.as_ref())
            .await?;

        // Save final transcripts.json with validation
        if let Some(folder) = &self.meeting_folder {
            if let Err(e) = self.write_transcripts_json(folder) {
                error!("❌ Failed to write final transcripts: {}", e);
                return Err(format!("Failed to save transcripts: {}", e));
            }

            // Verify transcripts were written correctly
            let transcript_path = folder.join("transcripts.json");
            if !transcript_path.exists() {
                error!("Transcript file was not created");
                return Err("Transcript file verification failed".to_string());
            }
            info!("Transcripts saved and verified");
        }

        // Update metadata to completed status with actual recording duration
        if let (Some(folder), Some(mut metadata)) = (&self.meeting_folder, self.metadata.clone()) {
            metadata.status = if had_persistence_failure
                || microphone_error.is_some()
                || system_audio_error.is_some()
            {
                "partial".to_string()
            } else {
                "completed".to_string()
            };
            metadata.completed_at = Some(chrono::Utc::now().to_rfc3339());

            // Use actual recording duration from RecordingState (more accurate than transcript segments)
            // Falls back to last transcript segment if duration not provided
            metadata.duration_seconds = recording_duration.or_else(|| {
                if let Ok(segments) = self.transcript_segments.lock() {
                    segments.last().map(|seg| seg.audio_end_time)
                } else {
                    None
                }
            });
            for origin in &mut metadata.origins {
                origin.status = match origin.kind.as_str() {
                    "microphone" => completed_origin_status(
                        microphone_path.is_some(),
                        microphone_persistence_failed,
                    ),
                    "system-audio" => completed_origin_status(
                        system_audio_path.is_some(),
                        system_audio_persistence_failed,
                    ),
                    _ => completed_origin_status(false, false),
                }
                .to_string();
            }

            if let Err(e) = self.write_metadata(folder, &metadata) {
                error!("❌ Failed to update metadata to completed: {}", e);
                return Err(format!("Failed to update metadata: {}", e));
            }

            info!(
                "✅ Metadata updated with duration: {:?}s",
                metadata.duration_seconds
            );
        }

        // Emit save event with audio and transcript paths
        let save_event = serde_json::json!({
            "audio_file": final_audio_path.to_string_lossy(),
            "microphone_file": microphone_path.as_ref().map(|path| path.to_string_lossy()),
            "system_audio_file": system_audio_path.as_ref().map(|path| path.to_string_lossy()),
            "transcript_file": self.meeting_folder.as_ref()
                .map(|f| f.join("transcripts.json").to_string_lossy().to_string()),
            "meeting_name": self.meeting_name,
            "meeting_folder": self.meeting_folder.as_ref()
                .map(|f| f.to_string_lossy().to_string())
        });

        if let Err(e) = app.emit("recording-saved", &save_event) {
            warn!("Failed to emit recording-saved event: {}", e);
        }

        if had_persistence_failure || microphone_error.is_some() || system_audio_error.is_some() {
            let _ = app.emit(
                "recording-save-warning",
                serde_json::json!({
                    "message": "One audio source was unavailable; the remaining source was preserved"
                }),
            );
        }

        // Clean up transcript segments
        if let Ok(mut segments) = self.transcript_segments.lock() {
            segments.clear();
        }

        Ok(Some(final_audio_path.to_string_lossy().to_string()))
    }

    async fn finalize_origin(
        saver: &Option<Arc<AsyncMutex<IncrementalAudioSaver>>>,
        label: &str,
    ) -> (Option<PathBuf>, Option<String>) {
        let Some(saver) = saver.as_ref() else {
            return (None, None);
        };
        match saver.lock().await.finalize().await {
            Ok(path) => (Some(path), None),
            Err(_) => {
                warn!("{} origin could not be finalized", label);
                (None, Some(format!("{label} origin could not be finalized")))
            }
        }
    }

    async fn create_mixed_track(
        &self,
        microphone: Option<&PathBuf>,
        system_audio: Option<&PathBuf>,
    ) -> Result<PathBuf, String> {
        let folder = self
            .meeting_folder
            .as_ref()
            .ok_or_else(|| "Recording folder is unavailable".to_string())?;
        let output = folder.join("audio.mp4");
        let temp = output.with_extension("mixing.mp4");
        let _ = std::fs::remove_file(&temp);

        match (microphone, system_audio) {
            (Some(microphone), Some(system_audio)) => {
                let ffmpeg =
                    super::ffmpeg::find_ffmpeg_path().map_err(|error| error.code().to_string())?;
                let command = ffmpeg.command().map_err(|error| error.code().to_string())?;
                let mut command = tokio::process::Command::from(command);
                let result = command
                    .args([
                        "-i",
                        microphone
                            .to_str()
                            .ok_or_else(|| "Invalid microphone track path".to_string())?,
                        "-i",
                        system_audio
                            .to_str()
                            .ok_or_else(|| "Invalid system-audio track path".to_string())?,
                        "-filter_complex",
                        "amix=inputs=2:duration=longest:normalize=0",
                        "-c:a",
                        "aac",
                        "-y",
                        temp.to_str()
                            .ok_or_else(|| "Invalid mixed track path".to_string())?,
                    ])
                    .output()
                    .await
                    .map_err(|_| "Failed to generate mixed playback track".to_string())?;

                if !result.status.success() {
                    let _ = std::fs::remove_file(&temp);
                    return Err("Failed to generate mixed playback track".to_string());
                }
            }
            (Some(origin), None) | (None, Some(origin)) => {
                std::fs::copy(origin, &temp)
                    .map_err(|_| "Failed to create mixed playback track".to_string())?;
            }
            (None, None) => return Err("No audio samples were stored".to_string()),
        }

        if !temp.exists() || temp.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
            let _ = std::fs::remove_file(&temp);
            return Err("Mixed playback track is empty".to_string());
        }
        std::fs::rename(&temp, &output)
            .map_err(|_| "Failed to install mixed playback track".to_string())?;
        Ok(output)
    }

    /// Get the meeting folder path (for passing to backend)
    pub fn get_meeting_folder(&self) -> Option<&PathBuf> {
        self.meeting_folder.as_ref()
    }

    /// Get accumulated transcript segments (for reload sync)
    pub fn get_transcript_segments(&self) -> Vec<TranscriptSegment> {
        if let Ok(segments) = self.transcript_segments.lock() {
            segments.clone()
        } else {
            Vec::new()
        }
    }

    /// Get meeting name (for reload sync)
    pub fn get_meeting_name(&self) -> Option<String> {
        self.meeting_name.clone()
    }
}

impl Default for RecordingSaver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    fn source_hash(path: &std::path::Path) -> Vec<u8> {
        Sha256::digest(std::fs::read(path).unwrap()).to_vec()
    }

    #[tokio::test]
    async fn creates_playback_track_from_both_origins() {
        let temp_dir = tempdir().unwrap();
        let folder = temp_dir.path().to_path_buf();
        let microphone = folder.join("microphone.mp4");
        let system_audio = folder.join("system-audio.mp4");
        let mut microphone_samples = vec![0.0_f32; 48_000];
        microphone_samples[12_000..12_960].fill(0.8);
        let mut system_samples = vec![0.0_f32; 48_000];
        system_samples[36_000..36_960].fill(0.8);

        super::super::encode::encode_single_audio(
            bytemuck::cast_slice(&microphone_samples),
            48_000,
            1,
            &microphone,
        )
        .unwrap();
        super::super::encode::encode_single_audio(
            bytemuck::cast_slice(&system_samples),
            48_000,
            1,
            &system_audio,
        )
        .unwrap();
        let microphone_hash = source_hash(&microphone);
        let system_hash = source_hash(&system_audio);

        let mut saver = RecordingSaver::new();
        saver.meeting_folder = Some(folder.clone());
        let mixed = saver
            .create_mixed_track(Some(&microphone), Some(&system_audio))
            .await
            .unwrap();

        assert_eq!(mixed, folder.join("audio.mp4"));
        assert!(mixed.metadata().unwrap().len() > 0);
        assert_eq!(source_hash(&microphone), microphone_hash);
        assert_eq!(source_hash(&system_audio), system_hash);

        fn marker_time(path: &std::path::Path, expected_seconds: f64) -> f64 {
            let samples = super::super::decoder::decode_audio_file(path)
                .unwrap()
                .to_whisper_format();
            let start = ((expected_seconds - 0.1) * 16_000.0) as usize;
            let end = (((expected_seconds + 0.1) * 16_000.0) as usize).min(samples.len());
            let index = samples[start..end]
                .iter()
                .enumerate()
                .max_by(|(_, left), (_, right)| left.abs().total_cmp(&right.abs()))
                .unwrap()
                .0
                + start;
            index as f64 / 16_000.0
        }

        for (origin, expected) in [(&microphone, 0.25), (&system_audio, 0.75)] {
            let origin_time = marker_time(origin, expected);
            let mixed_time = marker_time(&mixed, expected);
            assert!(
                (origin_time - mixed_time).abs()
                    <= super::super::constants::MIXED_TRACK_SEEK_TOLERANCE_SECONDS
            );
        }
    }

    #[tokio::test]
    async fn preserves_available_origin_when_the_other_is_lost() {
        let temp_dir = tempdir().unwrap();
        let folder = temp_dir.path().to_path_buf();
        let microphone = folder.join("microphone.mp4");
        std::fs::write(&microphone, b"available-origin").unwrap();

        let mut saver = RecordingSaver::new();
        saver.meeting_folder = Some(folder.clone());
        let mixed = saver
            .create_mixed_track(Some(&microphone), None)
            .await
            .unwrap();

        assert_eq!(std::fs::read(mixed).unwrap(), b"available-origin");
    }

    #[test]
    fn stored_origin_with_a_persistence_failure_remains_partial() {
        assert_eq!(completed_origin_status(true, true), "partial");
        assert_eq!(completed_origin_status(true, false), "stored");
    }

    #[test]
    fn meeting_metadata_persists_microphone_participants() {
        let metadata = MeetingMetadata {
            version: "1.0".to_string(),
            meeting_id: None,
            meeting_name: Some("Snapshot test".to_string()),
            created_at: "2026-08-05T00:00:00Z".to_string(),
            completed_at: None,
            duration_seconds: None,
            devices: DeviceInfo {
                microphone: None,
                system_audio: None,
            },
            audio_file: "audio.mp4".to_string(),
            transcript_file: "transcripts.json".to_string(),
            sample_rate: 48_000,
            status: "starting".to_string(),
            origins: vec![],
            microphone_participants: Some(
                super::super::recording_preferences::MicrophoneParticipantSnapshot {
                    local_speaker_name: "Guilherme".to_string(),
                    microphone_is_shared: false,
                },
            ),
        };

        let serialized = serde_json::to_value(metadata).unwrap();

        assert_eq!(
            serialized["microphone_participants"]["local_speaker_name"],
            "Guilherme"
        );
        assert_eq!(
            serialized["microphone_participants"]["microphone_is_shared"],
            false
        );
    }
}
