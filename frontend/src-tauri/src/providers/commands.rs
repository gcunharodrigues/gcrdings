use super::{
    CredentialStore, ProviderAdapter, ProviderError, ProviderPreview, ProviderTransferOutcome,
};
use super::{ProviderAuthorizationStatus, ProviderConfiguration, AGENT_HANDOFF_TASK};
use crate::providers::keychain::MacKeychain;
use crate::providers::repository::ProviderRepository;
use crate::state::AppState;
use crate::summary::llm_client::ProviderHttpAdapter;
use crate::verifiable_record::repository::VerifiableRecordRepository;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tauri::State;

static PROVIDER_AUTHORIZATION_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn save_provider_credential(
    pool: &SqlitePool,
    credentials: &dyn CredentialStore,
    configuration: ProviderConfiguration,
    credential: &str,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    ProviderRepository::require_legacy_credentials_migrated(pool, credentials).await?;
    let staged = ProviderRepository::stage_replacement(pool, &configuration).await?;
    if let Err(error) = credentials.save(&staged.credential_account, credential) {
        let _ = ProviderRepository::rollback_staged_replacement(
            pool,
            &configuration.provider,
            &configuration.task,
            staged.generation,
        )
        .await;
        return Err(error);
    }
    ProviderRepository::mark_generation_ready(
        pool,
        &configuration.provider,
        &configuration.task,
        staged.generation,
    )
    .await?;
    ProviderRepository::cleanup_superseded_credential(
        pool,
        credentials,
        &configuration.provider,
        &configuration.task,
    )
    .await?;
    ProviderRepository::status(
        pool,
        credentials,
        &configuration.provider,
        &configuration.task,
    )
    .await
}

#[tauri::command]
pub async fn api_save_provider_credential(
    state: State<'_, AppState>,
    configuration: ProviderConfiguration,
    credential: String,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let credentials = MacKeychain::default();
    save_provider_credential(
        state.db_manager.pool(),
        &credentials,
        configuration,
        &credential,
    )
    .await
}

#[tauri::command]
pub async fn api_get_provider_status(
    state: State<'_, AppState>,
    provider: String,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    let credentials = MacKeychain::default();
    ProviderRepository::require_legacy_credentials_migrated(state.db_manager.pool(), &credentials)
        .await?;
    ProviderRepository::status(
        state.db_manager.pool(),
        &credentials,
        &provider,
        AGENT_HANDOFF_TASK,
    )
    .await
}

#[tauri::command]
pub async fn api_set_provider_task_enabled(
    state: State<'_, AppState>,
    provider: String,
    enabled: bool,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    let credentials = MacKeychain::default();
    ProviderRepository::require_legacy_credentials_migrated(state.db_manager.pool(), &credentials)
        .await?;
    ProviderRepository::set_enabled(
        state.db_manager.pool(),
        &provider,
        AGENT_HANDOFF_TASK,
        enabled,
    )
    .await?;
    ProviderRepository::status(
        state.db_manager.pool(),
        &credentials,
        &provider,
        AGENT_HANDOFF_TASK,
    )
    .await
}

#[tauri::command]
pub async fn api_remove_provider_credential(
    state: State<'_, AppState>,
    provider: String,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    let credentials = MacKeychain::default();
    ProviderRepository::require_legacy_credentials_migrated(state.db_manager.pool(), &credentials)
        .await?;
    remove_provider_credential(state.db_manager.pool(), &credentials, &provider).await?;
    ProviderRepository::status(
        state.db_manager.pool(),
        &credentials,
        &provider,
        AGENT_HANDOFF_TASK,
    )
    .await
}

async fn remove_provider_credential(
    pool: &SqlitePool,
    credentials: &dyn CredentialStore,
    provider: &str,
) -> Result<(), ProviderError> {
    ProviderRepository::remove_all_credentials(pool, credentials, provider, AGENT_HANDOFF_TASK)
        .await
}

#[tauri::command]
pub async fn api_test_provider_credential(
    state: State<'_, AppState>,
    provider: String,
) -> Result<ProviderAuthorizationStatus, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    let credentials = MacKeychain::default();
    ProviderRepository::require_legacy_credentials_migrated(state.db_manager.pool(), &credentials)
        .await?;
    let status = ProviderRepository::status(
        state.db_manager.pool(),
        &credentials,
        &provider,
        AGENT_HANDOFF_TASK,
    )
    .await?;
    let (account, _) = ProviderRepository::ready_credential_account(
        state.db_manager.pool(),
        &provider,
        AGENT_HANDOFF_TASK,
    )
    .await?;
    let credential = credentials
        .read(&account)?
        .ok_or(ProviderError::CredentialUnavailable)?;
    let (configuration, _) =
        ProviderRepository::configuration(state.db_manager.pool(), &provider, AGENT_HANDOFF_TASK)
            .await?;
    ProviderHttpAdapter::default()
        .test(&configuration, &credential)
        .await?;
    Ok(status)
}

pub async fn preview_transfer(
    pool: &SqlitePool,
    meeting_id: &str,
    provider: &str,
    purpose: &str,
) -> Result<ProviderPreview, ProviderError> {
    if purpose.trim().is_empty() || purpose.len() > 200 || purpose.chars().any(char::is_control) {
        return Err(ProviderError::InvalidConfiguration);
    }
    let (configuration, _) =
        ProviderRepository::configuration(pool, provider, AGENT_HANDOFF_TASK).await?;
    let record = VerifiableRecordRepository::get(pool, meeting_id)
        .await
        .map_err(|_| ProviderError::Storage)?;
    let payload = crate::agent_handoff::serialize_json(&record)
        .map_err(|_| ProviderError::InvalidResponse)?;
    let snapshot_digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
    let snapshot_version: i64 =
        sqlx::query_scalar("SELECT canonical_snapshot_version FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_one(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
    let configuration_generation =
        ProviderRepository::configuration_generation(pool, provider, AGENT_HANDOFF_TASK).await?;
    let transfer_id = uuid::Uuid::new_v4().to_string();
    let data_types = transfer_data_types();
    let preview_digest = digest(
        &transfer_id,
        &configuration,
        purpose,
        record.principal_transcript_revision,
        &payload,
    )?;
    ProviderRepository::create_preview(
        pool,
        &transfer_id,
        meeting_id,
        provider,
        AGENT_HANDOFF_TASK,
        record.principal_transcript_revision,
        snapshot_version,
        &snapshot_digest,
        configuration_generation,
        &preview_digest,
        &data_types,
        purpose,
    )
    .await?;
    Ok(ProviderPreview {
        transfer_id,
        preview_digest,
        provider: provider.into(),
        provider_display_name: configuration.display_name,
        session_title: record.title,
        purpose: purpose.into(),
        task: AGENT_HANDOFF_TASK.into(),
        data_types,
        principal_transcript_revision: record.principal_transcript_revision,
    })
}

pub async fn confirm_transfer(
    pool: &SqlitePool,
    credentials: &dyn CredentialStore,
    adapter: &dyn ProviderAdapter,
    preview_digest: &str,
) -> Result<ProviderTransferOutcome, ProviderError> {
    confirm_transfer_internal(pool, credentials, adapter, preview_digest, || async {}).await
}

async fn confirm_transfer_internal<F, Fut>(
    pool: &SqlitePool,
    credentials: &dyn CredentialStore,
    adapter: &dyn ProviderAdapter,
    preview_digest: &str,
    before_reservation: F,
) -> Result<ProviderTransferOutcome, ProviderError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    ProviderRepository::require_legacy_credentials_migrated(pool, credentials).await?;
    let transfer = ProviderRepository::transfer(pool, preview_digest).await?;
    if transfer.status != "confirmed" {
        return Err(ProviderError::AlreadyConsumed);
    }
    let (configuration, enabled) =
        ProviderRepository::configuration(pool, &transfer.provider, &transfer.task).await?;
    if !enabled {
        return Err(ProviderError::TaskDisabled);
    }
    let record = VerifiableRecordRepository::get(pool, &transfer.meeting_id)
        .await
        .map_err(|_| ProviderError::Storage)?;
    if record.principal_transcript_revision != transfer.snapshot_revision {
        return Err(ProviderError::StalePreview);
    }
    let payload = crate::agent_handoff::serialize_json(&record)
        .map_err(|_| ProviderError::InvalidResponse)?;
    let current_digest = digest(
        &transfer.id,
        &configuration,
        &transfer.purpose,
        record.principal_transcript_revision,
        &payload,
    )?;
    if current_digest != preview_digest {
        return Err(ProviderError::StalePreview);
    }
    let initial_snapshot_digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
    if initial_snapshot_digest != transfer.snapshot_digest {
        return Err(ProviderError::StalePreview);
    }
    before_reservation().await;
    let record = VerifiableRecordRepository::get(pool, &transfer.meeting_id)
        .await
        .map_err(|_| ProviderError::Storage)?;
    let payload = crate::agent_handoff::serialize_json(&record)
        .map_err(|_| ProviderError::InvalidResponse)?;
    let snapshot_digest = format!("{:x}", Sha256::digest(payload.as_bytes()));
    let snapshot_version: i64 =
        sqlx::query_scalar("SELECT canonical_snapshot_version FROM meetings WHERE id = ?")
            .bind(&transfer.meeting_id)
            .fetch_one(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
    if snapshot_digest != transfer.snapshot_digest || snapshot_version != transfer.snapshot_version
    {
        return Err(ProviderError::StalePreview);
    }
    let (account, generation) =
        ProviderRepository::ready_credential_account(pool, &transfer.provider, &transfer.task)
            .await?;
    if generation != transfer.configuration_generation {
        return Err(ProviderError::StalePreview);
    }
    let credential = credentials
        .read(&account)?
        .ok_or(ProviderError::CredentialUnavailable)?;
    ProviderRepository::begin_send(pool, &transfer).await?;
    let result = match adapter
        .send(&configuration, &credential, &transfer.id, &payload)
        .await
    {
        Ok(result) => result,
        Err(ProviderError::OutcomeUnknown) => {
            ProviderRepository::mark_outcome_unknown(pool, &transfer.id).await?;
            return Err(ProviderError::OutcomeUnknown);
        }
        Err(error) => {
            ProviderRepository::fail(pool, &transfer.id, error_code(&error)).await?;
            return Err(error);
        }
    };
    let serialized_result =
        serde_json::to_string(&result).map_err(|_| ProviderError::InvalidResponse)?;
    if std::str::from_utf8(credential.expose())
        .is_ok_and(|secret| serialized_result.contains(secret))
    {
        ProviderRepository::mark_outcome_unknown(pool, &transfer.id).await?;
        return Err(ProviderError::OutcomeUnknown);
    }
    if ProviderRepository::complete(pool, &transfer.id, &result)
        .await
        .is_err()
    {
        let _ = ProviderRepository::mark_outcome_unknown(pool, &transfer.id).await;
        return Err(ProviderError::OutcomeUnknown);
    }
    Ok(ProviderTransferOutcome {
        transfer_id: transfer.id,
        provider: transfer.provider,
        task: transfer.task,
        status: "completed".into(),
        result: Some(result),
        error_code: None,
    })
}

#[cfg(test)]
async fn confirm_transfer_with_before_reservation<F, Fut>(
    pool: &SqlitePool,
    credentials: &dyn CredentialStore,
    adapter: &dyn ProviderAdapter,
    preview_digest: &str,
    before_reservation: F,
) -> Result<ProviderTransferOutcome, ProviderError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    confirm_transfer_internal(
        pool,
        credentials,
        adapter,
        preview_digest,
        before_reservation,
    )
    .await
}

#[tauri::command]
pub async fn api_preview_provider_transfer(
    state: State<'_, AppState>,
    meeting_id: String,
    provider: String,
    purpose: String,
) -> Result<ProviderPreview, ProviderError> {
    let _guard = PROVIDER_AUTHORIZATION_GATE.lock().await;
    let credentials = MacKeychain::default();
    ProviderRepository::require_legacy_credentials_migrated(state.db_manager.pool(), &credentials)
        .await?;
    preview_transfer(state.db_manager.pool(), &meeting_id, &provider, &purpose).await
}

#[tauri::command]
pub async fn api_confirm_provider_transfer(
    state: State<'_, AppState>,
    preview_digest: String,
) -> Result<ProviderTransferOutcome, ProviderError> {
    confirm_transfer(
        state.db_manager.pool(),
        &MacKeychain::default(),
        &ProviderHttpAdapter::default(),
        &preview_digest,
    )
    .await
}

fn transfer_data_types() -> Vec<String> {
    [
        "session_metadata",
        "participants",
        "corrected_transcript",
        "local_generation",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn digest(
    transfer_id: &str,
    configuration: &ProviderConfiguration,
    purpose: &str,
    revision: i64,
    payload: &str,
) -> Result<String, ProviderError> {
    let facts = serde_json::to_vec(&serde_json::json!({
        "transfer_id": transfer_id,
        "provider": configuration.provider,
        "task": configuration.task,
        "endpoint": configuration.endpoint,
        "model": configuration.model,
        "purpose": purpose,
        "revision": revision,
        "payload": payload,
    }))
    .map_err(|_| ProviderError::InvalidResponse)?;
    Ok(format!("{:x}", Sha256::digest(facts)))
}

fn error_code(error: &ProviderError) -> &'static str {
    match error {
        ProviderError::RequestFailed => "request_failed",
        ProviderError::InvalidResponse => "invalid_response",
        ProviderError::CredentialUnavailable => "credential_unavailable",
        ProviderError::OutcomeUnknown => "outcome_unknown",
        _ => "transfer_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{ProviderConfiguration, SecretString, AGENT_HANDOFF_TASK};
    use async_trait::async_trait;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct TestCredentials;
    impl CredentialStore for TestCredentials {
        fn save(&self, _: &str, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }
        fn read(&self, _: &str) -> Result<Option<SecretString>, ProviderError> {
            Ok(Some(SecretString::new("synthetic-provider-secret")))
        }
        fn remove(&self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    struct FailingSaveCredentials(Mutex<Vec<u8>>);

    impl CredentialStore for FailingSaveCredentials {
        fn save(&self, _: &str, _: &str) -> Result<(), ProviderError> {
            Err(ProviderError::Storage)
        }

        fn read(&self, _: &str) -> Result<Option<SecretString>, ProviderError> {
            Ok(Some(SecretString::new(self.0.lock().unwrap().as_slice())))
        }

        fn remove(&self, _: &str) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct AccountCredentials(Mutex<HashMap<String, Vec<u8>>>);

    impl CredentialStore for AccountCredentials {
        fn save(&self, account: &str, secret: &str) -> Result<(), ProviderError> {
            self.0
                .lock()
                .unwrap()
                .insert(account.into(), secret.as_bytes().to_vec());
            Ok(())
        }

        fn read(&self, account: &str) -> Result<Option<SecretString>, ProviderError> {
            Ok(self.0.lock().unwrap().get(account).map(SecretString::new))
        }

        fn remove(&self, account: &str) -> Result<(), ProviderError> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    struct CapturingAdapter {
        calls: AtomicUsize,
        payloads: Mutex<Vec<String>>,
        fail: bool,
    }

    #[async_trait]
    impl ProviderAdapter for CapturingAdapter {
        async fn test(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
        ) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn send(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
            transfer_id: &str,
            payload: &str,
        ) -> Result<serde_json::Value, ProviderError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(
                !transfer_id.is_empty(),
                "every POST has a durable idempotency key"
            );
            self.payloads.lock().unwrap().push(payload.into());
            if self.fail {
                Err(ProviderError::RequestFailed)
            } else {
                Ok(serde_json::json!({"accepted": true}))
            }
        }
    }

    struct OutcomeUnknownAdapter {
        calls: AtomicUsize,
    }

    struct SecretReflectingAdapter;

    #[async_trait]
    impl ProviderAdapter for SecretReflectingAdapter {
        async fn test(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
        ) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn send(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
            _: &str,
            _: &str,
        ) -> Result<serde_json::Value, ProviderError> {
            Ok(serde_json::json!({"echo": "synthetic-provider-secret"}))
        }
    }

    #[async_trait]
    impl ProviderAdapter for OutcomeUnknownAdapter {
        async fn test(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
        ) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn send(
            &self,
            _: &ProviderConfiguration,
            _: &SecretString,
            transfer_id: &str,
            _: &str,
        ) -> Result<serde_json::Value, ProviderError> {
            assert!(!transfer_id.is_empty());
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(ProviderError::OutcomeUnknown)
        }
    }

    async fn fixture(enabled: bool) -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at, principal_transcript_revision) VALUES ('m1', 'Private Session', 'now', 'now', 4)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, display_name) VALUES ('p1', 'm1', 'Local participant')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, participant_id, audio_start_time, audio_end_time) VALUES ('b1', 'm1', 'corrected transcript', 'now', 'p1', 0.0, 1.0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO record_generations (meeting_id, record_type, status, prompt_version) VALUES ('m1', 'meeting', 'pending', 'fixture')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO provider_authorizations (provider, task, display_name, endpoint, model, enabled, generation, ready_generation, credential_account) VALUES ('fixture', ?, 'Fixture Provider', 'http://127.0.0.1:9', 'fixture', ?, 1, 1, 'transfer:fixture:test')")
            .bind(AGENT_HANDOFF_TASK).bind(enabled).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn disabled_task_and_unconfirmed_preview_send_zero_requests() {
        let pool = fixture(false).await;
        let adapter = CapturingAdapter {
            calls: AtomicUsize::new(0),
            payloads: Mutex::new(vec![]),
            fail: false,
        };
        let preview = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        assert_eq!(preview.provider_display_name, "Fixture Provider");
        assert_eq!(preview.session_title, "Private Session");
        assert_eq!(preview.principal_transcript_revision, 4);
        assert_eq!(
            preview.data_types,
            [
                "session_metadata",
                "participants",
                "corrected_transcript",
                "local_generation"
            ]
        );
        assert_eq!(
            adapter.calls.load(Ordering::SeqCst),
            0,
            "preview and cancellation never invoke the adapter"
        );
        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &preview.preview_digest).await,
            Err(ProviderError::TaskDisabled)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn confirmation_sends_exact_current_payload_once_without_media_and_records_separate_provenance(
    ) {
        let pool = fixture(true).await;
        let adapter = CapturingAdapter {
            calls: AtomicUsize::new(0),
            payloads: Mutex::new(vec![]),
            fail: false,
        };
        let preview = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        let outcome = confirm_transfer(&pool, &TestCredentials, &adapter, &preview.preview_digest)
            .await
            .unwrap();
        assert_eq!(outcome.provider, "fixture");
        assert_eq!(outcome.task, AGENT_HANDOFF_TASK);
        assert_eq!(outcome.status, "completed");
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        let payload = adapter.payloads.lock().unwrap()[0].clone();
        assert!(payload.contains("corrected transcript"));
        assert!(!payload.contains("video"));
        assert!(!payload.contains("media"));
        assert!(!payload.contains("path"));
        assert!(!payload.contains("synthetic-provider-secret"));
        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &preview.preview_digest).await,
            Err(ProviderError::AlreadyConsumed)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        let local_status: String =
            sqlx::query_scalar("SELECT status FROM record_generations WHERE meeting_id = 'm1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(local_status, "pending");
        let provenance: (String, String, String) =
            sqlx::query_as("SELECT provider, task, status FROM provider_transfers WHERE id = ?")
                .bind(&outcome.transfer_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            provenance,
            (
                "fixture".into(),
                AGENT_HANDOFF_TASK.into(),
                "completed".into()
            )
        );
    }

    #[tokio::test]
    async fn changed_snapshot_and_failure_require_a_new_confirmation_without_fallback() {
        let pool = fixture(true).await;
        let adapter = CapturingAdapter {
            calls: AtomicUsize::new(0),
            payloads: Mutex::new(vec![]),
            fail: true,
        };
        let stale = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        sqlx::query("UPDATE meetings SET principal_transcript_revision = 5 WHERE id = 'm1'")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &stale.preview_digest).await,
            Err(ProviderError::StalePreview)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);

        let current = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &current.preview_digest).await,
            Err(ProviderError::RequestFailed)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &current.preview_digest).await,
            Err(ProviderError::AlreadyConsumed)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        let retry = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        assert_ne!(retry.preview_digest, current.preview_digest);
    }

    #[tokio::test]
    async fn failed_credential_replacement_keeps_old_destination_disabled() {
        let pool = fixture(true).await;
        let credentials = FailingSaveCredentials(Mutex::new(b"old-secret".to_vec()));
        let replacement = ProviderConfiguration {
            provider: "fixture".into(),
            display_name: "Fixture Provider".into(),
            endpoint: "https://new-provider.example/v1".into(),
            model: "replacement".into(),
            task: AGENT_HANDOFF_TASK.into(),
        };

        assert_eq!(
            save_provider_credential(&pool, &credentials, replacement, "new-secret").await,
            Err(ProviderError::Storage)
        );

        let (configuration, enabled) =
            ProviderRepository::configuration(&pool, "fixture", AGENT_HANDOFF_TASK)
                .await
                .unwrap();
        assert!(!enabled);
        assert_eq!(configuration.endpoint, "http://127.0.0.1:9");
        assert_eq!(
            credentials.read("fixture").unwrap().unwrap().expose(),
            b"old-secret"
        );
    }

    #[tokio::test]
    async fn transfer_credential_is_isolated_from_legacy_migration_and_settings_writes() {
        let pool = fixture(false).await;
        let credentials = AccountCredentials::default();
        let configuration = ProviderConfiguration {
            provider: "custom-openai".into(),
            display_name: "Custom OpenAI".into(),
            endpoint: "https://provider.example/v1".into(),
            model: "fixture".into(),
            task: AGENT_HANDOFF_TASK.into(),
        };
        save_provider_credential(&pool, &credentials, configuration, "transfer-secret")
            .await
            .unwrap();

        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, customOpenAIConfig) VALUES ('1', 'custom-openai', 'fixture', 'fixture', ?)")
            .bind(r#"{"apiKey":"legacy-secret"}"#)
            .execute(&pool)
            .await
            .unwrap();
        ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
            .await
            .unwrap();
        credentials
            .save("custom-openai", "legacy-settings-secret")
            .unwrap();

        let (transfer_account, _) = ProviderRepository::ready_credential_account(
            &pool,
            "custom-openai",
            AGENT_HANDOFF_TASK,
        )
        .await
        .unwrap();
        let transfer = credentials
            .read(&transfer_account)
            .unwrap()
            .expect("task-scoped transfer credential");
        assert_eq!(transfer.expose(), b"transfer-secret");
        assert_eq!(
            credentials.read("custom-openai").unwrap().unwrap().expose(),
            b"legacy-settings-secret"
        );
    }

    #[tokio::test]
    async fn crash_after_keychain_write_recovers_only_the_matching_disabled_generation() {
        let pool = fixture(true).await;
        let credentials = AccountCredentials::default();
        credentials
            .save("transfer:agent_handoff:fixture", "old-secret")
            .unwrap();
        let replacement = ProviderConfiguration {
            provider: "fixture".into(),
            display_name: "Fixture Provider".into(),
            endpoint: "https://new-provider.example/v1".into(),
            model: "replacement".into(),
            task: AGENT_HANDOFF_TASK.into(),
        };

        let staged = ProviderRepository::stage_replacement(&pool, &replacement)
            .await
            .unwrap();
        credentials
            .save(&staged.credential_account, "new-secret")
            .unwrap();
        ProviderRepository::recover_staged_credentials(&pool, &credentials)
            .await
            .unwrap();

        let status = ProviderRepository::status(&pool, &credentials, "fixture", AGENT_HANDOFF_TASK)
            .await
            .unwrap();
        assert!(!status.enabled);
        assert!(status.credential_present);
        assert_eq!(status.endpoint, replacement.endpoint);
        assert_eq!(status.model, replacement.model);
        assert_eq!(
            credentials
                .read(&staged.credential_account)
                .unwrap()
                .unwrap()
                .expose(),
            b"new-secret"
        );
    }

    #[tokio::test]
    async fn reservation_rechecks_title_participant_and_generation_after_preview_validation() {
        for mutation in ["title", "participant", "generation"] {
            let pool = fixture(true).await;
            let adapter = CapturingAdapter {
                calls: AtomicUsize::new(0),
                payloads: Mutex::new(vec![]),
                fail: false,
            };
            let preview = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
                .await
                .unwrap();
            let outcome = confirm_transfer_with_before_reservation(
                &pool,
                &TestCredentials,
                &adapter,
                &preview.preview_digest,
                || async {
                    match mutation {
                        "title" => {
                            sqlx::query("UPDATE meetings SET title = 'Changed title' WHERE id = 'm1'")
                                .execute(&pool)
                                .await
                                .unwrap();
                        }
                        "participant" => {
                            sqlx::query("UPDATE participants SET display_name = 'Changed speaker' WHERE id = 'p1'")
                                .execute(&pool)
                                .await
                                .unwrap();
                        }
                        _ => {
                            sqlx::query("UPDATE record_generations SET status = 'failed', error_code = 'timeout' WHERE meeting_id = 'm1'")
                                .execute(&pool)
                                .await
                                .unwrap();
                        }
                    }
                },
            )
            .await;
            assert_eq!(outcome, Err(ProviderError::StalePreview), "{mutation}");
            assert_eq!(adapter.calls.load(Ordering::SeqCst), 0, "{mutation}");
        }
    }

    #[tokio::test]
    async fn accepted_post_with_dropped_response_is_durable_and_cannot_be_retried() {
        let pool = fixture(true).await;
        let adapter = OutcomeUnknownAdapter {
            calls: AtomicUsize::new(0),
        };
        let preview = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();

        assert_eq!(
            confirm_transfer(&pool, &TestCredentials, &adapter, &preview.preview_digest).await,
            Err(ProviderError::OutcomeUnknown)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        let status: String =
            sqlx::query_scalar("SELECT status FROM provider_transfers WHERE id = ?")
                .bind(&preview.transfer_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "outcome_unknown");
        assert_eq!(
            preview_transfer(&pool, "m1", "fixture", "Analyze the Session").await,
            Err(ProviderError::OutcomeUnknown)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn replacement_recovery_and_removal_leave_no_generation_credential() {
        let pool = fixture(false).await;
        let credentials = AccountCredentials::default();
        credentials
            .save("transfer:fixture:test", "old-secret")
            .unwrap();
        let first = ProviderConfiguration {
            provider: "fixture".into(),
            display_name: "Fixture Provider".into(),
            endpoint: "https://provider.example/v2".into(),
            model: "v2".into(),
            task: AGENT_HANDOFF_TASK.into(),
        };
        save_provider_credential(&pool, &credentials, first, "replacement-secret")
            .await
            .unwrap();
        assert!(credentials.read("transfer:fixture:test").unwrap().is_none());

        let second = ProviderConfiguration {
            provider: "fixture".into(),
            display_name: "Fixture Provider".into(),
            endpoint: "https://provider.example/v3".into(),
            model: "v3".into(),
            task: AGENT_HANDOFF_TASK.into(),
        };
        let before_write = ProviderRepository::stage_replacement(&pool, &second)
            .await
            .unwrap();
        ProviderRepository::recover_staged_credentials(&pool, &credentials)
            .await
            .unwrap();
        let (rolled_back, _) =
            ProviderRepository::configuration(&pool, "fixture", AGENT_HANDOFF_TASK)
                .await
                .unwrap();
        assert_eq!(rolled_back.endpoint, "https://provider.example/v2");
        assert!(credentials
            .read(&before_write.credential_account)
            .unwrap()
            .is_none());

        let staged = ProviderRepository::stage_replacement(&pool, &second)
            .await
            .unwrap();
        credentials
            .save(&staged.credential_account, "crash-secret")
            .unwrap();
        ProviderRepository::mark_generation_ready(
            &pool,
            "fixture",
            AGENT_HANDOFF_TASK,
            staged.generation,
        )
        .await
        .unwrap();
        ProviderRepository::recover_staged_credentials(&pool, &credentials)
            .await
            .unwrap();
        remove_provider_credential(&pool, &credentials, "fixture")
            .await
            .unwrap();

        assert!(credentials
            .0
            .lock()
            .unwrap()
            .keys()
            .all(|account| !account.starts_with("transfer:")));
    }

    #[tokio::test]
    async fn startup_and_post_success_validation_make_unresolved_posts_non_retryable() {
        let pool = fixture(true).await;
        let preview = preview_transfer(&pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        sqlx::query("UPDATE provider_transfers SET status = 'sending' WHERE id = ?")
            .bind(&preview.transfer_id)
            .execute(&pool)
            .await
            .unwrap();
        ProviderRepository::recover_unresolved_transfers(&pool)
            .await
            .unwrap();
        assert_eq!(
            preview_transfer(&pool, "m1", "fixture", "Analyze the Session").await,
            Err(ProviderError::OutcomeUnknown)
        );

        let second_pool = fixture(true).await;
        let second = preview_transfer(&second_pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        assert_eq!(
            confirm_transfer(
                &second_pool,
                &TestCredentials,
                &SecretReflectingAdapter,
                &second.preview_digest,
            )
            .await,
            Err(ProviderError::OutcomeUnknown)
        );
        assert_eq!(
            preview_transfer(&second_pool, "m1", "fixture", "Analyze the Session").await,
            Err(ProviderError::OutcomeUnknown)
        );

        let completion_pool = fixture(true).await;
        let completion = preview_transfer(&completion_pool, "m1", "fixture", "Analyze the Session")
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER reject_provider_completion BEFORE UPDATE OF status ON provider_transfers WHEN NEW.status = 'completed' BEGIN SELECT RAISE(ABORT, 'synthetic completion failure'); END")
            .execute(&completion_pool)
            .await
            .unwrap();
        let adapter = CapturingAdapter {
            calls: AtomicUsize::new(0),
            payloads: Mutex::new(vec![]),
            fail: false,
        };
        assert_eq!(
            confirm_transfer(
                &completion_pool,
                &TestCredentials,
                &adapter,
                &completion.preview_digest,
            )
            .await,
            Err(ProviderError::OutcomeUnknown)
        );
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            preview_transfer(&completion_pool, "m1", "fixture", "Analyze the Session").await,
            Err(ProviderError::OutcomeUnknown)
        );
    }
}
