use super::models::{
    FailureCode, GeneratedRecord, GenerationStatus, RecordBlock, RecordType, UnavailableReason,
    VerifiableRecord, VerifiableRecordError, VERIFIABLE_RECORD_SCHEMA_VERSION,
};
use crate::database::models::Participant;
use sqlx::{FromRow, SqlitePool};

const PROMPT_VERSION: &str = "local-findings-v1";

#[derive(FromRow)]
struct GenerationRow {
    record_type: String,
    status: String,
    input_revision: Option<i64>,
    result_json: Option<String>,
    error_code: Option<String>,
}

#[derive(FromRow)]
struct BlockRow {
    id: String,
    text: String,
    participant_id: Option<String>,
    audio_start_time: Option<f64>,
    audio_end_time: Option<f64>,
}

pub struct VerifiableRecordRepository;

impl VerifiableRecordRepository {
    pub async fn get(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<VerifiableRecord, VerifiableRecordError> {
        let mut transaction = pool
            .begin()
            .await
            .map_err(|_| VerifiableRecordError::Storage)?;
        let meeting: Option<(String, i64)> = sqlx::query_as(
            "SELECT title, principal_transcript_revision FROM meetings WHERE id = ?",
        )
        .bind(meeting_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| VerifiableRecordError::Storage)?;
        let (title, revision) = meeting.ok_or(VerifiableRecordError::NotFound)?;
        let participants = sqlx::query_as::<_, Participant>("SELECT id, display_name, speaker_cluster_id FROM participants WHERE meeting_id = ? ORDER BY id")
            .bind(meeting_id).fetch_all(&mut *transaction).await.map_err(|_| VerifiableRecordError::Storage)?;
        let blocks = sqlx::query_as::<_, BlockRow>("SELECT id, transcript AS text, participant_id, audio_start_time, audio_end_time FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time, audio_end_time, id")
            .bind(meeting_id).fetch_all(&mut *transaction).await.map_err(|_| VerifiableRecordError::Storage)?;
        let transcript = blocks
            .into_iter()
            .map(|block| {
                let participant_id = block
                    .participant_id
                    .ok_or(VerifiableRecordError::InvalidOutput)?;
                let start = block
                    .audio_start_time
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .ok_or(VerifiableRecordError::InvalidOutput)?;
                let end = block
                    .audio_end_time
                    .filter(|value| value.is_finite() && *value >= start)
                    .ok_or(VerifiableRecordError::InvalidOutput)?;
                Ok(RecordBlock {
                    id: block.id,
                    text: block.text,
                    participant_id,
                    start_ms: (start * 1000.0).round() as u64,
                    end_ms: (end * 1000.0).round() as u64,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let generation = sqlx::query_as::<_, GenerationRow>("SELECT record_type, status, input_revision, result_json, error_code FROM record_generations WHERE meeting_id = ?")
            .bind(meeting_id).fetch_optional(&mut *transaction).await.map_err(|_| VerifiableRecordError::Storage)?;
        let (record_type, mut status, input_revision, result_json, error_code) = match generation {
            Some(row) => (
                RecordType::parse(&row.record_type)?,
                GenerationStatus::parse(&row.status)?,
                row.input_revision,
                row.result_json,
                row.error_code,
            ),
            None => (
                RecordType::Meeting,
                GenerationStatus::Pending,
                None,
                None,
                None,
            ),
        };
        if input_revision.is_some_and(|input| input != revision) && result_json.is_some() {
            status = GenerationStatus::Stale;
        }
        let generated = if status == GenerationStatus::Completed {
            let json = result_json.ok_or(VerifiableRecordError::InvalidOutput)?;
            Some(serde_json::from_str(&json).map_err(|_| VerifiableRecordError::InvalidOutput)?)
        } else {
            None
        };
        transaction
            .commit()
            .await
            .map_err(|_| VerifiableRecordError::Storage)?;
        Ok(VerifiableRecord {
            schema_version: VERIFIABLE_RECORD_SCHEMA_VERSION,
            meeting_id: meeting_id.into(),
            title,
            record_type,
            principal_transcript_revision: revision,
            participants,
            transcript,
            generation_status: status,
            generated,
            error_code,
        })
    }

    pub async fn set_record_type(
        pool: &SqlitePool,
        meeting_id: &str,
        record_type: RecordType,
    ) -> Result<(), VerifiableRecordError> {
        let mut transaction = pool
            .begin()
            .await
            .map_err(|_| VerifiableRecordError::Storage)?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM meetings WHERE id = ?)")
            .bind(meeting_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| VerifiableRecordError::Storage)?;
        if !exists {
            return Err(VerifiableRecordError::NotFound);
        }
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM record_generations WHERE meeting_id = ? AND status = 'processing')")
            .bind(meeting_id).fetch_one(&mut *transaction).await.map_err(|_| VerifiableRecordError::Storage)?;
        if active {
            return Err(VerifiableRecordError::AlreadyProcessing);
        }
        sqlx::query("INSERT INTO record_generations (meeting_id, record_type, status, prompt_version) VALUES (?, ?, 'pending', ?) ON CONFLICT(meeting_id) DO UPDATE SET record_type = excluded.record_type, status = CASE WHEN record_generations.status = 'processing' THEN record_generations.status ELSE 'pending' END, result_json = CASE WHEN record_generations.record_type = excluded.record_type THEN record_generations.result_json ELSE NULL END, input_revision = CASE WHEN record_generations.record_type = excluded.record_type THEN record_generations.input_revision ELSE NULL END, error_code = NULL, updated_at = CURRENT_TIMESTAMP")
            .bind(meeting_id).bind(record_type.as_str()).bind(PROMPT_VERSION).execute(&mut *transaction).await.map_err(|_| VerifiableRecordError::Storage)?;
        transaction
            .commit()
            .await
            .map_err(|_| VerifiableRecordError::Storage)
    }

    pub async fn begin(
        pool: &SqlitePool,
        meeting_id: &str,
        record_type: RecordType,
        expected_revision: i64,
    ) -> Result<(), VerifiableRecordError> {
        let mut tx = pool
            .begin()
            .await
            .map_err(|_| VerifiableRecordError::Storage)?;
        let revision: Option<i64> =
            sqlx::query_scalar("SELECT principal_transcript_revision FROM meetings WHERE id = ?")
                .bind(meeting_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|_| VerifiableRecordError::Storage)?;
        let revision = revision.ok_or(VerifiableRecordError::NotFound)?;
        if revision != expected_revision {
            return Err(VerifiableRecordError::StaleRevision);
        }
        let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM record_generations WHERE meeting_id = ? AND status = 'processing')").bind(meeting_id).fetch_one(&mut *tx).await.map_err(|_| VerifiableRecordError::Storage)?;
        if active {
            return Err(VerifiableRecordError::AlreadyProcessing);
        }
        let request_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO record_generations (meeting_id, record_type, status, prompt_version, active_revision, active_request_id) VALUES (?, ?, 'processing', ?, ?, ?) ON CONFLICT(meeting_id) DO UPDATE SET record_type = excluded.record_type, status = 'processing', prompt_version = excluded.prompt_version, active_revision = excluded.active_revision, active_request_id = excluded.active_request_id, error_code = NULL, updated_at = CURRENT_TIMESTAMP")
            .bind(meeting_id).bind(record_type.as_str()).bind(PROMPT_VERSION).bind(revision).bind(request_id).execute(&mut *tx).await.map_err(|_| VerifiableRecordError::Storage)?;
        tx.commit()
            .await
            .map_err(|_| VerifiableRecordError::Storage)
    }

    pub async fn active_request(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<(String, i64), VerifiableRecordError> {
        sqlx::query_as("SELECT active_request_id, active_revision FROM record_generations WHERE meeting_id = ? AND status = 'processing'")
            .bind(meeting_id).fetch_optional(pool).await.map_err(|_| VerifiableRecordError::Storage)?.ok_or(VerifiableRecordError::Cancelled)
    }

    pub async fn recover_interrupted(pool: &SqlitePool) -> Result<u64, VerifiableRecordError> {
        let recovered = sqlx::query(
            "UPDATE record_generations SET status = CASE WHEN result_json IS NULL THEN 'pending' ELSE 'completed' END, active_revision = NULL, active_request_id = NULL, error_code = NULL, updated_at = CURRENT_TIMESTAMP WHERE status = 'processing'",
        )
        .execute(pool)
        .await
        .map_err(|_| VerifiableRecordError::Storage)?;
        Ok(recovered.rows_affected())
    }

    pub async fn accept(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
        revision: i64,
        result: &GeneratedRecord,
        model_version: &str,
    ) -> Result<(), VerifiableRecordError> {
        let json =
            serde_json::to_string(result).map_err(|_| VerifiableRecordError::InvalidOutput)?;
        let updated = sqlx::query("UPDATE record_generations SET status = 'completed', input_revision = ?, result_json = ?, error_code = NULL, model_version = ?, active_revision = NULL, active_request_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE meeting_id = ? AND status = 'processing' AND active_request_id = ? AND active_revision = ? AND EXISTS (SELECT 1 FROM meetings WHERE id = ? AND principal_transcript_revision = ?)")
            .bind(revision).bind(json).bind(model_version).bind(meeting_id).bind(request_id).bind(revision).bind(meeting_id).bind(revision).execute(pool).await.map_err(|_| VerifiableRecordError::Storage)?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else if Self::mark_stale(pool, meeting_id, request_id).await? {
            Err(VerifiableRecordError::StaleRevision)
        } else {
            Err(VerifiableRecordError::Cancelled)
        }
    }

    async fn finish_error(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
        status: &str,
        code: &str,
    ) -> Result<(), VerifiableRecordError> {
        let updated = sqlx::query("UPDATE record_generations SET status = ?, error_code = ?, active_revision = NULL, active_request_id = NULL, updated_at = CURRENT_TIMESTAMP WHERE meeting_id = ? AND status = 'processing' AND active_request_id = ?")
            .bind(status).bind(code).bind(meeting_id).bind(request_id).execute(pool).await.map_err(|_| VerifiableRecordError::Storage)?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else {
            Err(VerifiableRecordError::Cancelled)
        }
    }

    pub async fn finish_unavailable(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
        reason: UnavailableReason,
    ) -> Result<(), VerifiableRecordError> {
        Self::finish_error(pool, meeting_id, request_id, "unavailable", reason.as_str()).await
    }

    pub async fn finish_failed(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
        code: FailureCode,
    ) -> Result<(), VerifiableRecordError> {
        Self::finish_error(pool, meeting_id, request_id, "failed", code.as_str()).await
    }

    pub async fn cancel(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
    ) -> Result<(), VerifiableRecordError> {
        let updated = sqlx::query("UPDATE record_generations SET status = CASE WHEN result_json IS NULL THEN 'pending' ELSE 'completed' END, active_revision = NULL, active_request_id = NULL, error_code = NULL, updated_at = CURRENT_TIMESTAMP WHERE meeting_id = ? AND status = 'processing' AND active_request_id = ?")
            .bind(meeting_id).bind(request_id).execute(pool).await.map_err(|_| VerifiableRecordError::Storage)?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else {
            Err(VerifiableRecordError::Cancelled)
        }
    }

    pub async fn cancel_active(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
    ) -> Result<bool, VerifiableRecordError> {
        let updated = sqlx::query("UPDATE record_generations SET status = CASE WHEN result_json IS NULL THEN 'pending' ELSE 'completed' END, active_revision = NULL, active_request_id = NULL, error_code = NULL, updated_at = CURRENT_TIMESTAMP WHERE meeting_id = ? AND status = 'processing' AND active_request_id = ?")
            .bind(meeting_id).bind(request_id).execute(pool).await.map_err(|_| VerifiableRecordError::Storage)?;
        Ok(updated.rows_affected() == 1)
    }

    async fn mark_stale(
        pool: &SqlitePool,
        meeting_id: &str,
        request_id: &str,
    ) -> Result<bool, VerifiableRecordError> {
        let updated = sqlx::query("UPDATE record_generations SET status = 'stale', active_revision = NULL, active_request_id = NULL, error_code = 'stale_revision', updated_at = CURRENT_TIMESTAMP WHERE meeting_id = ? AND status = 'processing' AND active_request_id = ?")
            .bind(meeting_id).bind(request_id).execute(pool).await.map_err(|_| VerifiableRecordError::Storage)?;
        Ok(updated.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::Duration;
    use tokio::sync::Notify;

    async fn fixture() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at, principal_transcript_revision) VALUES ('m1', 'Session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 2)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, display_name) VALUES ('p1', 'm1', 'Speaker 1')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, participant_id, audio_start_time, audio_end_time) VALUES ('b1', 'm1', 'Current text', '00:00:01', 'p1', 1.0, 2.0)").execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn canonical_snapshot_is_read_from_principal_sqlite_state() {
        let pool = fixture().await;
        let record = VerifiableRecordRepository::get(&pool, "m1").await.unwrap();
        assert_eq!(record.principal_transcript_revision, 2);
        assert_eq!(record.transcript[0].text, "Current text");
        assert_eq!(record.transcript[0].start_ms, 1000);
    }

    #[tokio::test]
    async fn canonical_snapshot_never_tears_across_concurrent_principal_updates() {
        let directory = tempfile::tempdir().unwrap();
        let options = SqliteConnectOptions::new()
            .filename(directory.path().join("snapshot.sqlite"))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let writer = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&writer).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at, principal_transcript_revision) VALUES ('m1', 'Session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 2)").execute(&writer).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, display_name) VALUES ('p1', 'm1', 'Old speaker')").execute(&writer).await.unwrap();
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, participant_id, audio_start_time, audio_end_time) VALUES ('b1', 'm1', 'Old text', '00:00:01', 'p1', 1.0, 2.0)").execute(&writer).await.unwrap();
        let old_generation = serde_json::to_string(&GeneratedRecord::Meeting {
            summary: "Old result".into(),
            decisions: vec![],
            action_items: vec![],
            key_points: vec![],
        })
        .unwrap();
        sqlx::query("INSERT INTO record_generations (meeting_id, record_type, status, input_revision, result_json, prompt_version) VALUES ('m1', 'meeting', 'completed', 2, ?, 'fixture')")
            .bind(old_generation)
            .execute(&writer)
            .await
            .unwrap();

        let first_query_released = Arc::new(Notify::new());
        let resume_reader = Arc::new(Notify::new());
        let pause_once = Arc::new(AtomicBool::new(true));
        let reader = SqlitePoolOptions::new()
            .max_connections(1)
            .after_release({
                let first_query_released = first_query_released.clone();
                let resume_reader = resume_reader.clone();
                move |_connection, _metadata| {
                    let first_query_released = first_query_released.clone();
                    let resume_reader = resume_reader.clone();
                    let should_pause = pause_once.swap(false, Ordering::SeqCst);
                    Box::pin(async move {
                        if should_pause {
                            first_query_released.notify_one();
                            resume_reader.notified().await;
                        }
                        Ok(true)
                    })
                }
            })
            .connect_with(options)
            .await
            .unwrap();

        let mut read = tokio::spawn(async move {
            VerifiableRecordRepository::get(&reader, "m1")
                .await
                .unwrap()
        });
        let record = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                biased;
                record = &mut read => record.unwrap(),
                _ = first_query_released.notified() => {
                    let new_generation = serde_json::to_string(&GeneratedRecord::Meeting {
                        summary: "New result".into(),
                        decisions: vec![],
                        action_items: vec![],
                        key_points: vec![],
                    }).unwrap();
                    let mut update = writer.begin().await.unwrap();
                    sqlx::query("UPDATE meetings SET principal_transcript_revision = 3 WHERE id = 'm1'").execute(&mut *update).await.unwrap();
                    sqlx::query("UPDATE participants SET display_name = 'New speaker' WHERE id = 'p1'").execute(&mut *update).await.unwrap();
                    sqlx::query("UPDATE transcripts SET transcript = 'New text' WHERE id = 'b1'").execute(&mut *update).await.unwrap();
                    sqlx::query("UPDATE record_generations SET input_revision = 3, result_json = ? WHERE meeting_id = 'm1'")
                        .bind(new_generation)
                        .execute(&mut *update)
                        .await
                        .unwrap();
                    update.commit().await.unwrap();
                    resume_reader.notify_one();
                    read.await.unwrap()
                }
            }
        })
        .await
        .expect("snapshot read must not hang");
        resume_reader.notify_one();

        let observed = (
            record.principal_transcript_revision,
            record.participants[0].display_name.as_str(),
            record.transcript[0].text.as_str(),
            record.generation_status,
            record.generated.as_ref().map(GeneratedRecord::summary),
        );
        assert!(
            observed
                == (
                    2,
                    "Old speaker",
                    "Old text",
                    GenerationStatus::Completed,
                    Some("Old result"),
                )
                || observed
                    == (
                        3,
                        "New speaker",
                        "New text",
                        GenerationStatus::Completed,
                        Some("New result"),
                    ),
            "torn canonical snapshot: {observed:?}"
        );
    }

    #[tokio::test]
    async fn begin_rejects_stale_and_concurrent_generation() {
        let pool = fixture().await;
        assert_eq!(
            VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 1).await,
            Err(VerifiableRecordError::StaleRevision)
        );
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        assert_eq!(
            VerifiableRecordRepository::set_record_type(&pool, "m1", RecordType::Content).await,
            Err(VerifiableRecordError::AlreadyProcessing)
        );
        assert_eq!(
            VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2).await,
            Err(VerifiableRecordError::AlreadyProcessing)
        );
    }

    #[tokio::test]
    async fn acceptance_is_revision_cas_and_stale_output_is_not_current() {
        let pool = fixture().await;
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        let (request_id, revision) = VerifiableRecordRepository::active_request(&pool, "m1")
            .await
            .unwrap();
        sqlx::query("UPDATE meetings SET principal_transcript_revision = 3 WHERE id = 'm1'")
            .execute(&pool)
            .await
            .unwrap();
        let result = GeneratedRecord::Meeting {
            summary: "Grounded".into(),
            decisions: vec![],
            action_items: vec![],
            key_points: vec![],
        };
        assert_eq!(
            VerifiableRecordRepository::accept(
                &pool,
                "m1",
                &request_id,
                revision,
                &result,
                "fixture"
            )
            .await,
            Err(VerifiableRecordError::StaleRevision)
        );
        let record = VerifiableRecordRepository::get(&pool, "m1").await.unwrap();
        assert_eq!(record.generation_status, GenerationStatus::Stale);
        assert!(record.generated.is_none());
    }

    #[tokio::test]
    async fn cancellation_preserves_prior_accepted_result() {
        let pool = fixture().await;
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        let (request_id, revision) = VerifiableRecordRepository::active_request(&pool, "m1")
            .await
            .unwrap();
        let result = GeneratedRecord::Meeting {
            summary: "Accepted".into(),
            decisions: vec![],
            action_items: vec![],
            key_points: vec![],
        };
        VerifiableRecordRepository::accept(&pool, "m1", &request_id, revision, &result, "fixture")
            .await
            .unwrap();
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        let (retry_id, _) = VerifiableRecordRepository::active_request(&pool, "m1")
            .await
            .unwrap();
        VerifiableRecordRepository::cancel(&pool, "m1", &retry_id)
            .await
            .unwrap();
        assert_eq!(
            VerifiableRecordRepository::get(&pool, "m1")
                .await
                .unwrap()
                .generated
                .unwrap()
                .summary(),
            "Accepted"
        );
    }

    #[tokio::test]
    async fn restart_recovers_processing_to_prior_output_or_pending() {
        let pool = fixture().await;
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        assert_eq!(
            VerifiableRecordRepository::recover_interrupted(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            VerifiableRecordRepository::get(&pool, "m1")
                .await
                .unwrap()
                .generation_status,
            GenerationStatus::Pending
        );

        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        let (request_id, revision) = VerifiableRecordRepository::active_request(&pool, "m1")
            .await
            .unwrap();
        let result = GeneratedRecord::Meeting {
            summary: "Prior accepted output".into(),
            decisions: vec![],
            action_items: vec![],
            key_points: vec![],
        };
        VerifiableRecordRepository::accept(&pool, "m1", &request_id, revision, &result, "fixture")
            .await
            .unwrap();
        VerifiableRecordRepository::begin(&pool, "m1", RecordType::Meeting, 2)
            .await
            .unwrap();
        assert_eq!(
            VerifiableRecordRepository::recover_interrupted(&pool)
                .await
                .unwrap(),
            1
        );
        let recovered = VerifiableRecordRepository::get(&pool, "m1").await.unwrap();
        assert_eq!(recovered.generation_status, GenerationStatus::Completed);
        assert_eq!(
            recovered.generated.unwrap().summary(),
            "Prior accepted output"
        );
    }
}
