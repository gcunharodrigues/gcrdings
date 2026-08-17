use std::ffi::{CStr, CString};
use std::fmt;
use std::path::Path;

#[link(name = "FluidAudioLocalBridge")]
extern "C" {
    fn fluidaudio_local_create() -> *mut std::ffi::c_void;
    fn fluidaudio_local_destroy(bridge: *mut std::ffi::c_void);
    fn fluidaudio_local_initialize_diarization(
        bridge: *mut std::ffi::c_void,
        threshold: f64,
        model_directory: *const i8,
    ) -> i32;
    fn fluidaudio_local_diarize_file(
        bridge: *mut std::ffi::c_void,
        path: *const i8,
        out_speaker_ids: *mut *mut *mut i8,
        out_start_times: *mut *mut f32,
        out_end_times: *mut *mut f32,
        out_quality_scores: *mut *mut f32,
        out_count: *mut u32,
    ) -> i32;
    fn fluidaudio_local_free_result(
        speaker_ids: *mut *mut i8,
        start_times: *mut f32,
        end_times: *mut f32,
        quality_scores: *mut f32,
        count: u32,
    );
}

#[derive(Debug)]
pub enum FluidAudioError {
    InvalidPath,
    BridgeCreation,
    ModelLoad,
    Diarization,
    FileNotFound,
}

impl fmt::Display for FluidAudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPath => "path contains an invalid byte",
            Self::BridgeCreation => "failed to create FluidAudio bridge",
            Self::ModelLoad => "failed to load verified diarization models",
            Self::Diarization => "speaker diarization failed",
            Self::FileNotFound => "audio file not found",
        })
    }
}

impl std::error::Error for FluidAudioError {}

#[derive(Debug, Clone)]
pub struct DiarizationSegment {
    pub speaker_id: String,
    pub start_time: f32,
    pub end_time: f32,
    pub quality_score: f32,
}

pub struct FluidAudio {
    bridge: *mut std::ffi::c_void,
}

impl FluidAudio {
    pub fn new() -> Result<Self, FluidAudioError> {
        let bridge = unsafe { fluidaudio_local_create() };
        if bridge.is_null() {
            Err(FluidAudioError::BridgeCreation)
        } else {
            Ok(Self { bridge })
        }
    }

    pub fn init_diarization_local<P: AsRef<Path>>(
        &self,
        threshold: f64,
        model_directory: P,
    ) -> Result<(), FluidAudioError> {
        let path = CString::new(model_directory.as_ref().to_string_lossy().as_bytes())
            .map_err(|_| FluidAudioError::InvalidPath)?;
        let result = unsafe {
            fluidaudio_local_initialize_diarization(self.bridge, threshold, path.as_ptr())
        };
        (result == 0)
            .then_some(())
            .ok_or(FluidAudioError::ModelLoad)
    }

    pub fn diarize_file<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<Vec<DiarizationSegment>, FluidAudioError> {
        if !path.as_ref().exists() {
            return Err(FluidAudioError::FileNotFound);
        }
        let path = CString::new(path.as_ref().to_string_lossy().as_bytes())
            .map_err(|_| FluidAudioError::InvalidPath)?;
        let mut speaker_ids = std::ptr::null_mut();
        let mut starts = std::ptr::null_mut();
        let mut ends = std::ptr::null_mut();
        let mut scores = std::ptr::null_mut();
        let mut count = 0;

        let result = unsafe {
            fluidaudio_local_diarize_file(
                self.bridge,
                path.as_ptr(),
                &mut speaker_ids,
                &mut starts,
                &mut ends,
                &mut scores,
                &mut count,
            )
        };
        if result != 0 {
            return Err(FluidAudioError::Diarization);
        }

        let mut segments = Vec::with_capacity(count as usize);
        if count > 0 {
            if speaker_ids.is_null() || starts.is_null() || ends.is_null() || scores.is_null() {
                unsafe { fluidaudio_local_free_result(speaker_ids, starts, ends, scores, count) };
                return Err(FluidAudioError::Diarization);
            }
            for index in 0..count as usize {
                let id = unsafe { *speaker_ids.add(index) };
                if id.is_null() {
                    unsafe {
                        fluidaudio_local_free_result(speaker_ids, starts, ends, scores, count)
                    };
                    return Err(FluidAudioError::Diarization);
                }
                segments.push(DiarizationSegment {
                    speaker_id: unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned(),
                    start_time: unsafe { *starts.add(index) },
                    end_time: unsafe { *ends.add(index) },
                    quality_score: unsafe { *scores.add(index) },
                });
            }
            unsafe { fluidaudio_local_free_result(speaker_ids, starts, ends, scores, count) };
        }
        Ok(segments)
    }
}

impl Drop for FluidAudio {
    fn drop(&mut self) {
        unsafe { fluidaudio_local_destroy(self.bridge) };
    }
}
