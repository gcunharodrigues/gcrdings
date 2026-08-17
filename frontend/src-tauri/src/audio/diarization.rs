use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerSegment {
    pub speaker_id: String,
    pub start_time: f64,
    pub end_time: f64,
}

pub fn speaker_label_for_interval(
    speakers: &[SpeakerSegment],
    start_time: f64,
    end_time: f64,
) -> Option<String> {
    let mut overlapping_speakers = BTreeSet::<&str>::new();

    for segment in speakers {
        let overlap = end_time.min(segment.end_time) - start_time.max(segment.start_time);
        if overlap > 0.0 {
            overlapping_speakers.insert(segment.speaker_id.as_str());
        }
    }

    (!overlapping_speakers.is_empty()).then(|| {
        overlapping_speakers
            .into_iter()
            .collect::<Vec<_>>()
            .join(" + ")
    })
}

#[cfg(target_os = "macos")]
pub async fn diarize_file(path: PathBuf) -> Result<Vec<SpeakerSegment>> {
    let _model_guard = super::diarization_models::processing_guard().await;
    let model_directory = super::diarization_models::verified_model_directory()?;
    tokio::task::spawn_blocking(move || {
        let engine = fluidaudio_rs::FluidAudio::new()
            .map_err(|error| anyhow!("Failed to initialize speaker diarization: {error}"))?;
        engine
            .init_diarization_local(0.6, model_directory)
            .map_err(|error| anyhow!("Failed to load speaker diarization: {error}"))?;

        engine
            .diarize_file(path)
            .map(|segments| {
                segments
                    .into_iter()
                    .map(|segment| SpeakerSegment {
                        speaker_id: segment.speaker_id,
                        start_time: segment.start_time as f64,
                        end_time: segment.end_time as f64,
                    })
                    .collect()
            })
            .map_err(|error| anyhow!("Speaker diarization failed: {error}"))
    })
    .await
    .map_err(|error| anyhow!("Speaker diarization task failed: {error}"))?
}

#[cfg(not(target_os = "macos"))]
pub async fn diarize_file(_path: PathBuf) -> Result<Vec<SpeakerSegment>> {
    Err(anyhow!("Speaker diarization is available on macOS only"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_every_speaker_overlapping_the_transcript_block() {
        let speakers = vec![
            SpeakerSegment {
                speaker_id: "Speaker 1".to_string(),
                start_time: 0.0,
                end_time: 4.0,
            },
            SpeakerSegment {
                speaker_id: "Speaker 2".to_string(),
                start_time: 4.0,
                end_time: 10.0,
            },
        ];

        assert_eq!(
            speaker_label_for_interval(&speakers, 2.0, 8.0),
            Some("Speaker 1 + Speaker 2".to_string())
        );
    }

    #[test]
    fn leaves_non_overlapping_transcripts_unassigned() {
        let speakers = vec![SpeakerSegment {
            speaker_id: "Speaker 1".to_string(),
            start_time: 10.0,
            end_time: 12.0,
        }];

        assert_eq!(speaker_label_for_interval(&speakers, 0.0, 2.0), None);
    }
}
