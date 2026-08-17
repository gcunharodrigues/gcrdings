use crate::api::TranscriptSegment;
use crate::database::models::ProcessingJob;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StartProcessingJob {
    pub meeting_id: String,
    pub language: Option<String>,
    pub provider: String,
    pub model: String,
    pub input_manifest_hash: String,
    pub source_origins: Vec<String>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq, Serialize)]
#[serde(tag = "code", content = "message", rename_all = "snake_case")]
pub enum ProcessingJobError {
    #[error("Processing is already active for this session")]
    Duplicate,
    #[error("Processing job was not found")]
    NotFound,
    #[error("Processing input is invalid")]
    InvalidInput,
    #[error("Source audio changed during processing")]
    SourceChanged,
    #[error("Speaker labels could not be verified")]
    MalformedDiarization,
    #[error("Audio processing failed")]
    ProcessingFailed,
    #[error("Processing was cancelled")]
    Cancelled,
    #[error("Processing storage failed")]
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingJobWarning {
    SpeakerLabelsUnavailable,
    PartialOriginUnavailable,
}

impl ProcessingJobWarning {
    fn code(self) -> &'static str {
        match self {
            Self::SpeakerLabelsUnavailable => "speaker_labels_unavailable",
            Self::PartialOriginUnavailable => "partial_origin_unavailable",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::SpeakerLabelsUnavailable => "Transcript saved without verified speaker labels",
            Self::PartialOriginUnavailable => "Transcript saved with one unavailable audio origin",
        }
    }
}

impl ProcessingJobError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Duplicate => "duplicate",
            Self::NotFound => "not_found",
            Self::InvalidInput => "invalid_input",
            Self::SourceChanged => "source_changed",
            Self::MalformedDiarization => "malformed_diarization",
            Self::ProcessingFailed => "processing_failed",
            Self::Cancelled => "cancelled",
            Self::Storage => "storage",
        }
    }
}

#[derive(FromRow)]
struct ProcessingJobRow {
    id: String,
    meeting_id: String,
    status: String,
    language: Option<String>,
    provider: String,
    model: String,
    input_manifest_hash: String,
    source_origins: String,
    error_code: Option<String>,
    error_message: Option<String>,
    warning_code: Option<String>,
    warning_message: Option<String>,
}

impl TryFrom<ProcessingJobRow> for ProcessingJob {
    type Error = ProcessingJobError;

    fn try_from(row: ProcessingJobRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            meeting_id: row.meeting_id,
            status: row.status,
            language: row.language,
            provider: row.provider,
            model: row.model,
            input_manifest_hash: row.input_manifest_hash,
            source_origins: serde_json::from_str(&row.source_origins)
                .map_err(|_| ProcessingJobError::Storage)?,
            error_code: row.error_code,
            error_message: row.error_message,
            warning_code: row.warning_code,
            warning_message: row.warning_message,
        })
    }
}

pub struct ProcessingJobsRepository;

impl ProcessingJobsRepository {
    pub async fn start(
        pool: &SqlitePool,
        request: StartProcessingJob,
    ) -> Result<ProcessingJob, ProcessingJobError> {
        if request.meeting_id.trim().is_empty()
            || request.provider.trim().is_empty()
            || request.model.trim().is_empty()
            || request.input_manifest_hash.len() != 64
            || !request
                .input_manifest_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || request.source_origins.is_empty()
            || request
                .source_origins
                .iter()
                .any(|origin| origin.trim().is_empty())
        {
            return Err(ProcessingJobError::InvalidInput);
        }

        let id = format!("processing-job-{}", Uuid::new_v4());
        let origins = serde_json::to_string(&request.source_origins)
            .map_err(|_| ProcessingJobError::InvalidInput)?;
        let result = sqlx::query(
            "INSERT INTO processing_jobs (id, meeting_id, status, language, provider, model, input_manifest_hash, source_origins) VALUES (?, ?, 'running', ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&request.meeting_id)
        .bind(&request.language)
        .bind(&request.provider)
        .bind(&request.model)
        .bind(&request.input_manifest_hash)
        .bind(origins)
        .execute(pool)
        .await;

        match result {
            Ok(_) => Self::get(pool, &id).await,
            Err(error)
                if error
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation()) =>
            {
                Err(ProcessingJobError::Duplicate)
            }
            Err(_) => Err(ProcessingJobError::Storage),
        }
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<ProcessingJob, ProcessingJobError> {
        let row = sqlx::query_as::<_, ProcessingJobRow>(
            "SELECT id, meeting_id, status, language, provider, model, input_manifest_hash, source_origins, error_code, error_message, warning_code, warning_message FROM processing_jobs WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ProcessingJobError::Storage)?
        .ok_or(ProcessingJobError::NotFound)?;
        row.try_into()
    }

    pub async fn recover_interrupted(pool: &SqlitePool) -> Result<u64, ProcessingJobError> {
        sqlx::query("UPDATE processing_jobs SET status = 'interrupted', error_code = 'interrupted', error_message = 'Processing was interrupted and can be retried', updated_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP WHERE status = 'running'")
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
            .map_err(|_| ProcessingJobError::Storage)
    }

    pub async fn fail(
        pool: &SqlitePool,
        id: &str,
        error: ProcessingJobError,
    ) -> Result<(), ProcessingJobError> {
        let result = sqlx::query("UPDATE processing_jobs SET status = 'failed', error_code = ?, error_message = ?, updated_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'running'")
            .bind(error.code())
            .bind(error.to_string())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(ProcessingJobError::NotFound)
        }
    }

    pub async fn warn(
        pool: &SqlitePool,
        id: &str,
        warning: ProcessingJobWarning,
    ) -> Result<(), ProcessingJobError> {
        let result = sqlx::query("UPDATE processing_jobs SET warning_code = ?, warning_message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'running'")
            .bind(warning.code())
            .bind(warning.message())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(ProcessingJobError::NotFound)
        }
    }

    pub async fn complete_with_transcript(
        pool: &SqlitePool,
        id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<(), ProcessingJobError> {
        if segments.is_empty() {
            return Err(ProcessingJobError::InvalidInput);
        }
        let mut transaction = pool
            .begin()
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        let job = sqlx::query_as::<_, ProcessingJobRow>(
            "SELECT id, meeting_id, status, language, provider, model, input_manifest_hash, source_origins, error_code, error_message, warning_code, warning_message FROM processing_jobs WHERE id = ? AND status = 'running'",
        )
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| ProcessingJobError::Storage)?
        .ok_or(ProcessingJobError::NotFound)?;

        sqlx::query("DELETE FROM transcripts WHERE meeting_id = ?")
            .bind(&job.meeting_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        for segment in segments {
            sqlx::query(
                "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, speaker, source_origin, language, speaker_cluster_id, ambiguity_group_id, alignment_decision, audio_start_time, audio_end_time, duration) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&segment.id)
            .bind(&job.meeting_id)
            .bind(&segment.text)
            .bind(&segment.timestamp)
            .bind(&segment.speaker)
            .bind(&segment.source_origin)
            .bind(&job.language)
            .bind(&segment.speaker_cluster_id)
            .bind(&segment.ambiguity_group_id)
            .bind(&segment.alignment_decision)
            .bind(segment.audio_start_time)
            .bind(segment.audio_end_time)
            .bind(segment.duration)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        }
        sqlx::query("UPDATE meetings SET principal_transcript_revision = principal_transcript_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(&job.meeting_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        sqlx::query("UPDATE processing_jobs SET status = 'completed', updated_at = CURRENT_TIMESTAMP, completed_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProcessingJobError::Storage)?;
        transaction
            .commit()
            .await
            .map_err(|_| ProcessingJobError::Storage)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::TranscriptSegment;
    use crate::database::repositories::processing_job::{
        ProcessingJobError, ProcessingJobWarning, ProcessingJobsRepository, StartProcessingJob,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fixture() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('meeting-1', 'Imported audio', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn request() -> StartProcessingJob {
        StartProcessingJob {
            meeting_id: "meeting-1".into(),
            language: Some("pt".into()),
            provider: "whisper".into(),
            model: "base".into(),
            input_manifest_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .into(),
            source_origins: vec!["imported".into()],
        }
    }

    #[tokio::test]
    async fn processing_job_is_durable_and_duplicate_start_is_rejected() {
        let pool = fixture().await;
        let job = ProcessingJobsRepository::start(&pool, request())
            .await
            .unwrap();

        assert_eq!(job.status, "running");
        assert_eq!(job.language.as_deref(), Some("pt"));
        assert_eq!(job.source_origins, vec!["imported"]);
        assert_eq!(
            ProcessingJobsRepository::start(&pool, request()).await,
            Err(ProcessingJobError::Duplicate)
        );
        assert_eq!(
            ProcessingJobsRepository::get(&pool, &job.id).await.unwrap(),
            job
        );
    }

    #[tokio::test]
    async fn restart_interrupts_running_jobs_and_allows_one_retry() {
        let pool = fixture().await;
        let first = ProcessingJobsRepository::start(&pool, request())
            .await
            .unwrap();

        assert_eq!(
            ProcessingJobsRepository::recover_interrupted(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            ProcessingJobsRepository::get(&pool, &first.id)
                .await
                .unwrap()
                .status,
            "interrupted"
        );
        assert!(ProcessingJobsRepository::start(&pool, request())
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn stored_failure_is_typed_and_does_not_leak_private_paths() {
        let pool = fixture().await;
        let job = ProcessingJobsRepository::start(&pool, request())
            .await
            .unwrap();

        ProcessingJobsRepository::fail(&pool, &job.id, ProcessingJobError::MalformedDiarization)
            .await
            .unwrap();
        let failed = ProcessingJobsRepository::get(&pool, &job.id).await.unwrap();

        assert_eq!(failed.error_code.as_deref(), Some("malformed_diarization"));
        assert_eq!(
            failed.error_message.as_deref(),
            Some("Speaker labels could not be verified")
        );
        assert!(!failed.error_message.unwrap().contains('/'));
    }

    #[tokio::test]
    async fn principal_transcript_replacement_is_atomic_and_keeps_metadata() {
        let pool = fixture().await;
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, source_origin) VALUES ('old', 'meeting-1', 'old text', '0', 'imported')")
            .execute(&pool)
            .await
            .unwrap();
        let job = ProcessingJobsRepository::start(&pool, request())
            .await
            .unwrap();
        let invalid = vec![
            segment("duplicate", "first"),
            segment("duplicate", "second"),
        ];

        assert_eq!(
            ProcessingJobsRepository::complete_with_transcript(&pool, &job.id, &invalid).await,
            Err(ProcessingJobError::Storage)
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT transcript FROM transcripts WHERE meeting_id = 'meeting-1'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "old text"
        );

        ProcessingJobsRepository::complete_with_transcript(
            &pool,
            &job.id,
            &[segment("new", "new text")],
        )
        .await
        .unwrap();
        let saved: (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT transcript, language, source_origin FROM transcripts WHERE meeting_id = 'meeting-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            saved,
            (
                "new text".into(),
                Some("pt".into()),
                Some("imported".into())
            )
        );
        assert_eq!(
            ProcessingJobsRepository::get(&pool, &job.id)
                .await
                .unwrap()
                .status,
            "completed"
        );
    }

    #[tokio::test]
    async fn valid_transcript_with_failed_diarization_completes_with_typed_warning() {
        let pool = fixture().await;
        let job = ProcessingJobsRepository::start(&pool, request())
            .await
            .unwrap();

        ProcessingJobsRepository::warn(
            &pool,
            &job.id,
            ProcessingJobWarning::SpeakerLabelsUnavailable,
        )
        .await
        .unwrap();
        ProcessingJobsRepository::complete_with_transcript(
            &pool,
            &job.id,
            &[segment("new", "verified text")],
        )
        .await
        .unwrap();
        let completed = ProcessingJobsRepository::get(&pool, &job.id).await.unwrap();

        assert_eq!(completed.status, "completed");
        assert_eq!(
            completed.warning_code.as_deref(),
            Some("speaker_labels_unavailable")
        );
        assert_eq!(completed.error_code, None);
    }

    fn segment(id: &str, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            text: text.into(),
            timestamp: "0".into(),
            speaker: None,
            source_origin: Some("imported".into()),
            speaker_cluster_id: None,
            ambiguity_group_id: None,
            alignment_decision: None,
            audio_start_time: Some(0.0),
            audio_end_time: Some(1.0),
            duration: Some(1.0),
        }
    }
}
