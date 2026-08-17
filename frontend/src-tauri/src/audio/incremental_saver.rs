use super::encode::encode_single_audio;
use super::recording_state::AudioChunk;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::ffmpeg::find_ffmpeg_path;

/// Audio samples for one persisted origin.
#[derive(Clone)]
struct AudioData {
    data: Vec<f32>,
    // sample_rate: u32,
}

/// Incremental audio saver that writes checkpoints every 30 seconds
/// to minimize memory usage and enable crash recovery
pub struct IncrementalAudioSaver {
    checkpoint_buffer: Vec<AudioData>,
    checkpoint_interval_samples: usize, // 30s at 48kHz = 1,440,000 samples
    checkpoint_count: u32,
    checkpoints_dir: PathBuf,
    meeting_folder: PathBuf,
    sample_rate: u32,
    final_file_name: String,
    next_sample_index: usize,
}

impl IncrementalAudioSaver {
    /// Create a new incremental saver
    ///
    /// # Arguments
    /// * `meeting_folder` - Path to the meeting folder (contains .checkpoints/)
    /// * `sample_rate` - Sample rate of audio (typically 48000)
    pub fn new(meeting_folder: PathBuf, sample_rate: u32) -> Result<Self> {
        let checkpoints_dir = meeting_folder.join(".checkpoints");
        Self::with_paths(
            meeting_folder,
            checkpoints_dir,
            sample_rate,
            "audio.mp4".to_string(),
        )
    }

    pub fn new_for_track(
        meeting_folder: PathBuf,
        sample_rate: u32,
        track_name: &str,
    ) -> Result<Self> {
        let checkpoints_dir = meeting_folder.join(".checkpoints").join(track_name);
        std::fs::create_dir_all(&checkpoints_dir)?;
        Self::with_paths(
            meeting_folder,
            checkpoints_dir,
            sample_rate,
            format!("{track_name}.mp4"),
        )
    }

    fn with_paths(
        meeting_folder: PathBuf,
        checkpoints_dir: PathBuf,
        sample_rate: u32,
        final_file_name: String,
    ) -> Result<Self> {
        // Verify checkpoints directory exists
        if !checkpoints_dir.exists() {
            return Err(anyhow!("Checkpoints directory does not exist"));
        }

        Ok(Self {
            checkpoint_buffer: Vec::new(),
            checkpoint_interval_samples: sample_rate as usize * 30, // 30 seconds
            checkpoint_count: 0,
            checkpoints_dir,
            meeting_folder,
            sample_rate,
            final_file_name,
            next_sample_index: 0,
        })
    }

    /// Add an audio chunk to the buffer
    /// Automatically saves a checkpoint when buffer reaches 30 seconds
    pub fn add_chunk(&mut self, mut chunk: AudioChunk) -> Result<bool> {
        let desired_start = (chunk.timestamp.max(0.0) * self.sample_rate as f64).round() as usize;

        if desired_start > self.next_sample_index {
            self.append_silence(desired_start - self.next_sample_index)?;
            self.next_sample_index = desired_start;
        } else if desired_start < self.next_sample_index {
            let overlap = self.next_sample_index - desired_start;
            if overlap >= chunk.data.len() {
                warn!(
                    "Dropped an overlapping audio-origin chunk ({} samples)",
                    chunk.data.len()
                );
                return Ok(false);
            }
            warn!("Trimmed {} overlapping audio-origin samples", overlap);
            chunk.data.drain(..overlap);
        }

        self.next_sample_index += chunk.data.len();
        self.append_samples(&chunk.data)?;

        // The recording may only be announced after at least one checkpoint
        // exists on disk. Later checkpoints keep the 30-second cadence.
        if self.checkpoint_count == 0 && !self.checkpoint_buffer.is_empty() {
            self.save_checkpoint()?;
            self.checkpoint_buffer.clear();
        }
        Ok(true)
    }

    pub fn buffered_samples(&self) -> Vec<f32> {
        self.checkpoint_buffer
            .iter()
            .flat_map(|chunk| chunk.data.iter().copied())
            .collect()
    }

    fn append_silence(&mut self, mut samples: usize) -> Result<()> {
        while samples > 0 {
            let available = self.checkpoint_interval_samples - self.buffered_sample_count();
            let count = samples.min(available);
            self.checkpoint_buffer.push(AudioData {
                data: vec![0.0; count],
            });
            samples -= count;
            self.flush_full_checkpoint()?;
        }
        Ok(())
    }

    fn append_samples(&mut self, mut samples: &[f32]) -> Result<()> {
        while !samples.is_empty() {
            let available = self.checkpoint_interval_samples - self.buffered_sample_count();
            let count = samples.len().min(available);
            self.checkpoint_buffer.push(AudioData {
                data: samples[..count].to_vec(),
            });
            samples = &samples[count..];
            self.flush_full_checkpoint()?;
        }
        Ok(())
    }

    fn buffered_sample_count(&self) -> usize {
        self.checkpoint_buffer
            .iter()
            .map(|chunk| chunk.data.len())
            .sum()
    }

    fn flush_full_checkpoint(&mut self) -> Result<()> {
        if self.buffered_sample_count() == self.checkpoint_interval_samples {
            self.save_checkpoint()?;
            self.checkpoint_buffer.clear();
        }
        Ok(())
    }

    /// Save current buffer as a checkpoint file
    fn save_checkpoint(&mut self) -> Result<()> {
        // Concatenate all chunks in buffer
        let audio_data: Vec<f32> = self
            .checkpoint_buffer
            .iter()
            .flat_map(|c| &c.data)
            .cloned()
            .collect();

        if audio_data.is_empty() {
            warn!("Attempted to save empty checkpoint, skipping");
            return Ok(());
        }

        // Generate checkpoint filename
        let checkpoint_path = self
            .checkpoints_dir
            .join(format!("audio_chunk_{:03}.mp4", self.checkpoint_count));

        // Encode and save checkpoint
        encode_single_audio(
            bytemuck::cast_slice(&audio_data),
            self.sample_rate,
            1, // mono
            &checkpoint_path,
        )?;

        let duration_seconds = audio_data.len() as f32 / self.sample_rate as f32;
        self.checkpoint_count += 1;

        info!(
            "Saved checkpoint {}: {:.2}s of audio ({} samples)",
            self.checkpoint_count,
            duration_seconds,
            audio_data.len()
        );

        Ok(())
    }

    /// Finalize the recording: save final checkpoint, merge all checkpoints, cleanup
    ///
    /// Returns the path to the final merged audio.mp4 file
    pub async fn finalize(&mut self) -> Result<PathBuf> {
        info!("Finalizing incremental recording...");

        // Save final buffer if not empty
        if !self.checkpoint_buffer.is_empty() {
            info!(
                "Saving final checkpoint with remaining {} chunks",
                self.checkpoint_buffer.len()
            );
            self.save_checkpoint()?;
            self.checkpoint_buffer.clear();
        }

        if self.checkpoint_count == 0 {
            return Err(anyhow!(
                "No audio checkpoints to merge - recording may have failed"
            ));
        }

        // Merge all checkpoints using FFmpeg concat
        let final_audio_path = self.meeting_folder.join(&self.final_file_name);
        self.merge_checkpoints(&final_audio_path).await?;

        // Clean up checkpoints directory
        info!("Cleaning up {} checkpoint files", self.checkpoint_count);
        if let Err(e) = std::fs::remove_dir_all(&self.checkpoints_dir) {
            warn!("Failed to clean up checkpoints directory: {}", e);
            // Non-fatal - user can manually delete
        }

        info!("Finalized recording");

        Ok(final_audio_path)
    }

    /// Merge all checkpoint files into final audio.mp4 using FFmpeg concat
    /// Uses concat demuxer for fast merging without re-encoding
    async fn merge_checkpoints(&self, output: &Path) -> Result<()> {
        info!(
            "Merging {} checkpoints into final audio file...",
            self.checkpoint_count
        );

        // Create concat list file for FFmpeg
        let list_file = self.checkpoints_dir.join("concat_list.txt");
        let mut list_content = String::new();

        for i in 0..self.checkpoint_count {
            let file_name = format!("audio_chunk_{:03}.mp4", i);
            let checkpoint_path = self.checkpoints_dir.join(&file_name);

            // Verify checkpoint exists
            if !checkpoint_path.exists() {
                return Err(anyhow!("Checkpoint file missing"));
            }

            list_content.push_str(&format!("file '{file_name}'\n"));
        }

        std::fs::write(&list_file, list_content)?;

        let ffmpeg_path = find_ffmpeg_path().map_err(|error| anyhow!(error.code()))?;
        // Run FFmpeg concat command
        // Using concat demuxer with copy codec for fast merging (no re-encoding)

        let temp = temporary_output(output);
        let _ = std::fs::remove_file(&temp);
        let mut command = ffmpeg_path
            .command()
            .map_err(|error| anyhow!(error.code()))?;

        command.args([
            "-f",
            "concat", // Use concat demuxer
            "-safe",
            "1",
            "-i",
            "concat_list.txt",
            "-c",
            "copy", // Copy codec - no re-encoding!
            "-y",   // Overwrite output file
            temp.to_str().unwrap(),
        ]);
        command.current_dir(&self.checkpoints_dir);

        // Hide console window on Windows to prevent CMD popup during finalization
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let ffmpeg_output = command.output()?;

        if !ffmpeg_output.status.success() {
            let _ = std::fs::remove_file(&temp);
            error!("FFmpeg merge failed");
            return Err(anyhow!("FFmpeg concat failed"));
        }

        install_output(&temp, output).map_err(anyhow::Error::msg)?;

        info!("Successfully merged {} checkpoints", self.checkpoint_count);

        Ok(())
    }

    /// Get the meeting folder path
    pub fn get_meeting_folder(&self) -> &PathBuf {
        &self.meeting_folder
    }

    /// Get current checkpoint count
    pub fn get_checkpoint_count(&self) -> u32 {
        self.checkpoint_count
    }
}

/// Audio recovery status for transcript recovery feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioRecoveryStatus {
    pub status: String, // "success" | "partial" | "failed" | "none"
    pub chunk_count: u32,
    pub estimated_duration_seconds: f64,
    pub audio_file_path: Option<String>,
    pub message: String,
}

fn temporary_output(output: &Path) -> PathBuf {
    output.with_extension("recovering.mp4")
}

fn install_output(temp: &Path, output: &Path) -> Result<(), String> {
    if !temp.exists() || temp.metadata().map(|meta| meta.len()).unwrap_or(0) == 0 {
        let _ = std::fs::remove_file(temp);
        return Err("Recovered audio output is empty".to_string());
    }
    std::fs::rename(temp, output)
        .map_err(|error| format!("Failed to install recovered audio: {error}"))
}

fn validate_managed_meeting_folder(
    meeting_folder: &str,
    recordings_root: &Path,
) -> Result<PathBuf, String> {
    let folder = PathBuf::from(meeting_folder)
        .canonicalize()
        .map_err(|_| "Recording folder is unavailable".to_string())?;
    let root = recordings_root
        .canonicalize()
        .map_err(|_| "Recordings library is unavailable".to_string())?;
    if folder.parent() != Some(root.as_path()) {
        return Err("Recording folder is outside the recordings library".to_string());
    }
    Ok(folder)
}

fn has_checkpoint_files(folder: &Path) -> Result<bool, String> {
    let checkpoints_dir = folder.join(".checkpoints");
    if !checkpoints_dir.exists() {
        return Ok(false);
    }
    let contains_mp4 = |directory: &Path| -> Result<bool, String> {
        Ok(std::fs::read_dir(directory)
            .map_err(|error| format!("Failed to read checkpoints directory: {error}"))?
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("mp4")))
    };
    let mut found = contains_mp4(&checkpoints_dir)?;
    for track in ["microphone", "system-audio"] {
        let directory = checkpoints_dir.join(track);
        if directory.exists() {
            found |= contains_mp4(&directory)?;
        }
    }
    Ok(found)
}

fn recover_track(checkpoints_dir: &Path, output: &Path) -> Result<u32, String> {
    if !checkpoints_dir.exists() {
        return Ok(0);
    }
    let mut checkpoints: Vec<PathBuf> = std::fs::read_dir(checkpoints_dir)
        .map_err(|error| format!("Failed to read audio checkpoints: {error}"))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("mp4"))
        .collect();
    checkpoints.sort();
    if checkpoints.is_empty() {
        return Ok(0);
    }

    let concat_file = checkpoints_dir.join("concat_list.txt");
    let concat = checkpoints
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| format!("file '{name}'\n"))
                .ok_or_else(|| "Invalid audio checkpoint name".to_string())
        })
        .collect::<Result<String, String>>()?;
    std::fs::write(&concat_file, concat)
        .map_err(|error| format!("Failed to prepare audio recovery: {error}"))?;

    let ffmpeg = find_ffmpeg_path().map_err(|error| error.code().to_string())?;
    let temp = temporary_output(output);
    let _ = std::fs::remove_file(&temp);
    let mut command = ffmpeg.command().map_err(|error| error.code().to_string())?;
    let result = command
        .current_dir(checkpoints_dir)
        .args(["-f", "concat", "-safe", "1", "-i", "concat_list.txt"])
        .args(["-c", "copy", "-y"])
        .arg(&temp)
        .output()
        .map_err(|error| format!("Failed to run audio recovery: {error}"))?;
    let _ = std::fs::remove_file(concat_file);
    if !result.status.success() {
        let _ = std::fs::remove_file(&temp);
        return Err("Failed to merge audio checkpoints".to_string());
    }
    install_output(&temp, output)?;
    Ok(checkpoints.len() as u32)
}

fn origin_recovery_is_partial(directory: &Path, result: &Result<u32, String>) -> bool {
    directory.exists() && !matches!(result, Ok(count) if *count > 0)
}

/// Recover each persisted origin independently, then recreate the playback mix.
#[tauri::command]
pub async fn recover_audio_from_checkpoints(
    meeting_folder: String,
    _sample_rate: u32,
) -> Result<AudioRecoveryStatus, String> {
    let recordings_root = super::recording_preferences::get_default_recordings_folder();
    let folder = validate_managed_meeting_folder(&meeting_folder, &recordings_root)?;
    let checkpoints = folder.join(".checkpoints");
    if !checkpoints.exists() {
        return Ok(AudioRecoveryStatus {
            status: "none".to_string(),
            chunk_count: 0,
            estimated_duration_seconds: 0.0,
            audio_file_path: None,
            message: "No audio checkpoints found".to_string(),
        });
    }

    let microphone = folder.join("microphone.mp4");
    let system_audio = folder.join("system-audio.mp4");
    let microphone_result = recover_track(&checkpoints.join("microphone"), &microphone);
    let system_result = recover_track(&checkpoints.join("system-audio"), &system_audio);
    let microphone_ok = matches!(microphone_result, Ok(count) if count > 0);
    let system_ok = matches!(system_result, Ok(count) if count > 0);
    let mut chunk_count = microphone_result.as_ref().copied().unwrap_or(0)
        + system_result.as_ref().copied().unwrap_or(0);
    let partial = origin_recovery_is_partial(&checkpoints.join("microphone"), &microphone_result)
        || origin_recovery_is_partial(&checkpoints.join("system-audio"), &system_result);

    // Preserve compatibility with recordings made before origins were split.
    let playback = folder.join("audio.mp4");
    if chunk_count == 0 {
        chunk_count = recover_track(&checkpoints, &playback)?;
    } else {
        match (microphone_ok, system_ok) {
            (true, true) => {
                let ffmpeg = find_ffmpeg_path().map_err(|error| error.code().to_string())?;
                let temp = temporary_output(&playback);
                let _ = std::fs::remove_file(&temp);
                let mut command = ffmpeg.command().map_err(|error| error.code().to_string())?;
                let result = command
                    .arg("-i")
                    .arg(&microphone)
                    .arg("-i")
                    .arg(&system_audio)
                    .args([
                        "-filter_complex",
                        "amix=inputs=2:duration=longest:normalize=0",
                        "-c:a",
                        "aac",
                        "-y",
                    ])
                    .arg(&temp)
                    .output()
                    .map_err(|error| {
                        format!("Failed to create recovered playback audio: {error}")
                    })?;
                if !result.status.success() {
                    let _ = std::fs::remove_file(&temp);
                    return Err("Failed to create recovered playback audio".to_string());
                }
                install_output(&temp, &playback)?;
            }
            (true, false) => {
                let temp = temporary_output(&playback);
                std::fs::copy(&microphone, &temp)
                    .map_err(|error| format!("Failed to recover microphone audio: {error}"))?;
                install_output(&temp, &playback)?;
            }
            (false, true) => {
                let temp = temporary_output(&playback);
                std::fs::copy(&system_audio, &temp)
                    .map_err(|error| format!("Failed to recover system audio: {error}"))?;
                install_output(&temp, &playback)?;
            }
            (false, false) => {}
        }
    }

    if chunk_count == 0 || !playback.exists() {
        return Ok(AudioRecoveryStatus {
            status: "none".to_string(),
            chunk_count: 0,
            estimated_duration_seconds: 0.0,
            audio_file_path: None,
            message: "No audio checkpoint files found".to_string(),
        });
    }

    Ok(AudioRecoveryStatus {
        status: if partial { "partial" } else { "success" }.to_string(),
        chunk_count,
        estimated_duration_seconds: super::decoder::decode_audio_file(&playback)
            .map(|audio| audio.duration_seconds)
            .unwrap_or(0.0),
        audio_file_path: Some(playback.to_string_lossy().to_string()),
        message: format!("Recovered {chunk_count} audio chunks"),
    })
}

/// Clean up checkpoint files after successful recording or recovery
/// This command is called by the frontend after successful save to clean up checkpoint files
#[tauri::command]
pub async fn cleanup_checkpoints(meeting_folder: String) -> Result<(), String> {
    let recordings_root = super::recording_preferences::get_default_recordings_folder();
    let folder_path = validate_managed_meeting_folder(&meeting_folder, &recordings_root)?;
    let checkpoints_dir = folder_path.join(".checkpoints");

    if checkpoints_dir.exists() {
        std::fs::remove_dir_all(&checkpoints_dir)
            .map_err(|e| format!("Failed to remove checkpoints directory: {}", e))?;
        info!("Successfully cleaned up checkpoints directory");
    } else {
        info!("No checkpoints directory to clean up");
    }

    Ok(())
}

/// Check if a meeting folder has audio checkpoint files
/// Returns true if .checkpoints/ directory exists and contains .mp4 files
#[tauri::command]
pub async fn has_audio_checkpoints(meeting_folder: String) -> Result<bool, String> {
    let recordings_root = super::recording_preferences::get_default_recordings_folder();
    let folder_path = validate_managed_meeting_folder(&meeting_folder, &recordings_root)?;
    has_checkpoint_files(&folder_path)
}

#[cfg(test)]
mod tests {
    use super::super::recording_state::DeviceType;
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_checkpoint_creation() {
        // Create temp meeting folder
        let temp_dir = tempdir().unwrap();
        let meeting_folder = temp_dir.path().join("Test_Meeting");
        std::fs::create_dir_all(&meeting_folder).unwrap();
        std::fs::create_dir_all(meeting_folder.join(".checkpoints")).unwrap();

        let mut saver = IncrementalAudioSaver::new(meeting_folder.clone(), 48000).unwrap();

        // Add 60 seconds worth of audio (should create 2 checkpoints)
        for i in 0..120 {
            // 120 chunks of 0.5s each
            let chunk = AudioChunk {
                data: vec![0.5f32; 24000], // 0.5s at 48kHz
                sample_rate: 48000,
                timestamp: i as f64 * 0.5, // timestamp in seconds
                chunk_id: i as u64,
                device_type: DeviceType::Microphone,
            };
            saver.add_chunk(chunk).unwrap();
        }

        // Verify 2 checkpoints created
        assert_eq!(saver.checkpoint_count, 2);

        // Finalize and verify merge
        let final_path = saver.finalize().await.unwrap();
        assert!(final_path.exists());

        // Verify checkpoints directory deleted
        assert!(!meeting_folder.join(".checkpoints").exists());
    }

    #[tokio::test]
    async fn test_empty_recording() {
        let temp_dir = tempdir().unwrap();
        let meeting_folder = temp_dir.path().join("Empty_Test");
        std::fs::create_dir_all(&meeting_folder).unwrap();
        std::fs::create_dir_all(meeting_folder.join(".checkpoints")).unwrap();

        let mut saver = IncrementalAudioSaver::new(meeting_folder.clone(), 48000).unwrap();

        // Try to finalize without adding any chunks
        let result = saver.finalize().await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No audio checkpoints"));
    }

    #[test]
    fn aligns_origin_chunks_to_the_recording_timeline() {
        let temp_dir = tempdir().unwrap();
        let meeting_folder = temp_dir.path().join("Aligned_Recording");
        std::fs::create_dir_all(meeting_folder.join(".checkpoints")).unwrap();

        let mut saver =
            IncrementalAudioSaver::new_for_track(meeting_folder, 4, "microphone").unwrap();
        saver.checkpoint_count = 1;

        saver
            .add_chunk(AudioChunk {
                data: vec![1.0, 1.0],
                sample_rate: 4,
                timestamp: 0.0,
                chunk_id: 1,
                device_type: DeviceType::Microphone,
            })
            .unwrap();
        saver
            .add_chunk(AudioChunk {
                data: vec![2.0, 2.0],
                sample_rate: 4,
                timestamp: 1.0,
                chunk_id: 2,
                device_type: DeviceType::Microphone,
            })
            .unwrap();

        assert_eq!(saver.buffered_samples(), vec![1.0, 1.0, 0.0, 0.0, 2.0, 2.0]);
    }

    #[test]
    fn first_origin_chunk_is_checkpointed_before_recording_is_announced() {
        let temp_dir = tempdir().unwrap();
        let folder = temp_dir.path().join("Durable_Start");
        std::fs::create_dir_all(&folder).unwrap();
        let mut saver = IncrementalAudioSaver::new_for_track(folder, 48_000, "microphone").unwrap();

        saver
            .add_chunk(AudioChunk {
                data: vec![0.0; 480],
                sample_rate: 48_000,
                timestamp: 0.0,
                chunk_id: 1,
                device_type: DeviceType::Microphone,
            })
            .unwrap();

        assert_eq!(saver.get_checkpoint_count(), 1);
        assert!(saver.checkpoints_dir.join("audio_chunk_000.mp4").exists());
    }

    #[test]
    fn detects_origin_checkpoints_after_a_crash() {
        let temp_dir = tempdir().unwrap();
        let folder = temp_dir.path().join("Recoverable");
        let track = folder.join(".checkpoints/microphone");
        std::fs::create_dir_all(&track).unwrap();
        std::fs::write(track.join("audio_chunk_000.mp4"), b"checkpoint").unwrap();

        assert!(has_checkpoint_files(&folder).unwrap());
    }

    #[test]
    fn rejects_recovery_outside_the_recordings_root() {
        let root = tempdir().unwrap();
        let outside = tempdir().unwrap();
        assert!(
            validate_managed_meeting_folder(outside.path().to_str().unwrap(), root.path()).is_err()
        );
    }

    #[test]
    fn failed_recovery_preserves_an_existing_track() {
        if find_ffmpeg_path().is_err() {
            return;
        }
        let temp_dir = tempdir().unwrap();
        let checkpoints = temp_dir.path().join("checkpoints");
        std::fs::create_dir_all(&checkpoints).unwrap();
        std::fs::write(checkpoints.join("audio_chunk_000.mp4"), b"not-media").unwrap();
        let output = temp_dir.path().join("audio.mp4");
        std::fs::write(&output, b"known-good").unwrap();

        assert!(recover_track(&checkpoints, &output).is_err());
        assert_eq!(std::fs::read(output).unwrap(), b"known-good");
    }

    #[test]
    fn empty_selected_origin_makes_recovery_partial() {
        let temp_dir = tempdir().unwrap();
        let empty_origin = temp_dir.path().join("system-audio");
        std::fs::create_dir(&empty_origin).unwrap();

        assert!(origin_recovery_is_partial(&empty_origin, &Ok(0)));
        assert!(!origin_recovery_is_partial(
            &temp_dir.path().join("microphone"),
            &Ok(0)
        ));
    }

    #[test]
    fn aligns_two_origins_to_one_measured_timeline() {
        let temp_dir = tempdir().unwrap();
        let folder = temp_dir.path().join("Synchronized");
        std::fs::create_dir_all(&folder).unwrap();
        let mut microphone =
            IncrementalAudioSaver::new_for_track(folder.clone(), 10, "microphone").unwrap();
        let mut system = IncrementalAudioSaver::new_for_track(folder, 10, "system-audio").unwrap();
        microphone.checkpoint_count = 1;
        system.checkpoint_count = 1;

        microphone
            .add_chunk(AudioChunk {
                data: vec![0.0, 0.0, 0.0, 1.0, 0.0],
                sample_rate: 10,
                timestamp: 0.0,
                chunk_id: 1,
                device_type: DeviceType::Microphone,
            })
            .unwrap();
        system
            .add_chunk(AudioChunk {
                data: vec![0.0, 1.0, 0.0],
                sample_rate: 10,
                timestamp: 0.2,
                chunk_id: 1,
                device_type: DeviceType::System,
            })
            .unwrap();

        let microphone_marker = microphone
            .buffered_samples()
            .iter()
            .position(|sample| *sample == 1.0);
        let system_marker = system
            .buffered_samples()
            .iter()
            .position(|sample| *sample == 1.0);
        assert_eq!(microphone_marker, system_marker);
        assert_eq!(microphone_marker, Some(3));
    }
}
