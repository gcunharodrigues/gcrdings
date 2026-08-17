use anyhow::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

use super::buffer_pool::AudioBufferPool;
use super::devices::AudioDevice;

/// Device type for audio chunks
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceType {
    Microphone,
    System,
}

/// Audio chunk with metadata for processing
#[derive(Debug, Clone)]
pub struct AudioChunk {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: f64,
    pub chunk_id: u64,
    pub device_type: DeviceType,
}

/// Processed audio chunk (post-VAD) for recording
#[derive(Debug, Clone)]
pub struct ProcessedAudioChunk {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub timestamp: f64,
    pub device_type: DeviceType,
}

/// Comprehensive error types for audio system
#[derive(Debug, Clone)]
pub enum AudioError {
    DeviceDisconnected,
    StreamFailed,
    ProcessingFailed,
    TranscriptionFailed,
    ChannelClosed,
    InitializationFailed,
    ConfigurationError,
    PermissionDenied,
    BufferOverflow,
    SampleRateUnsupported,
}

impl AudioError {
    /// Check if error is recoverable (can attempt reconnection)
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Device disconnect is now recoverable - we can attempt reconnection
            AudioError::DeviceDisconnected => true,
            AudioError::StreamFailed => true,
            AudioError::ProcessingFailed => true,
            AudioError::TranscriptionFailed => true,
            AudioError::ChannelClosed => false,
            AudioError::InitializationFailed => false,
            AudioError::ConfigurationError => false,
            AudioError::PermissionDenied => false,
            AudioError::BufferOverflow => true,
            AudioError::SampleRateUnsupported => false,
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> &'static str {
        match self {
            AudioError::DeviceDisconnected => "Audio device was disconnected",
            AudioError::StreamFailed => "Audio stream encountered an error",
            AudioError::ProcessingFailed => "Audio processing failed",
            AudioError::TranscriptionFailed => "Speech transcription failed",
            AudioError::ChannelClosed => "Audio channel was closed unexpectedly",
            AudioError::InitializationFailed => "Failed to initialize audio system",
            AudioError::ConfigurationError => "Audio configuration error",
            AudioError::PermissionDenied => "Microphone permission denied",
            AudioError::BufferOverflow => "Audio buffer overflow",
            AudioError::SampleRateUnsupported => "Audio sample rate not supported",
        }
    }
}

/// Recording statistics
#[derive(Debug, Default)]
pub struct RecordingStats {
    pub chunks_processed: u64,
    pub total_duration: f64,
    pub last_activity: Option<Instant>,
}

/// Unified state management for audio recording
pub struct RecordingState {
    // Core recording state
    is_recording: AtomicBool,
    is_paused: AtomicBool,
    is_reconnecting: AtomicBool, // NEW: Attempting to reconnect to device

    // Audio devices
    microphone_device: Mutex<Option<Arc<AudioDevice>>>,
    system_device: Mutex<Option<Arc<AudioDevice>>>,
    // Track which device is disconnected for reconnection attempts
    disconnected_device: Mutex<Option<(Arc<AudioDevice>, DeviceType)>>,

    // Audio pipeline
    audio_sender: Mutex<Option<mpsc::UnboundedSender<AudioChunk>>>,

    // Memory optimization
    buffer_pool: AudioBufferPool,

    // Error handling
    error_count: AtomicU32,
    recoverable_error_count: AtomicU32,
    microphone_level_bits: AtomicU32,
    system_audio_level_bits: AtomicU32,
    microphone_has_samples: AtomicBool,
    system_audio_has_samples: AtomicBool,
    microphone_persistence_failed: AtomicBool,
    system_audio_persistence_failed: AtomicBool,
    microphone_last_persisted: Mutex<Option<Instant>>,
    system_audio_last_persisted: Mutex<Option<Instant>>,
    last_error: Mutex<Option<AudioError>>,
    // Callback shape is the stable thread-safe error notification boundary.
    #[allow(clippy::type_complexity)]
    error_callback: Mutex<Option<Box<dyn Fn(&AudioError) + Send + Sync>>>,

    // Statistics
    stats: Mutex<RecordingStats>,

    // Recording start time for accurate timestamps
    recording_start: Mutex<Option<Instant>>,
    // Pause time tracking
    pause_start: Mutex<Option<Instant>>,
    total_pause_duration: Mutex<std::time::Duration>,
}

impl RecordingState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_recording: AtomicBool::new(false),
            is_paused: AtomicBool::new(false),
            is_reconnecting: AtomicBool::new(false),
            microphone_device: Mutex::new(None),
            system_device: Mutex::new(None),
            disconnected_device: Mutex::new(None),
            audio_sender: Mutex::new(None),
            buffer_pool: AudioBufferPool::new(16, 48000), // Pool of 16 buffers with 48kHz samples capacity
            error_count: AtomicU32::new(0),
            recoverable_error_count: AtomicU32::new(0),
            microphone_level_bits: AtomicU32::new(0),
            system_audio_level_bits: AtomicU32::new(0),
            microphone_has_samples: AtomicBool::new(false),
            system_audio_has_samples: AtomicBool::new(false),
            microphone_persistence_failed: AtomicBool::new(false),
            system_audio_persistence_failed: AtomicBool::new(false),
            microphone_last_persisted: Mutex::new(None),
            system_audio_last_persisted: Mutex::new(None),
            last_error: Mutex::new(None),
            error_callback: Mutex::new(None),
            stats: Mutex::new(RecordingStats::default()),
            recording_start: Mutex::new(None),
            pause_start: Mutex::new(None),
            total_pause_duration: Mutex::new(std::time::Duration::ZERO),
        })
    }

    // Recording control
    pub fn start_recording(&self) -> Result<()> {
        self.is_recording.store(true, Ordering::SeqCst);
        *self.recording_start.lock().unwrap() = Some(Instant::now());
        self.error_count.store(0, Ordering::SeqCst);
        self.recoverable_error_count.store(0, Ordering::SeqCst);
        self.microphone_level_bits.store(0, Ordering::Relaxed);
        self.system_audio_level_bits.store(0, Ordering::Relaxed);
        self.microphone_has_samples.store(false, Ordering::Relaxed);
        self.system_audio_has_samples
            .store(false, Ordering::Relaxed);
        self.microphone_persistence_failed
            .store(false, Ordering::Relaxed);
        self.system_audio_persistence_failed
            .store(false, Ordering::Relaxed);
        *self.microphone_last_persisted.lock().unwrap() = None;
        *self.system_audio_last_persisted.lock().unwrap() = None;
        *self.last_error.lock().unwrap() = None;
        Ok(())
    }

    pub fn stop_recording(&self) {
        self.is_recording.store(false, Ordering::SeqCst);
        self.is_paused.store(false, Ordering::SeqCst);
        // Clear pause tracking when stopping
        *self.pause_start.lock().unwrap() = None;
        // CRITICAL: Clear audio sender to close the pipeline channel
        // This ensures the pipeline loop exits properly after processing all chunks
        *self.audio_sender.lock().unwrap() = None;
        // CRITICAL: Clear device references to release microphone/speaker
        // Without this, Arc<AudioDevice> references persist and keep the mic active
        *self.microphone_device.lock().unwrap() = None;
        *self.system_device.lock().unwrap() = None;
        *self.disconnected_device.lock().unwrap() = None;
        log::info!("Recording stopped, device references cleared");
    }

    pub fn pause_recording(&self) -> Result<()> {
        if !self.is_recording() {
            return Err(anyhow::anyhow!("Cannot pause when not recording"));
        }
        if self.is_paused() {
            return Err(anyhow::anyhow!("Recording is already paused"));
        }

        self.is_paused.store(true, Ordering::SeqCst);
        *self.pause_start.lock().unwrap() = Some(Instant::now());
        log::info!("Recording paused");
        Ok(())
    }

    pub fn resume_recording(&self) -> Result<()> {
        if !self.is_recording() {
            return Err(anyhow::anyhow!("Cannot resume when not recording"));
        }
        if !self.is_paused() {
            return Err(anyhow::anyhow!("Recording is not paused"));
        }

        // Calculate pause duration and add to total
        if let Some(pause_start) = self.pause_start.lock().unwrap().take() {
            let pause_duration = pause_start.elapsed();
            *self.total_pause_duration.lock().unwrap() += pause_duration;
            log::info!(
                "Recording resumed after pause of {:.2}s",
                pause_duration.as_secs_f64()
            );
        }

        self.is_paused.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused.load(Ordering::SeqCst)
    }

    pub fn is_active(&self) -> bool {
        self.is_recording() && !self.is_paused()
    }

    // Reconnection state management
    pub fn start_reconnecting(&self, device: Arc<AudioDevice>, device_type: DeviceType) {
        self.is_reconnecting.store(true, Ordering::SeqCst);
        *self.disconnected_device.lock().unwrap() = Some((device, device_type));
        log::info!("Started reconnection attempt for device");
    }

    pub fn stop_reconnecting(&self) {
        self.is_reconnecting.store(false, Ordering::SeqCst);
        *self.disconnected_device.lock().unwrap() = None;
        log::info!("Stopped reconnection attempt");
    }

    pub fn is_reconnecting(&self) -> bool {
        self.is_reconnecting.load(Ordering::SeqCst)
    }

    pub fn get_disconnected_device(&self) -> Option<(Arc<AudioDevice>, DeviceType)> {
        self.disconnected_device.lock().unwrap().clone()
    }

    // Device management
    pub fn set_microphone_device(&self, device: Arc<AudioDevice>) {
        *self.microphone_device.lock().unwrap() = Some(device);
    }

    pub fn set_system_device(&self, device: Arc<AudioDevice>) {
        *self.system_device.lock().unwrap() = Some(device);
    }

    pub fn get_microphone_device(&self) -> Option<Arc<AudioDevice>> {
        self.microphone_device.lock().unwrap().clone()
    }

    pub fn get_system_device(&self) -> Option<Arc<AudioDevice>> {
        self.system_device.lock().unwrap().clone()
    }

    // Audio pipeline management
    pub fn set_audio_sender(&self, sender: mpsc::UnboundedSender<AudioChunk>) {
        *self.audio_sender.lock().unwrap() = Some(sender);
    }

    pub fn send_audio_chunk(&self, chunk: AudioChunk) -> Result<()> {
        // Don't send audio chunks when paused
        if self.is_paused() {
            return Ok(()); // Silently discard chunks while paused
        }

        if let Some(sender) = self.audio_sender.lock().unwrap().as_ref() {
            let rms = if chunk.data.is_empty() {
                0.0
            } else {
                (chunk.data.iter().map(|sample| sample * sample).sum::<f32>()
                    / chunk.data.len() as f32)
                    .sqrt()
            };
            let device_type = chunk.device_type.clone();
            sender
                .send(chunk)
                .map_err(|_| anyhow::anyhow!("Failed to send audio chunk"))?;
            match device_type {
                DeviceType::Microphone => {
                    self.microphone_level_bits
                        .store(rms.to_bits(), Ordering::Relaxed);
                }
                DeviceType::System => {
                    self.system_audio_level_bits
                        .store(rms.to_bits(), Ordering::Relaxed);
                }
            }

            // Update statistics
            let mut stats = self.stats.lock().unwrap();
            stats.chunks_processed += 1;
            stats.last_activity = Some(Instant::now());
            Ok(())
        } else {
            // Return an error when no sender is available (pipeline not ready)
            Err(anyhow::anyhow!(
                "Audio pipeline not ready - no sender available"
            ))
        }
    }

    pub fn mark_origin_persisted(&self, device_type: DeviceType) {
        let (has_samples, last_persisted) = match device_type {
            DeviceType::Microphone => (
                &self.microphone_has_samples,
                &self.microphone_last_persisted,
            ),
            DeviceType::System => (
                &self.system_audio_has_samples,
                &self.system_audio_last_persisted,
            ),
        };
        has_samples.store(true, Ordering::Relaxed);
        *last_persisted.lock().unwrap() = Some(Instant::now());
    }

    pub fn mark_origin_failed(&self, device_type: DeviceType) {
        match device_type {
            DeviceType::Microphone => self
                .microphone_persistence_failed
                .store(true, Ordering::Relaxed),
            DeviceType::System => self
                .system_audio_persistence_failed
                .store(true, Ordering::Relaxed),
        }
    }

    pub fn get_source_statuses(&self) -> (&'static str, &'static str) {
        (
            self.source_status(
                &self.microphone_has_samples,
                &self.microphone_persistence_failed,
                &self.microphone_last_persisted,
            ),
            self.source_status(
                &self.system_audio_has_samples,
                &self.system_audio_persistence_failed,
                &self.system_audio_last_persisted,
            ),
        )
    }

    fn source_status(
        &self,
        has_samples: &AtomicBool,
        failed: &AtomicBool,
        last_persisted: &Mutex<Option<Instant>>,
    ) -> &'static str {
        if failed.load(Ordering::Relaxed) {
            "failed"
        } else if !has_samples.load(Ordering::Relaxed) {
            "waiting"
        } else if self.is_paused() {
            "storing"
        } else if last_persisted
            .lock()
            .unwrap()
            .is_some_and(|last| last.elapsed() > std::time::Duration::from_secs(3))
        {
            "lost"
        } else {
            "storing"
        }
    }

    pub fn report_source_error(&self, device_type: DeviceType, error: AudioError) {
        self.mark_origin_failed(device_type);
        *self.last_error.lock().unwrap() = Some(error.clone());
        if let Some(callback) = self.error_callback.lock().unwrap().as_ref() {
            callback(&error);
        }
    }

    // Error handling
    pub fn set_error_callback<F>(&self, callback: F)
    where
        F: Fn(&AudioError) + Send + Sync + 'static,
    {
        *self.error_callback.lock().unwrap() = Some(Box::new(callback));
    }

    pub fn report_error(&self, error: AudioError) {
        let count = self.error_count.fetch_add(1, Ordering::SeqCst) + 1;

        // Track recoverable vs non-recoverable errors separately
        if error.is_recoverable() {
            let recoverable_count = self.recoverable_error_count.fetch_add(1, Ordering::SeqCst) + 1;
            log::warn!(
                "Recoverable audio error ({}): {:?}",
                recoverable_count,
                error
            );

            // Allow more recoverable errors before stopping
            if recoverable_count >= 10 {
                log::error!(
                    "Too many recoverable errors ({}), stopping recording",
                    recoverable_count
                );
                self.stop_recording();
            }
        } else {
            log::error!("Non-recoverable audio error: {:?}", error);
            // Stop immediately for non-recoverable errors
            self.stop_recording();
        }

        *self.last_error.lock().unwrap() = Some(error.clone());

        // Call error callback if set
        if let Some(callback) = self.error_callback.lock().unwrap().as_ref() {
            callback(&error);
        }

        // Fallback: stop recording after too many total errors
        if count >= 15 {
            log::error!(
                "Too many total audio errors ({}), stopping recording",
                count
            );
            self.stop_recording();
        }
    }

    pub fn get_error_count(&self) -> u32 {
        self.error_count.load(Ordering::SeqCst)
    }

    pub fn get_recoverable_error_count(&self) -> u32 {
        self.recoverable_error_count.load(Ordering::SeqCst)
    }

    pub fn get_last_error(&self) -> Option<AudioError> {
        self.last_error.lock().unwrap().clone()
    }

    pub fn has_fatal_error(&self) -> bool {
        if let Some(error) = &*self.last_error.lock().unwrap() {
            !error.is_recoverable() && self.error_count.load(Ordering::SeqCst) > 0
        } else {
            false
        }
    }

    // Statistics
    pub fn get_stats(&self) -> RecordingStats {
        self.stats.lock().unwrap().clone()
    }

    pub fn get_audio_levels(&self) -> (f32, f32) {
        (
            f32::from_bits(self.microphone_level_bits.load(Ordering::Relaxed)),
            f32::from_bits(self.system_audio_level_bits.load(Ordering::Relaxed)),
        )
    }

    pub fn get_recording_duration(&self) -> Option<f64> {
        self.recording_start
            .lock()
            .unwrap()
            .map(|start| start.elapsed().as_secs_f64())
    }

    pub fn get_active_recording_duration(&self) -> Option<f64> {
        self.recording_start.lock().unwrap().map(|start| {
            let total_duration = start.elapsed().as_secs_f64();
            let pause_duration = self.get_total_pause_duration();
            let current_pause = if self.is_paused() {
                self.pause_start
                    .lock()
                    .unwrap()
                    .map(|p| p.elapsed().as_secs_f64())
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            total_duration - pause_duration - current_pause
        })
    }

    pub fn get_total_pause_duration(&self) -> f64 {
        self.total_pause_duration.lock().unwrap().as_secs_f64()
    }

    pub fn get_current_pause_duration(&self) -> Option<f64> {
        if self.is_paused() {
            self.pause_start
                .lock()
                .unwrap()
                .map(|start| start.elapsed().as_secs_f64())
        } else {
            None
        }
    }

    // Memory management
    pub fn get_buffer_pool(&self) -> AudioBufferPool {
        self.buffer_pool.clone()
    }

    // Cleanup
    pub fn cleanup(&self) {
        self.stop_recording();
        self.stop_reconnecting();
        *self.microphone_device.lock().unwrap() = None;
        *self.system_device.lock().unwrap() = None;
        *self.disconnected_device.lock().unwrap() = None;
        *self.audio_sender.lock().unwrap() = None;
        *self.last_error.lock().unwrap() = None;
        *self.error_callback.lock().unwrap() = None;
        *self.stats.lock().unwrap() = RecordingStats::default();
        *self.recording_start.lock().unwrap() = None;
        *self.pause_start.lock().unwrap() = None;
        *self.total_pause_duration.lock().unwrap() = std::time::Duration::ZERO;
        self.error_count.store(0, Ordering::SeqCst);
        self.recoverable_error_count.store(0, Ordering::SeqCst);
        self.microphone_level_bits.store(0, Ordering::Relaxed);
        self.system_audio_level_bits.store(0, Ordering::Relaxed);
        self.microphone_has_samples.store(false, Ordering::Relaxed);
        self.system_audio_has_samples
            .store(false, Ordering::Relaxed);
        self.microphone_persistence_failed
            .store(false, Ordering::Relaxed);
        self.system_audio_persistence_failed
            .store(false, Ordering::Relaxed);
        *self.microphone_last_persisted.lock().unwrap() = None;
        *self.system_audio_last_persisted.lock().unwrap() = None;

        // Clear buffer pool to free memory
        self.buffer_pool.clear();
    }
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            is_paused: AtomicBool::new(false),
            is_reconnecting: AtomicBool::new(false),
            microphone_device: Mutex::new(None),
            system_device: Mutex::new(None),
            disconnected_device: Mutex::new(None),
            audio_sender: Mutex::new(None),
            buffer_pool: AudioBufferPool::new(16, 48000), // Pool of 16 buffers with 48kHz samples capacity
            error_count: AtomicU32::new(0),
            recoverable_error_count: AtomicU32::new(0),
            microphone_level_bits: AtomicU32::new(0),
            system_audio_level_bits: AtomicU32::new(0),
            microphone_has_samples: AtomicBool::new(false),
            system_audio_has_samples: AtomicBool::new(false),
            microphone_persistence_failed: AtomicBool::new(false),
            system_audio_persistence_failed: AtomicBool::new(false),
            microphone_last_persisted: Mutex::new(None),
            system_audio_last_persisted: Mutex::new(None),
            last_error: Mutex::new(None),
            error_callback: Mutex::new(None),
            stats: Mutex::new(RecordingStats::default()),
            recording_start: Mutex::new(None),
            pause_start: Mutex::new(None),
            total_pause_duration: Mutex::new(std::time::Duration::ZERO),
        }
    }
}

// Thread-safe cloning for RecordingStats
impl Clone for RecordingStats {
    fn clone(&self) -> Self {
        Self {
            chunks_processed: self.chunks_processed,
            total_duration: self.total_duration,
            last_activity: self.last_activity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_health_tracks_persistence_and_failure_independently() {
        let state = RecordingState::new();
        state.start_recording().unwrap();
        state.mark_origin_persisted(DeviceType::Microphone);
        assert_eq!(state.get_source_statuses(), ("storing", "waiting"));

        state.report_source_error(DeviceType::System, AudioError::StreamFailed);
        assert_eq!(state.get_source_statuses(), ("storing", "failed"));
        state.mark_origin_persisted(DeviceType::System);
        assert_eq!(state.get_source_statuses(), ("storing", "failed"));
        assert!(state.is_recording());
    }

    #[test]
    fn paused_source_is_not_reported_lost() {
        let state = RecordingState::new();
        state.start_recording().unwrap();
        state.mark_origin_persisted(DeviceType::Microphone);
        *state.microphone_last_persisted.lock().unwrap() =
            Some(Instant::now() - std::time::Duration::from_secs(4));
        state.pause_recording().unwrap();

        assert_eq!(state.get_source_statuses().0, "storing");
    }
}
