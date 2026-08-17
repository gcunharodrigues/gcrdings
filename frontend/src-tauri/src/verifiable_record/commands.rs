use super::apple_foundation::AppleFoundation;
use super::models::{
    validate_completed, FailureCode, GenerationRequest, GenerationResponse, RecordType,
    UnavailableReason, VerifiableRecord, VerifiableRecordError, VERIFIABLE_RECORD_SCHEMA_VERSION,
};
use super::repository::VerifiableRecordRepository;
use crate::state::AppState;
use dashmap::mapref::entry::Entry;
use dashmap::DashMap;
use sqlx::SqlitePool;
use std::sync::{Arc, LazyLock, OnceLock};
use tauri::{AppHandle, Runtime};
use tokio_util::sync::CancellationToken;

struct ActiveGeneration {
    cancellation: CancellationToken,
    request_id: OnceLock<String>,
}

static ACTIVE_GENERATIONS: LazyLock<DashMap<String, Arc<ActiveGeneration>>> =
    LazyLock::new(DashMap::new);

struct GenerationLease {
    meeting_id: String,
    active: Arc<ActiveGeneration>,
}

impl GenerationLease {
    fn bind_request(&self, request_id: String) -> Result<(), VerifiableRecordError> {
        self.active
            .request_id
            .set(request_id)
            .map_err(|_| VerifiableRecordError::Storage)
    }
}

impl Drop for GenerationLease {
    fn drop(&mut self) {
        ACTIVE_GENERATIONS.remove(&self.meeting_id);
    }
}

fn reserve_generation(meeting_id: &str) -> Result<GenerationLease, VerifiableRecordError> {
    let active = Arc::new(ActiveGeneration {
        cancellation: CancellationToken::new(),
        request_id: OnceLock::new(),
    });
    match ACTIVE_GENERATIONS.entry(meeting_id.to_owned()) {
        Entry::Occupied(_) => Err(VerifiableRecordError::AlreadyProcessing),
        Entry::Vacant(entry) => {
            entry.insert(active.clone());
            Ok(GenerationLease {
                meeting_id: meeting_id.to_owned(),
                active,
            })
        }
    }
}

async fn cancel_generation_lease(
    pool: &SqlitePool,
    meeting_id: &str,
    active: Arc<ActiveGeneration>,
) -> Result<(), VerifiableRecordError> {
    active.cancellation.cancel();
    let request_id = active
        .request_id
        .get()
        .ok_or(VerifiableRecordError::Cancelled)?;
    if VerifiableRecordRepository::cancel_active(pool, meeting_id, request_id).await? {
        Ok(())
    } else {
        Err(VerifiableRecordError::Cancelled)
    }
}

async fn cancel_generation(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<(), VerifiableRecordError> {
    let active = ACTIVE_GENERATIONS
        .get(meeting_id)
        .map(|entry| entry.value().clone())
        .ok_or(VerifiableRecordError::Cancelled)?;
    cancel_generation_lease(pool, meeting_id, active).await
}

#[tauri::command]
pub async fn api_get_verifiable_record<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<VerifiableRecord, VerifiableRecordError> {
    VerifiableRecordRepository::get(state.db_manager.pool(), &meeting_id).await
}

#[tauri::command]
pub async fn api_set_record_type<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    record_type: RecordType,
    state: tauri::State<'_, AppState>,
) -> Result<VerifiableRecord, VerifiableRecordError> {
    VerifiableRecordRepository::set_record_type(state.db_manager.pool(), &meeting_id, record_type)
        .await?;
    VerifiableRecordRepository::get(state.db_manager.pool(), &meeting_id).await
}

#[tauri::command]
pub async fn api_generate_verifiable_record<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    expected_revision: i64,
    state: tauri::State<'_, AppState>,
) -> Result<VerifiableRecord, VerifiableRecordError> {
    let pool = state.db_manager.pool();
    let lease = reserve_generation(&meeting_id)?;
    let snapshot = VerifiableRecordRepository::get(pool, &meeting_id).await?;
    VerifiableRecordRepository::begin(pool, &meeting_id, snapshot.record_type, expected_revision)
        .await?;
    let (request_id, revision) =
        VerifiableRecordRepository::active_request(pool, &meeting_id).await?;
    lease.bind_request(request_id.clone())?;
    let request = GenerationRequest {
        schema_version: VERIFIABLE_RECORD_SCHEMA_VERSION,
        request_id: request_id.clone(),
        record_type: snapshot.record_type,
        principal_transcript_revision: revision,
        participants: snapshot.participants,
        transcript: snapshot.transcript,
        prompt_version: format!("{}-v1", snapshot.record_type.as_str()),
    };
    let response = match AppleFoundation::packaged() {
        Ok(helper) => {
            helper
                .run(&request, lease.active.cancellation.clone())
                .await
        }
        Err(_) if lease.active.cancellation.is_cancelled() => Err(VerifiableRecordError::Cancelled),
        Err(VerifiableRecordError::Unavailable) => Ok(GenerationResponse::Unavailable {
            schema_version: VERIFIABLE_RECORD_SCHEMA_VERSION,
            request_id: request_id.clone(),
            reason: UnavailableReason::HelperUnavailable,
        }),
        Err(error) => Err(error),
    };
    let response = if lease.active.cancellation.is_cancelled() {
        Err(VerifiableRecordError::Cancelled)
    } else {
        response
    };
    match response {
        Ok(response @ GenerationResponse::Completed { .. }) => {
            let result = match validate_completed(&request, &response) {
                Ok(result) => result,
                Err(error) => {
                    VerifiableRecordRepository::finish_failed(
                        pool,
                        &meeting_id,
                        &request_id,
                        FailureCode::InvalidOutput,
                    )
                    .await?;
                    return Err(error);
                }
            };
            VerifiableRecordRepository::accept(
                pool,
                &meeting_id,
                &request_id,
                revision,
                &result,
                "apple-foundation-models-v1",
            )
            .await?;
        }
        Ok(GenerationResponse::Unavailable { reason, .. }) => {
            VerifiableRecordRepository::finish_unavailable(pool, &meeting_id, &request_id, reason)
                .await?;
        }
        Ok(GenerationResponse::Failed { code, .. }) => {
            VerifiableRecordRepository::finish_failed(pool, &meeting_id, &request_id, code).await?;
        }
        Err(VerifiableRecordError::Cancelled) => {
            VerifiableRecordRepository::cancel(pool, &meeting_id, &request_id).await?;
            return Err(VerifiableRecordError::Cancelled);
        }
        Err(error) => {
            let code = if error == VerifiableRecordError::Timeout {
                FailureCode::Timeout
            } else {
                FailureCode::HelperFailed
            };
            VerifiableRecordRepository::finish_failed(pool, &meeting_id, &request_id, code).await?;
            return Err(error);
        }
    }
    VerifiableRecordRepository::get(pool, &meeting_id).await
}

#[tauri::command]
pub async fn api_cancel_verifiable_record(
    meeting_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), VerifiableRecordError> {
    cancel_generation(state.db_manager.pool(), &meeting_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verifiable_record::models::{GeneratedRecord, GenerationStatus};
    use sqlx::sqlite::SqlitePoolOptions;
    use tokio::sync::oneshot;

    #[derive(Clone, Copy)]
    enum TerminalOutcome {
        Completed,
        Failed,
        Unavailable,
    }

    async fn fixture(meeting_id: &str) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at, principal_transcript_revision) VALUES (?, 'Session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1)")
            .bind(meeting_id)
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn generated(summary: &str) -> GeneratedRecord {
        GeneratedRecord::Meeting {
            summary: summary.into(),
            decisions: vec![],
            action_items: vec![],
            key_points: vec![],
        }
    }

    async fn begin_retry_with_prior_result(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> (GenerationLease, String, i64) {
        VerifiableRecordRepository::begin(pool, meeting_id, RecordType::Meeting, 1)
            .await
            .unwrap();
        let (request_id, revision) = VerifiableRecordRepository::active_request(pool, meeting_id)
            .await
            .unwrap();
        VerifiableRecordRepository::accept(
            pool,
            meeting_id,
            &request_id,
            revision,
            &generated("Prior accepted output"),
            "fixture",
        )
        .await
        .unwrap();

        let lease = reserve_generation(meeting_id).unwrap();
        VerifiableRecordRepository::begin(pool, meeting_id, RecordType::Meeting, 1)
            .await
            .unwrap();
        let (request_id, revision) = VerifiableRecordRepository::active_request(pool, meeting_id)
            .await
            .unwrap();
        lease.bind_request(request_id.clone()).unwrap();
        (lease, request_id, revision)
    }

    async fn assert_cancel_wins_before_terminal(terminal: TerminalOutcome) {
        let meeting_id = format!("terminal-race-{}", uuid::Uuid::new_v4());
        let pool = fixture(&meeting_id).await;
        let (_lease, request_id, revision) =
            begin_retry_with_prior_result(&pool, &meeting_id).await;
        let (ready_tx, ready_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let terminal_pool = pool.clone();
        let terminal_meeting_id = meeting_id.clone();
        let terminal_request_id = request_id.clone();
        let terminal_task = tokio::spawn(async move {
            ready_tx.send(()).unwrap();
            release_rx.await.unwrap();
            match terminal {
                TerminalOutcome::Completed => {
                    VerifiableRecordRepository::accept(
                        &terminal_pool,
                        &terminal_meeting_id,
                        &terminal_request_id,
                        revision,
                        &generated("Late output"),
                        "fixture",
                    )
                    .await
                }
                TerminalOutcome::Failed => {
                    VerifiableRecordRepository::finish_failed(
                        &terminal_pool,
                        &terminal_meeting_id,
                        &terminal_request_id,
                        FailureCode::HelperFailed,
                    )
                    .await
                }
                TerminalOutcome::Unavailable => {
                    VerifiableRecordRepository::finish_unavailable(
                        &terminal_pool,
                        &terminal_meeting_id,
                        &terminal_request_id,
                        UnavailableReason::HelperUnavailable,
                    )
                    .await
                }
            }
        });

        ready_rx.await.unwrap();
        assert_eq!(cancel_generation(&pool, &meeting_id).await, Ok(()));
        release_tx.send(()).unwrap();
        assert_eq!(
            terminal_task.await.unwrap(),
            Err(VerifiableRecordError::Cancelled)
        );
        let record = VerifiableRecordRepository::get(&pool, &meeting_id)
            .await
            .unwrap();
        assert_eq!(record.generation_status, GenerationStatus::Completed);
        assert_eq!(record.generated.unwrap().summary(), "Prior accepted output");
    }

    #[tokio::test]
    async fn cancellation_reserved_before_durable_begin_is_not_lost() {
        let meeting_id = format!("race-{}", uuid::Uuid::new_v4());
        let pool = fixture(&meeting_id).await;

        let lease = reserve_generation(&meeting_id).unwrap();
        assert_eq!(
            cancel_generation(&pool, &meeting_id).await,
            Err(VerifiableRecordError::Cancelled)
        );
        assert!(lease.active.cancellation.is_cancelled());

        VerifiableRecordRepository::begin(&pool, &meeting_id, RecordType::Meeting, 1)
            .await
            .unwrap();
        let (request_id, _) = VerifiableRecordRepository::active_request(&pool, &meeting_id)
            .await
            .unwrap();
        VerifiableRecordRepository::cancel(&pool, &meeting_id, &request_id)
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM record_generations WHERE meeting_id = ?")
                .bind(&meeting_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending");
    }

    #[tokio::test]
    async fn cancel_wins_immediately_before_completed_persistence() {
        assert_cancel_wins_before_terminal(TerminalOutcome::Completed).await;
    }

    #[tokio::test]
    async fn cancel_wins_immediately_before_failed_persistence() {
        assert_cancel_wins_before_terminal(TerminalOutcome::Failed).await;
    }

    #[tokio::test]
    async fn cancel_wins_immediately_before_unavailable_persistence() {
        assert_cancel_wins_before_terminal(TerminalOutcome::Unavailable).await;
    }

    #[tokio::test]
    async fn completed_terminal_wins_before_cancel_reports_no_active_generation() {
        let meeting_id = format!("terminal-first-{}", uuid::Uuid::new_v4());
        let pool = fixture(&meeting_id).await;
        let (_lease, request_id, revision) =
            begin_retry_with_prior_result(&pool, &meeting_id).await;
        VerifiableRecordRepository::accept(
            &pool,
            &meeting_id,
            &request_id,
            revision,
            &generated("New terminal output"),
            "fixture",
        )
        .await
        .unwrap();

        assert_eq!(
            cancel_generation(&pool, &meeting_id).await,
            Err(VerifiableRecordError::Cancelled)
        );
        assert_eq!(
            VerifiableRecordRepository::get(&pool, &meeting_id)
                .await
                .unwrap()
                .generated
                .unwrap()
                .summary(),
            "New terminal output"
        );
    }

    #[tokio::test]
    async fn delayed_cancel_for_terminal_generation_does_not_cancel_its_successor() {
        let meeting_id = format!("aba-cancel-{}", uuid::Uuid::new_v4());
        let pool = fixture(&meeting_id).await;
        let lease_a = reserve_generation(&meeting_id).unwrap();
        VerifiableRecordRepository::begin(&pool, &meeting_id, RecordType::Meeting, 1)
            .await
            .unwrap();
        let (request_a, revision) = VerifiableRecordRepository::active_request(&pool, &meeting_id)
            .await
            .unwrap();
        lease_a.bind_request(request_a.clone()).unwrap();
        let delayed_a = ACTIVE_GENERATIONS.get(&meeting_id).unwrap().value().clone();

        VerifiableRecordRepository::accept(
            &pool,
            &meeting_id,
            &request_a,
            revision,
            &generated("Generation A terminal output"),
            "fixture",
        )
        .await
        .unwrap();
        drop(lease_a);

        let lease_b = reserve_generation(&meeting_id).unwrap();
        VerifiableRecordRepository::begin(&pool, &meeting_id, RecordType::Meeting, 1)
            .await
            .unwrap();
        let (request_b, _) = VerifiableRecordRepository::active_request(&pool, &meeting_id)
            .await
            .unwrap();
        lease_b.bind_request(request_b.clone()).unwrap();

        let delayed_a_result = cancel_generation_lease(&pool, &meeting_id, delayed_a).await;
        let active_after_cancel =
            VerifiableRecordRepository::active_request(&pool, &meeting_id).await;

        assert_eq!(
            (delayed_a_result, active_after_cancel),
            (
                Err(VerifiableRecordError::Cancelled),
                Ok((request_b, revision))
            )
        );
    }
}
