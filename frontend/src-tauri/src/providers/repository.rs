use super::{
    credential_account, credential_generation_account, summary_credential_account, CredentialStore,
    ProviderAuthorizationStatus, ProviderConfiguration, ProviderError,
};
use sqlx::{sqlite::SqliteConnectOptions, Connection, Row};
use std::path::Path;

#[derive(sqlx::FromRow)]
pub(crate) struct TransferRow {
    pub id: String,
    pub meeting_id: String,
    pub provider: String,
    pub task: String,
    pub snapshot_revision: i64,
    pub snapshot_version: i64,
    pub snapshot_digest: String,
    pub configuration_generation: i64,
    pub preview_digest: String,
    pub purpose: String,
    pub status: String,
}

#[derive(sqlx::FromRow)]
struct AuthorizationRow {
    display_name: String,
    endpoint: String,
    model: String,
    enabled: bool,
    generation: i64,
    ready_generation: i64,
    credential_account: String,
    superseded_credential_account: Option<String>,
}

pub(crate) struct StagedCredential {
    pub credential_account: String,
    pub generation: i64,
}
use sqlx::SqlitePool;

pub struct ProviderRepository;

impl ProviderRepository {
    async fn authorization(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
    ) -> Result<AuthorizationRow, ProviderError> {
        sqlx::query_as("SELECT display_name, endpoint, model, enabled, generation, ready_generation, credential_account, superseded_credential_account FROM provider_authorizations WHERE provider = ? AND task = ?")
            .bind(provider)
            .bind(task)
            .fetch_optional(pool)
            .await
            .map_err(|_| ProviderError::Storage)?
            .ok_or(ProviderError::InvalidConfiguration)
    }

    pub async fn configuration(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
    ) -> Result<(ProviderConfiguration, bool), ProviderError> {
        let row = Self::authorization(pool, provider, task).await?;
        Ok((
            ProviderConfiguration {
                provider: provider.into(),
                display_name: row.display_name,
                endpoint: row.endpoint,
                model: row.model,
                task: task.into(),
            },
            row.enabled,
        ))
    }

    pub(crate) async fn configuration_generation(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
    ) -> Result<i64, ProviderError> {
        Ok(Self::authorization(pool, provider, task).await?.generation)
    }

    pub(crate) async fn ready_credential_account(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
    ) -> Result<(String, i64), ProviderError> {
        let row = Self::authorization(pool, provider, task).await?;
        if row.ready_generation != row.generation {
            return Err(ProviderError::CredentialUnavailable);
        }
        let account = if row.credential_account.is_empty() {
            credential_account(provider, task)
        } else {
            row.credential_account
        };
        Ok((account, row.generation))
    }

    // Preview persistence mirrors the canonical authorization snapshot columns.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn create_preview(
        pool: &SqlitePool,
        id: &str,
        meeting_id: &str,
        provider: &str,
        task: &str,
        snapshot_revision: i64,
        snapshot_version: i64,
        snapshot_digest: &str,
        configuration_generation: i64,
        preview_digest: &str,
        data_types: &[String],
        purpose: &str,
    ) -> Result<(), ProviderError> {
        let data_types_json =
            serde_json::to_string(data_types).map_err(|_| ProviderError::Storage)?;
        let unknown: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM provider_transfers WHERE meeting_id = ? AND provider = ? AND task = ? AND status IN ('sending', 'outcome_unknown'))")
            .bind(meeting_id).bind(provider).bind(task).fetch_one(pool).await.map_err(|_| ProviderError::Storage)?;
        if unknown {
            return Err(ProviderError::OutcomeUnknown);
        }
        sqlx::query("INSERT INTO provider_transfers (id, meeting_id, provider, task, snapshot_revision, snapshot_version, snapshot_digest, configuration_generation, preview_digest, data_types_json, purpose, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'confirmed')")
            .bind(id)
            .bind(meeting_id)
            .bind(provider)
            .bind(task)
            .bind(snapshot_revision)
            .bind(snapshot_version)
            .bind(snapshot_digest)
            .bind(configuration_generation)
            .bind(preview_digest)
            .bind(data_types_json)
            .bind(purpose)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        Ok(())
    }

    pub(crate) async fn transfer(
        pool: &SqlitePool,
        preview_digest: &str,
    ) -> Result<TransferRow, ProviderError> {
        sqlx::query_as("SELECT id, meeting_id, provider, task, snapshot_revision, snapshot_version, snapshot_digest, configuration_generation, preview_digest, purpose, status FROM provider_transfers WHERE preview_digest = ?")
            .bind(preview_digest)
            .fetch_optional(pool)
            .await
            .map_err(|_| ProviderError::Storage)?
            .ok_or(ProviderError::StalePreview)
    }

    pub(crate) async fn begin_send(
        pool: &SqlitePool,
        transfer: &TransferRow,
    ) -> Result<(), ProviderError> {
        let updated = sqlx::query("UPDATE provider_transfers SET status = 'sending' WHERE id = ? AND preview_digest = ? AND status = 'confirmed' AND EXISTS (SELECT 1 FROM meetings WHERE id = ? AND principal_transcript_revision = ? AND canonical_snapshot_version = ?) AND EXISTS (SELECT 1 FROM provider_authorizations WHERE provider = ? AND task = ? AND enabled = 1 AND generation = ? AND ready_generation = generation)")
            .bind(&transfer.id)
            .bind(&transfer.preview_digest)
            .bind(&transfer.meeting_id)
            .bind(transfer.snapshot_revision)
            .bind(transfer.snapshot_version)
            .bind(&transfer.provider)
            .bind(&transfer.task)
            .bind(transfer.configuration_generation)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        if updated.rows_affected() == 1 {
            Ok(())
        } else {
            let current = Self::transfer(pool, &transfer.preview_digest).await?;
            if current.status != "confirmed" {
                Err(ProviderError::AlreadyConsumed)
            } else {
                let (_, enabled) =
                    Self::configuration(pool, &transfer.provider, &transfer.task).await?;
                if !enabled {
                    Err(ProviderError::TaskDisabled)
                } else {
                    Err(ProviderError::StalePreview)
                }
            }
        }
    }

    pub(crate) async fn complete(
        pool: &SqlitePool,
        id: &str,
        result: &serde_json::Value,
    ) -> Result<(), ProviderError> {
        let result = serde_json::to_string(result).map_err(|_| ProviderError::InvalidResponse)?;
        if result.len() > 512_000 {
            return Err(ProviderError::InvalidResponse);
        }
        let updated = sqlx::query("UPDATE provider_transfers SET status = 'completed', result_json = ?, error_code = NULL, completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'sending'")
            .bind(result)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        (updated.rows_affected() == 1)
            .then_some(())
            .ok_or(ProviderError::AlreadyConsumed)
    }

    pub(crate) async fn fail(pool: &SqlitePool, id: &str, code: &str) -> Result<(), ProviderError> {
        let updated = sqlx::query("UPDATE provider_transfers SET status = 'failed', result_json = NULL, error_code = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'sending'")
            .bind(code)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        (updated.rows_affected() == 1)
            .then_some(())
            .ok_or(ProviderError::AlreadyConsumed)
    }

    pub(crate) async fn mark_outcome_unknown(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<(), ProviderError> {
        let updated = sqlx::query("UPDATE provider_transfers SET status = 'outcome_unknown', result_json = NULL, error_code = 'outcome_unknown', completed_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'sending'")
            .bind(id).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        (updated.rows_affected() == 1)
            .then_some(())
            .ok_or(ProviderError::AlreadyConsumed)
    }

    pub(crate) async fn recover_unresolved_transfers(
        pool: &SqlitePool,
    ) -> Result<u64, ProviderError> {
        let updated = sqlx::query("UPDATE provider_transfers SET status = 'outcome_unknown', result_json = NULL, error_code = 'outcome_unknown', completed_at = CURRENT_TIMESTAMP WHERE status = 'sending'")
            .execute(pool).await.map_err(|_| ProviderError::Storage)?;
        Ok(updated.rows_affected())
    }

    pub(crate) async fn stage_replacement(
        pool: &SqlitePool,
        configuration: &ProviderConfiguration,
    ) -> Result<StagedCredential, ProviderError> {
        validate_configuration(configuration)?;
        let current = Self::authorization(pool, &configuration.provider, &configuration.task)
            .await
            .ok();
        if current
            .as_ref()
            .is_some_and(|row| row.ready_generation != row.generation)
        {
            return Err(ProviderError::InvalidConfiguration);
        }
        let generation = current.as_ref().map_or(1, |row| row.generation + 1);
        let account = credential_generation_account(configuration, generation);
        sqlx::query("INSERT INTO provider_authorizations (provider, task, display_name, endpoint, model, enabled, generation, ready_generation, credential_account) VALUES (?, ?, ?, ?, ?, 0, ?, -1, ?) ON CONFLICT(provider, task) DO UPDATE SET superseded_display_name = provider_authorizations.display_name, superseded_endpoint = provider_authorizations.endpoint, superseded_model = provider_authorizations.model, superseded_generation = provider_authorizations.generation, superseded_ready_generation = provider_authorizations.ready_generation, superseded_credential_account = provider_authorizations.credential_account, display_name = excluded.display_name, endpoint = excluded.endpoint, model = excluded.model, enabled = 0, generation = excluded.generation, ready_generation = -1, credential_account = excluded.credential_account, updated_at = CURRENT_TIMESTAMP")
            .bind(&configuration.provider).bind(&configuration.task).bind(&configuration.display_name)
            .bind(&configuration.endpoint).bind(&configuration.model).bind(generation).bind(&account)
            .execute(pool).await.map_err(|_| ProviderError::Storage)?;
        Ok(StagedCredential {
            credential_account: account,
            generation,
        })
    }

    pub(crate) async fn rollback_staged_replacement(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
        generation: i64,
    ) -> Result<(), ProviderError> {
        let restored = sqlx::query("UPDATE provider_authorizations SET display_name = superseded_display_name, endpoint = superseded_endpoint, model = superseded_model, enabled = 0, generation = superseded_generation, ready_generation = superseded_ready_generation, credential_account = superseded_credential_account, superseded_display_name = NULL, superseded_endpoint = NULL, superseded_model = NULL, superseded_generation = NULL, superseded_ready_generation = NULL, superseded_credential_account = NULL, updated_at = CURRENT_TIMESTAMP WHERE provider = ? AND task = ? AND generation = ? AND ready_generation <> generation AND superseded_generation IS NOT NULL")
            .bind(provider).bind(task).bind(generation).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        if restored.rows_affected() == 0 {
            sqlx::query("DELETE FROM provider_authorizations WHERE provider = ? AND task = ? AND generation = ? AND ready_generation <> generation AND superseded_generation IS NULL")
                .bind(provider).bind(task).bind(generation).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        }
        Ok(())
    }

    pub(crate) async fn cleanup_superseded_credential(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
        provider: &str,
        task: &str,
    ) -> Result<(), ProviderError> {
        let row = Self::authorization(pool, provider, task).await?;
        if let Some(account) = row
            .superseded_credential_account
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            credentials.remove(account)?;
        }
        sqlx::query("UPDATE provider_authorizations SET superseded_display_name = NULL, superseded_endpoint = NULL, superseded_model = NULL, superseded_generation = NULL, superseded_ready_generation = NULL, superseded_credential_account = NULL WHERE provider = ? AND task = ?")
            .bind(provider).bind(task).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        Ok(())
    }

    pub(crate) async fn remove_all_credentials(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
        provider: &str,
        task: &str,
    ) -> Result<(), ProviderError> {
        let row = Self::authorization(pool, provider, task).await?;
        let current = if row.credential_account.is_empty() {
            credential_account(provider, task)
        } else {
            row.credential_account
        };
        credentials.remove(&current)?;
        if let Some(account) = row
            .superseded_credential_account
            .as_deref()
            .filter(|value| !value.is_empty() && *value != current)
        {
            credentials.remove(account)?;
        }
        sqlx::query("UPDATE provider_authorizations SET enabled = 0, superseded_display_name = NULL, superseded_endpoint = NULL, superseded_model = NULL, superseded_generation = NULL, superseded_ready_generation = NULL, superseded_credential_account = NULL WHERE provider = ? AND task = ?")
            .bind(provider).bind(task).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        Ok(())
    }

    pub(crate) async fn mark_generation_ready(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
        generation: i64,
    ) -> Result<(), ProviderError> {
        let updated = sqlx::query("UPDATE provider_authorizations SET ready_generation = generation, enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE provider = ? AND task = ? AND generation = ? AND ready_generation <> generation")
            .bind(provider).bind(task).bind(generation).execute(pool).await.map_err(|_| ProviderError::Storage)?;
        (updated.rows_affected() == 1)
            .then_some(())
            .ok_or(ProviderError::InvalidConfiguration)
    }

    pub(crate) async fn recover_staged_credentials(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
    ) -> Result<(), ProviderError> {
        let rows: Vec<(String, String, i64, String)> = sqlx::query_as("SELECT provider, task, generation, credential_account FROM provider_authorizations WHERE ready_generation <> generation")
            .fetch_all(pool).await.map_err(|_| ProviderError::Storage)?;
        for (provider, task, generation, account) in rows {
            if !account.is_empty() && credentials.read(&account)?.is_some() {
                Self::mark_generation_ready(pool, &provider, &task, generation).await?;
                Self::cleanup_superseded_credential(pool, credentials, &provider, &task).await?;
            } else {
                Self::rollback_staged_replacement(pool, &provider, &task, generation).await?;
            }
        }
        let cleanup: Vec<(String, String)> = sqlx::query_as("SELECT provider, task FROM provider_authorizations WHERE ready_generation = generation AND superseded_credential_account IS NOT NULL")
            .fetch_all(pool).await.map_err(|_| ProviderError::Storage)?;
        for (provider, task) in cleanup {
            Self::cleanup_superseded_credential(pool, credentials, &provider, &task).await?;
        }
        Ok(())
    }

    pub async fn configure(
        pool: &SqlitePool,
        configuration: &ProviderConfiguration,
    ) -> Result<(), ProviderError> {
        validate_configuration(configuration)?;
        sqlx::query("INSERT INTO provider_authorizations (provider, task, display_name, endpoint, model, enabled) VALUES (?, ?, ?, ?, ?, 0) ON CONFLICT(provider, task) DO UPDATE SET display_name = excluded.display_name, endpoint = excluded.endpoint, model = excluded.model, enabled = 0, updated_at = CURRENT_TIMESTAMP")
            .bind(&configuration.provider)
            .bind(&configuration.task)
            .bind(&configuration.display_name)
            .bind(&configuration.endpoint)
            .bind(&configuration.model)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        Ok(())
    }

    pub async fn set_enabled(
        pool: &SqlitePool,
        provider: &str,
        task: &str,
        enabled: bool,
    ) -> Result<(), ProviderError> {
        let result = sqlx::query("UPDATE provider_authorizations SET enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE provider = ? AND task = ? AND (? = 0 OR ready_generation = generation)")
            .bind(enabled)
            .bind(provider)
            .bind(task)
            .bind(enabled)
            .execute(pool)
            .await
            .map_err(|_| ProviderError::Storage)?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(ProviderError::InvalidConfiguration)
        }
    }

    pub async fn status(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
        provider: &str,
        task: &str,
    ) -> Result<ProviderAuthorizationStatus, ProviderError> {
        let authorization = Self::authorization(pool, provider, task).await?;
        let configuration = ProviderConfiguration {
            provider: provider.into(),
            display_name: authorization.display_name,
            endpoint: authorization.endpoint,
            model: authorization.model,
            task: task.into(),
        };
        let account = if authorization.credential_account.is_empty() {
            credential_account(provider, task)
        } else {
            authorization.credential_account
        };
        let credential_present = authorization.ready_generation == authorization.generation
            && credentials.read(&account)?.is_some();
        Ok(ProviderAuthorizationStatus {
            provider: provider.into(),
            task: task.into(),
            endpoint: configuration.endpoint,
            model: configuration.model,
            enabled: authorization.enabled && credential_present,
            credential_present,
            credential_mask: credential_present.then_some("••••••••"),
        })
    }

    pub async fn migrate_legacy_credentials(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
    ) -> Result<u64, ProviderError> {
        let mut migrated = 0;
        let mut connection = pool.acquire().await.map_err(|_| ProviderError::Storage)?;
        sqlx::query("PRAGMA secure_delete = ON")
            .execute(&mut *connection)
            .await
            .map_err(|_| ProviderError::Storage)?;
        let mut transaction = connection
            .begin()
            .await
            .map_err(|_| ProviderError::Storage)?;
        let mut purge_pending: bool = sqlx::query_scalar(
            "SELECT legacy_secret_purge_pending FROM provider_security_state WHERE id = 1",
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| ProviderError::Storage)?;
        if let Some(row) = sqlx::query("SELECT groqApiKey, openaiApiKey, anthropicApiKey, ollamaApiKey, openRouterApiKey, geminiApiKey, customOpenAIConfig FROM settings WHERE id = '1'")
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| ProviderError::Storage)?
        {
            for (column, provider) in [
                ("groqApiKey", "groq"),
                ("openaiApiKey", "openai"),
                ("anthropicApiKey", "claude"),
                ("ollamaApiKey", "ollama"),
                ("openRouterApiKey", "openrouter"),
                ("geminiApiKey", "gemini"),
            ] {
                if let Some(secret) = row.try_get::<Option<String>, _>(column).map_err(|_| ProviderError::Storage)? {
                    purge_pending |= !secret.is_empty();
                    let account = summary_credential_account(provider);
                    if !secret.is_empty() && credentials.read(&account)?.is_none() {
                        credentials.save(&account, &secret)?;
                        migrated += 1;
                    }
                }
            }
            let custom = row.try_get::<Option<String>, _>("customOpenAIConfig").map_err(|_| ProviderError::Storage)?;
            let sanitized_custom = if let Some(custom) = custom {
                let mut json: serde_json::Value = serde_json::from_str(&custom).map_err(|_| ProviderError::Storage)?;
                if let Some(secret) = json.get("apiKey").and_then(|value| value.as_str()).filter(|value| !value.is_empty()) {
                    purge_pending = true;
                    let account = summary_credential_account("custom-openai");
                    if credentials.read(&account)?.is_none() {
                        credentials.save(&account, secret)?;
                        migrated += 1;
                    }
                }
                json.as_object_mut().map(|object| object.remove("apiKey"));
                Some(serde_json::to_string(&json).map_err(|_| ProviderError::Storage)?)
            } else {
                None
            };
            sqlx::query("UPDATE settings SET groqApiKey = NULL, openaiApiKey = NULL, anthropicApiKey = NULL, ollamaApiKey = NULL, openRouterApiKey = NULL, geminiApiKey = NULL, customOpenAIConfig = ? WHERE id = '1'")
                .bind(sanitized_custom)
                .execute(&mut *transaction)
                .await
                .map_err(|_| ProviderError::Storage)?;
        }
        if let Some(row) = sqlx::query("SELECT whisperApiKey, deepgramApiKey, elevenLabsApiKey, groqApiKey, openaiApiKey FROM transcript_settings WHERE id = '1'")
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| ProviderError::Storage)?
        {
            for (column, account) in [
                ("whisperApiKey", "transcript:whisper"),
                ("deepgramApiKey", "transcript:deepgram"),
                ("elevenLabsApiKey", "transcript:elevenlabs"),
                ("groqApiKey", "transcript:groq"),
                ("openaiApiKey", "transcript:openai"),
            ] {
                if let Some(secret) = row.try_get::<Option<String>, _>(column).map_err(|_| ProviderError::Storage)? {
                    purge_pending |= !secret.is_empty();
                    if !secret.is_empty() && credentials.read(account)?.is_none() {
                        credentials.save(account, &secret)?;
                        migrated += 1;
                    }
                }
            }
            sqlx::query("UPDATE transcript_settings SET whisperApiKey = NULL, deepgramApiKey = NULL, elevenLabsApiKey = NULL, groqApiKey = NULL, openaiApiKey = NULL WHERE id = '1'")
                .execute(&mut *transaction)
                .await
                .map_err(|_| ProviderError::Storage)?;
        }
        if purge_pending {
            sqlx::query("UPDATE provider_security_state SET legacy_secret_purge_pending = 1, updated_at = CURRENT_TIMESTAMP WHERE id = 1")
                .execute(&mut *transaction)
                .await
                .map_err(|_| ProviderError::Storage)?;
        }
        transaction
            .commit()
            .await
            .map_err(|_| ProviderError::Storage)?;
        if purge_pending {
            checkpoint_provider_secret_purge(&mut connection).await?;
            sqlx::query("VACUUM")
                .execute(&mut *connection)
                .await
                .map_err(|_| ProviderError::Storage)?;
            checkpoint_provider_secret_purge(&mut connection).await?;
            scrub_retained_legacy_database(&mut connection).await?;
            sqlx::query("UPDATE provider_security_state SET legacy_secret_purge_pending = 0, updated_at = CURRENT_TIMESTAMP WHERE id = 1")
                .execute(&mut *connection)
                .await
                .map_err(|_| ProviderError::Storage)?;
        }
        drop(connection);
        for provider in [
            "groq",
            "openai",
            "claude",
            "ollama",
            "openrouter",
            "gemini",
            "custom-openai",
        ] {
            let account = summary_credential_account(provider);
            if credentials.read(&account)?.is_none() {
                if let Some(secret) = credentials.read(provider)? {
                    let value = std::str::from_utf8(secret.expose())
                        .map_err(|_| ProviderError::InvalidConfiguration)?;
                    credentials.save(&account, value)?;
                    migrated += 1;
                }
            }
            if credentials.read(&account)?.is_some() {
                credentials.remove(provider)?;
            }
        }
        let legacy_authorizations: Vec<(String, String, bool)> = sqlx::query_as(
            "SELECT provider, task, enabled FROM provider_authorizations WHERE generation = 0 AND credential_account = ''",
        )
        .fetch_all(pool)
        .await
        .map_err(|_| ProviderError::Storage)?;
        for (provider, task, was_enabled) in legacy_authorizations {
            let legacy_account = credential_account(&provider, &task);
            if let Some(secret) = credentials.read(&legacy_account)? {
                let (configuration, _) = Self::configuration(pool, &provider, &task).await?;
                let staged = Self::stage_replacement(pool, &configuration).await?;
                if credentials.read(&staged.credential_account)?.is_none() {
                    let value = std::str::from_utf8(secret.expose())
                        .map_err(|_| ProviderError::InvalidConfiguration)?;
                    credentials.save(&staged.credential_account, value)?;
                    migrated += 1;
                }
                Self::mark_generation_ready(pool, &provider, &task, staged.generation).await?;
                if was_enabled {
                    Self::set_enabled(pool, &provider, &task, true).await?;
                }
                credentials.remove(&legacy_account)?;
            }
        }
        Self::recover_staged_credentials(pool, credentials).await?;
        Ok(migrated)
    }

    pub async fn migrate_legacy_credentials_on_startup(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
    ) -> bool {
        Self::migrate_legacy_credentials(pool, credentials)
            .await
            .is_ok()
    }

    pub async fn require_legacy_credentials_migrated(
        pool: &SqlitePool,
        credentials: &dyn CredentialStore,
    ) -> Result<(), ProviderError> {
        Self::migrate_legacy_credentials(pool, credentials)
            .await
            .map(|_| ())
    }
}

async fn checkpoint_provider_secret_purge(
    connection: &mut sqlx::SqliteConnection,
) -> Result<(), ProviderError> {
    let checkpoint: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(connection)
        .await
        .map_err(|_| ProviderError::Storage)?;
    if checkpoint.0 == 0 && checkpoint.1 == checkpoint.2 {
        Ok(())
    } else {
        Err(ProviderError::Storage)
    }
}

async fn scrub_retained_legacy_database(
    active: &mut sqlx::SqliteConnection,
) -> Result<(), ProviderError> {
    let active_path: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(&mut *active)
            .await
            .map_err(|_| ProviderError::Storage)?;
    let active_path = Path::new(&active_path);
    if active_path.file_name().and_then(|name| name.to_str()) != Some("meeting_minutes.sqlite") {
        return Ok(());
    }
    let legacy_path = active_path.with_file_name("meeting_minutes.db");
    if !legacy_path.exists() {
        return Ok(());
    }

    let options = SqliteConnectOptions::new()
        .filename(&legacy_path)
        .create_if_missing(false);
    let mut legacy = sqlx::SqliteConnection::connect_with(&options)
        .await
        .map_err(|_| ProviderError::Storage)?;
    sqlx::query("PRAGMA secure_delete = ON")
        .execute(&mut legacy)
        .await
        .map_err(|_| ProviderError::Storage)?;
    let mut transaction = legacy.begin().await.map_err(|_| ProviderError::Storage)?;
    scrub_legacy_table(
        &mut transaction,
        "settings",
        &[
            "groqApiKey",
            "openaiApiKey",
            "anthropicApiKey",
            "ollamaApiKey",
            "openRouterApiKey",
            "geminiApiKey",
            "customOpenAIConfig",
        ],
    )
    .await?;
    scrub_legacy_table(
        &mut transaction,
        "transcript_settings",
        &[
            "whisperApiKey",
            "deepgramApiKey",
            "elevenLabsApiKey",
            "groqApiKey",
            "openaiApiKey",
        ],
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| ProviderError::Storage)?;
    checkpoint_provider_secret_purge(&mut legacy).await?;
    sqlx::query("VACUUM")
        .execute(&mut legacy)
        .await
        .map_err(|_| ProviderError::Storage)?;
    checkpoint_provider_secret_purge(&mut legacy).await
}

async fn scrub_legacy_table(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &str,
    sensitive_columns: &[&str],
) -> Result<(), ProviderError> {
    let columns: Vec<String> =
        sqlx::query_scalar(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(&mut **transaction)
            .await
            .map_err(|_| ProviderError::Storage)?;
    let assignments = sensitive_columns
        .iter()
        .filter(|column| columns.iter().any(|existing| existing == **column))
        .map(|column| format!("{column} = NULL"))
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        sqlx::query(&format!("UPDATE {table} SET {}", assignments.join(", ")))
            .execute(&mut **transaction)
            .await
            .map_err(|_| ProviderError::Storage)?;
    }
    Ok(())
}

fn validate_configuration(configuration: &ProviderConfiguration) -> Result<(), ProviderError> {
    let endpoint = url::Url::parse(&configuration.endpoint)
        .map_err(|_| ProviderError::InvalidConfiguration)?;
    let loopback = match endpoint.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    let secure_transport =
        endpoint.scheme() == "https" || (endpoint.scheme() == "http" && loopback);
    if configuration.provider.is_empty()
        || configuration.provider.len() > 64
        || !configuration
            .provider
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || configuration.display_name.trim().is_empty()
        || configuration.model.trim().is_empty()
        || configuration.task != super::AGENT_HANDOFF_TASK
        || !secure_transport
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
    {
        Err(ProviderError::InvalidConfiguration)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::SecretString;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryCredentials(Mutex<HashMap<String, Vec<u8>>>);

    impl CredentialStore for MemoryCredentials {
        fn save(&self, provider: &str, secret: &str) -> Result<(), ProviderError> {
            self.0
                .lock()
                .unwrap()
                .insert(provider.to_string(), secret.as_bytes().to_vec());
            Ok(())
        }

        fn read(&self, provider: &str) -> Result<Option<SecretString>, ProviderError> {
            Ok(self.0.lock().unwrap().get(provider).map(SecretString::new))
        }

        fn remove(&self, provider: &str) -> Result<(), ProviderError> {
            self.0.lock().unwrap().remove(provider);
            Ok(())
        }
    }

    struct UnavailableCredentials;

    impl CredentialStore for UnavailableCredentials {
        fn save(&self, _provider: &str, _secret: &str) -> Result<(), ProviderError> {
            Err(ProviderError::Storage)
        }

        fn read(&self, _provider: &str) -> Result<Option<SecretString>, ProviderError> {
            Err(ProviderError::Storage)
        }

        fn remove(&self, _provider: &str) -> Result<(), ProviderError> {
            Err(ProviderError::Storage)
        }
    }

    async fn fixture() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn migrates_every_plaintext_secret_then_clears_sqlite_idempotently() {
        let pool = fixture().await;
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey, openaiApiKey, anthropicApiKey, ollamaApiKey, openRouterApiKey, geminiApiKey, customOpenAIConfig) VALUES ('1', 'custom-openai', 'fixture', 'fixture', 'groq-secret', 'openai-secret', 'claude-secret', 'ollama-secret', 'router-secret', 'gemini-secret', ?)")
            .bind(r#"{"endpoint":"http://127.0.0.1:9","apiKey":"custom-secret","model":"fixture"}"#)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcript_settings (id, provider, model, whisperApiKey, deepgramApiKey, elevenLabsApiKey, groqApiKey, openaiApiKey) VALUES ('1', 'openai', 'fixture', 'whisper-secret', 'deepgram-secret', 'eleven-secret', 'transcript-groq-secret', 'transcript-openai-secret')")
            .execute(&pool).await.unwrap();
        let credentials = MemoryCredentials::default();

        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
                .await
                .unwrap(),
            12
        );
        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
                .await
                .unwrap(),
            0
        );

        // Test tuple mirrors the exact legacy settings projection being scrubbed.
        #[allow(clippy::type_complexity)]
        let summary: (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as("SELECT groqApiKey, openaiApiKey, anthropicApiKey, ollamaApiKey, openRouterApiKey, geminiApiKey, customOpenAIConfig FROM settings WHERE id = '1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(summary.0, None);
        assert_eq!(summary.1, None);
        assert_eq!(summary.2, None);
        assert_eq!(summary.3, None);
        assert_eq!(summary.4, None);
        assert_eq!(summary.5, None);
        assert!(!summary.6.unwrap().contains("apiKey"));
        // Test tuple mirrors the exact legacy transcript-settings projection being scrubbed.
        #[allow(clippy::type_complexity)]
        let transcript: (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as("SELECT whisperApiKey, deepgramApiKey, elevenLabsApiKey, groqApiKey, openaiApiKey FROM transcript_settings WHERE id = '1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(transcript, (None, None, None, None, None));

        let stored = credentials.0.lock().unwrap();
        let all = stored.values().flatten().copied().collect::<Vec<_>>();
        for secret in [
            "groq-secret",
            "openai-secret",
            "claude-secret",
            "ollama-secret",
            "router-secret",
            "gemini-secret",
            "custom-secret",
            "whisper-secret",
            "deepgram-secret",
            "eleven-secret",
            "transcript-groq-secret",
            "transcript-openai-secret",
        ] {
            assert!(all
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    #[tokio::test]
    async fn migration_removes_plaintext_secret_bytes_from_sqlite_and_wal() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("provider.sqlite");
        let options = SqliteConnectOptions::from_str(database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let secret = "synthetic-legacy-secret-that-must-not-remain";
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey) VALUES ('1', 'groq', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();

        ProviderRepository::migrate_legacy_credentials(&pool, &MemoryCredentials::default())
            .await
            .unwrap();

        let mut persisted = std::fs::read(&database).unwrap();
        let wal = database.with_extension("sqlite-wal");
        if wal.exists() {
            persisted.extend(std::fs::read(wal).unwrap());
        }
        assert!(!persisted
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
    }

    #[tokio::test]
    async fn migration_scrubs_secrets_left_in_historical_freelist_pages() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("provider.sqlite");
        let options = SqliteConnectOptions::from_str(database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let secret = "synthetic-historical-secret-that-must-not-remain";
        sqlx::query("PRAGMA secure_delete = OFF")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE historical_settings (secret TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO historical_settings (secret) VALUES (?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DROP TABLE historical_settings")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
        let freelist: i64 = sqlx::query_scalar("PRAGMA freelist_count")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(freelist > 0);

        ProviderRepository::migrate_legacy_credentials(&pool, &MemoryCredentials::default())
            .await
            .unwrap();

        let persisted = std::fs::read(&database).unwrap();
        assert!(!persisted
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
    }

    #[tokio::test]
    async fn migration_scrubs_the_retained_legacy_database_and_wal_copy() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("meeting_minutes.sqlite");
        let legacy_database = directory.path().join("meeting_minutes.db");
        let options = SqliteConnectOptions::from_str(database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let legacy_options = SqliteConnectOptions::from_str(legacy_database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let legacy_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(legacy_options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE settings (id TEXT PRIMARY KEY, groqApiKey TEXT, customOpenAIConfig TEXT)",
        )
        .execute(&legacy_pool)
        .await
        .unwrap();
        let main_secret = "synthetic-retained-legacy-main-secret";
        let wal_secret = "synthetic-retained-legacy-wal-secret";
        let custom = format!(r#"{{"apiKey":"{wal_secret}"}}"#);
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey, customOpenAIConfig) VALUES ('1', 'groq', 'fixture', 'fixture', ?, ?)")
            .bind(main_secret)
            .bind(&custom)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, groqApiKey) VALUES ('1', ?)")
            .bind(main_secret)
            .execute(&legacy_pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&legacy_pool)
            .await
            .unwrap();
        sqlx::query("UPDATE settings SET customOpenAIConfig = ? WHERE id = '1'")
            .bind(&custom)
            .execute(&legacy_pool)
            .await
            .unwrap();

        let credentials = MemoryCredentials::default();
        ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
            .await
            .unwrap();
        assert_eq!(
            credentials
                .read(&summary_credential_account("groq"))
                .unwrap()
                .unwrap()
                .expose(),
            main_secret.as_bytes()
        );

        let mut persisted = std::fs::read(&legacy_database).unwrap();
        let wal = legacy_database.with_extension("db-wal");
        if wal.exists() {
            persisted.extend(std::fs::read(wal).unwrap());
        }
        for secret in [main_secret, wal_secret] {
            assert!(!persisted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    #[tokio::test]
    async fn failed_keychain_write_preserves_the_retained_legacy_rollback_copy() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("meeting_minutes.sqlite");
        let legacy_database = directory.path().join("meeting_minutes.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&database)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let secret = "synthetic-retained-rollback-secret";
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey) VALUES ('1', 'groq', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        let mut legacy = sqlx::SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&legacy_database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE settings (id TEXT PRIMARY KEY, groqApiKey TEXT)")
            .execute(&mut legacy)
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, groqApiKey) VALUES ('1', ?)")
            .bind(secret)
            .execute(&mut legacy)
            .await
            .unwrap();
        drop(legacy);

        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(&pool, &UnavailableCredentials).await,
            Err(ProviderError::Storage)
        );

        let persisted = std::fs::read(&legacy_database).unwrap();
        assert!(persisted
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
    }

    #[tokio::test]
    async fn completed_secret_purge_does_not_checkpoint_unrelated_wal_activity() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("provider.sqlite");
        let options = SqliteConnectOptions::from_str(database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let credentials = MemoryCredentials::default();
        ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE unrelated_writes (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        let mut reader = pool.begin().await.unwrap();
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unrelated_writes")
            .fetch_one(&mut *reader)
            .await
            .unwrap();
        sqlx::query("INSERT INTO unrelated_writes (id) VALUES (1)")
            .execute(&pool)
            .await
            .unwrap();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            ProviderRepository::migrate_legacy_credentials(&pool, &credentials),
        )
        .await;

        assert!(matches!(result, Ok(Ok(0))));
        reader.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn interrupted_secret_purge_stays_pending_and_retries_after_reader_closes() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("provider.sqlite");
        let options = SqliteConnectOptions::from_str(database.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("CREATE TABLE purge_reader_anchor (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO purge_reader_anchor (id) VALUES (1)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
        let mut reader = pool.begin().await.unwrap();
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM purge_reader_anchor")
            .fetch_one(&mut *reader)
            .await
            .unwrap();
        let secret = "synthetic-retryable-purge-secret";
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey) VALUES ('1', 'groq', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        let credentials = MemoryCredentials::default();

        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(&pool, &credentials).await,
            Err(ProviderError::Storage)
        );
        let pending: bool = sqlx::query_scalar(
            "SELECT legacy_secret_purge_pending FROM provider_security_state WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(pending);
        reader.rollback().await.unwrap();

        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(&pool, &credentials)
                .await
                .unwrap(),
            0
        );
        let pending: bool = sqlx::query_scalar(
            "SELECT legacy_secret_purge_pending FROM provider_security_state WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!pending);
        let persisted = std::fs::read(&database).unwrap();
        assert!(!persisted
            .windows(secret.len())
            .any(|window| window == secret.as_bytes()));
    }

    #[tokio::test]
    async fn unavailable_keychain_defers_startup_and_keeps_plaintext_for_retry() {
        let pool = fixture().await;
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey) VALUES ('1', 'groq', 'fixture', 'fixture', 'synthetic-legacy-secret')")
            .execute(&pool)
            .await
            .unwrap();

        assert!(
            !ProviderRepository::migrate_legacy_credentials_on_startup(
                &pool,
                &UnavailableCredentials,
            )
            .await
        );

        let secret: Option<String> =
            sqlx::query_scalar("SELECT groqApiKey FROM settings WHERE id = '1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(secret.as_deref(), Some("synthetic-legacy-secret"));
    }

    #[tokio::test]
    async fn task_authorization_defaults_disabled_and_never_returns_the_secret() {
        let pool = fixture().await;
        let credentials = MemoryCredentials::default();
        credentials
            .save(
                &credential_account("fixture", super::super::AGENT_HANDOFF_TASK),
                "complete-private-secret",
            )
            .unwrap();
        let configuration = ProviderConfiguration {
            provider: "fixture".into(),
            display_name: "Fixture Provider".into(),
            endpoint: "http://127.0.0.1:9".into(),
            model: "fixture".into(),
            task: super::super::AGENT_HANDOFF_TASK.into(),
        };
        ProviderRepository::configure(&pool, &configuration)
            .await
            .unwrap();

        let initial = ProviderRepository::status(
            &pool,
            &credentials,
            "fixture",
            super::super::AGENT_HANDOFF_TASK,
        )
        .await
        .unwrap();
        assert!(!initial.enabled);
        assert!(initial.credential_present);
        assert_eq!(initial.credential_mask, Some("••••••••"));
        assert_eq!(initial.endpoint, "http://127.0.0.1:9");
        assert_eq!(initial.model, "fixture");
        assert!(!serde_json::to_string(&initial)
            .unwrap()
            .contains("complete-private-secret"));

        ProviderRepository::set_enabled(&pool, "fixture", super::super::AGENT_HANDOFF_TASK, true)
            .await
            .unwrap();
        assert!(
            ProviderRepository::status(
                &pool,
                &credentials,
                "fixture",
                super::super::AGENT_HANDOFF_TASK
            )
            .await
            .unwrap()
            .enabled
        );
    }

    #[tokio::test]
    async fn rejects_remote_cleartext_and_userinfo_endpoints() {
        let pool = fixture().await;
        for endpoint in [
            "http://provider.example/v1",
            "https://user@provider.example/v1",
            "https://user:secret@provider.example/v1",
        ] {
            let configuration = ProviderConfiguration {
                provider: "fixture".into(),
                display_name: "Fixture Provider".into(),
                endpoint: endpoint.into(),
                model: "fixture".into(),
                task: super::super::AGENT_HANDOFF_TASK.into(),
            };
            assert_eq!(
                ProviderRepository::configure(&pool, &configuration).await,
                Err(ProviderError::InvalidConfiguration),
                "endpoint must not persist: {endpoint}"
            );
        }

        for endpoint in [
            "http://127.0.0.1:18787/v1",
            "http://[::1]:18787/v1",
            "http://localhost:18787/v1",
            "https://provider.example/v1",
        ] {
            let configuration = ProviderConfiguration {
                provider: "fixture".into(),
                display_name: "Fixture Provider".into(),
                endpoint: endpoint.into(),
                model: "fixture".into(),
                task: super::super::AGENT_HANDOFF_TASK.into(),
            };
            ProviderRepository::configure(&pool, &configuration)
                .await
                .unwrap();
        }
    }
}
