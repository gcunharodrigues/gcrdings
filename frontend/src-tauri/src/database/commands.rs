use log::{error, info};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::manager::DatabaseManager;
use crate::providers::{keychain::MacKeychain, repository::ProviderRepository, CredentialStore};
use crate::state::AppState;

static DATABASE_INITIALIZATION_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn publish_database_ready(
    completion: sqlx::Result<()>,
    emit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    completion
        .map_err(|_| "Legacy database import completion could not be secured.".to_string())?;
    emit()
}

/// Check if this is the first launch (no database exists yet)
#[tauri::command]
pub async fn check_first_launch(app: AppHandle) -> Result<bool, String> {
    DatabaseManager::is_first_launch(&app)
        .await
        .map_err(|e| format!("Failed to check first launch: {}", e))
}

/// Open a dialog to select a folder or file for legacy database import
#[tauri::command]
pub async fn select_legacy_database_path(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    info!("Opening dialog to select legacy database location");

    let file_path = app
        .dialog()
        .file()
        .add_filter("Database Files", &["db"])
        .blocking_pick_file();

    if let Some(path) = file_path {
        let path_str = path.to_string();
        info!("User selected path: {}", path_str);
        Ok(Some(path_str))
    } else {
        info!("User cancelled file selection");
        Ok(None)
    }
}

/// Detect legacy database from a selected path (root repo, backend folder, or db file)
#[tauri::command]
pub async fn detect_legacy_database(selected_path: String) -> Result<Option<String>, String> {
    let path = PathBuf::from(&selected_path);

    info!("Detecting legacy database from path: {}", selected_path);

    // Case 1: User selected the .db file directly
    if path.is_file() {
        if let Some(extension) = path.extension() {
            if extension == "db" {
                info!("Direct .db file selected: {}", selected_path);
                return Ok(Some(selected_path));
            }
        }
    }

    // Case 2: User selected directory containing meeting_minutes.db
    if path.is_dir() {
        let direct_db = path.join("meeting_minutes.db");
        if direct_db.exists() && direct_db.is_file() {
            let db_path = direct_db.to_string_lossy().to_string();
            info!("Found database in selected directory: {}", db_path);
            return Ok(Some(db_path));
        }

        // Case 3: User selected root repo (check backend subdirectory)
        let backend_db = path.join("backend").join("meeting_minutes.db");
        if backend_db.exists() && backend_db.is_file() {
            let db_path = backend_db.to_string_lossy().to_string();
            info!("Found database in backend subdirectory: {}", db_path);
            return Ok(Some(db_path));
        }
    }

    info!("No legacy database found at path: {}", selected_path);
    Ok(None)
}

/// Check for legacy database in the default app data directory
#[tauri::command]
pub async fn check_default_legacy_database(app: AppHandle) -> Result<Option<String>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let legacy_db = app_data_dir.join("meeting_minutes.db");
    info!("Checking for default legacy database at: {:?}", legacy_db);

    if legacy_db.exists() && legacy_db.is_file() {
        let path_str = legacy_db.to_string_lossy().to_string();
        info!("Found default legacy database: {}", path_str);
        Ok(Some(path_str))
    } else {
        info!("No default legacy database found");
        Ok(None)
    }
}

/// Import legacy database and initialize the database manager
#[tauri::command]
pub async fn import_and_initialize_database(
    app: AppHandle,
    legacy_db_path: String,
) -> Result<(), String> {
    let _initialization_guard = DATABASE_INITIALIZATION_GATE.lock().await;
    info!(
        "Starting import of legacy database from: {}",
        legacy_db_path
    );

    // Import and get initialized manager
    let db_manager = DatabaseManager::import_legacy_database(&app, &legacy_db_path)
        .await
        .map_err(|e| {
            error!("Failed to import legacy database: {}", e);
            format!("Failed to import database: {}", e)
        })?;
    migrate_imported_provider_credentials(&db_manager, &MacKeychain::default()).await?;

    // Update app state with the new manager before publishing readiness.
    if !app.manage(AppState { db_manager }) {
        return Err("Database is already initialized.".to_string());
    }

    info!("Legacy database imported and initialized successfully");

    let completion = DatabaseManager::complete_legacy_import(&app).await;
    publish_database_ready(completion, || {
        // Emit only after migration, state installation, and reservation cleanup succeeded.
        app.emit("database-initialized", ())
            .map_err(|e| format!("Failed to emit database-initialized event: {}", e))
    })?;

    Ok(())
}

async fn migrate_imported_provider_credentials(
    db_manager: &DatabaseManager,
    credentials: &dyn CredentialStore,
) -> Result<(), String> {
    ProviderRepository::migrate_legacy_credentials(db_manager.pool(), credentials)
        .await
        .map(|_| ())
        .map_err(|_| {
            "Legacy credentials could not be secured. Unlock Keychain and retry the import."
                .to_string()
        })
}

/// Initialize a fresh database (for users who don't want to import)
#[tauri::command]
pub async fn initialize_fresh_database<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let _initialization_guard = DATABASE_INITIALIZATION_GATE.lock().await;
    info!("Initializing fresh database");
    if app.try_state::<AppState>().is_some() {
        return Err("Database is already initialized.".to_string());
    }

    let db_manager = DatabaseManager::create_fresh(&app).await.map_err(|e| {
        error!("Failed to initialize fresh database: {}", e);
        format!("Failed to initialize database: {}", e)
    })?;

    // Update app state with the new manager
    if !app.manage(AppState {
        db_manager: db_manager.clone(),
    }) {
        db_manager.pool().close().await;
        return Err("Database is already initialized.".to_string());
    }

    // Set default model configuration for fresh installs
    let pool = db_manager.pool();

    // V1 local findings use Apple Foundation Models without a configurable model.
    if let Err(e) = crate::database::repositories::setting::SettingsRepository::save_model_config(
        pool,
        "apple-foundation",
        "system",
        "large-v3", // Default whisper model (unused for builtin but required)
        None,
    )
    .await
    {
        error!("Failed to set default summary model config: {}", e);
    }

    // Default Transcription Model: Parakeet
    if let Err(e) =
        crate::database::repositories::setting::SettingsRepository::save_transcript_config(
            pool,
            "parakeet",
            crate::config::DEFAULT_PARAKEET_MODEL,
        )
        .await
    {
        error!("Failed to set default transcription model config: {}", e);
    }

    info!("Fresh database initialized successfully with default models");

    // Emit event to notify frontend that database is ready
    app.emit("database-initialized", ())
        .map_err(|e| format!("Failed to emit database-initialized event: {}", e))?;

    Ok(())
}

/// Get the database directory path
#[tauri::command]
pub async fn get_database_directory(app: AppHandle) -> Result<String, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    Ok(app_data_dir.to_string_lossy().to_string())
}

/// Open the database folder in the system file explorer
#[tauri::command]
pub async fn open_database_folder(app: AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    // Ensure directory exists before trying to open it
    if !app_data_dir.exists() {
        std::fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    let folder_path = app_data_dir.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    info!("Opened database folder: {}", folder_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{CredentialStore, ProviderError, SecretString};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };
    use tauri::Listener;

    #[derive(Default)]
    struct MemoryCredentials(Mutex<HashMap<String, Vec<u8>>>);

    impl CredentialStore for MemoryCredentials {
        fn save(&self, account: &str, secret: &str) -> Result<(), ProviderError> {
            self.0
                .lock()
                .unwrap()
                .insert(account.to_string(), secret.as_bytes().to_vec());
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

    #[test]
    fn failed_import_completion_does_not_publish_readiness() {
        let published = Cell::new(false);

        let result = publish_database_ready(
            Err(sqlx::Error::Protocol(
                "synthetic cleanup failure".to_string(),
            )),
            || {
                published.set(true);
                Ok(())
            },
        );

        assert!(result.is_err());
        assert!(!published.get());
    }

    #[tokio::test]
    async fn onboarding_import_migrates_credentials_before_database_ready() {
        let directory = tempfile::tempdir().unwrap();
        let legacy = directory.path().join("meeting_minutes.db");
        let active = directory.path().join("meeting_minutes.sqlite");
        let legacy_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&legacy)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations")
            .run(&legacy_pool)
            .await
            .unwrap();
        let secret = "synthetic-onboarding-import-secret";
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, groqApiKey) VALUES ('1', 'groq', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&legacy_pool)
            .await
            .unwrap();
        legacy_pool.close().await;
        let manager = DatabaseManager::new(active.to_str().unwrap(), legacy.to_str().unwrap())
            .await
            .unwrap();
        let credentials = MemoryCredentials::default();

        migrate_imported_provider_credentials(&manager, &credentials)
            .await
            .unwrap();

        let logical: Option<String> =
            sqlx::query_scalar("SELECT groqApiKey FROM settings WHERE id = '1'")
                .fetch_one(manager.pool())
                .await
                .unwrap();
        assert_eq!(logical, None);
        for path in [&active, &legacy] {
            let persisted = std::fs::read(path).unwrap();
            assert!(!persisted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    #[tokio::test]
    async fn repeated_fresh_command_preserves_configuration_and_emits_no_readiness() {
        let directory = tempfile::tempdir().unwrap();
        let active = directory.path().join("meeting_minutes.sqlite");
        let retained = directory.path().join("meeting_minutes.db");
        let manager = DatabaseManager::new(active.to_str().unwrap(), retained.to_str().unwrap())
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel) VALUES ('1', 'custom-existing', 'custom-model', 'custom-whisper')")
            .execute(manager.pool())
            .await
            .unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        assert!(app.manage(AppState {
            db_manager: manager.clone(),
        }));
        let ready_events = Arc::new(AtomicUsize::new(0));
        let observed_events = Arc::clone(&ready_events);
        app.listen("database-initialized", move |_| {
            observed_events.fetch_add(1, Ordering::SeqCst);
        });

        let result = initialize_fresh_database(app.handle().clone()).await;

        assert_eq!(result, Err("Database is already initialized.".to_string()));
        let configuration: (String, String, String) =
            sqlx::query_as("SELECT provider, model, whisperModel FROM settings WHERE id = '1'")
                .fetch_one(manager.pool())
                .await
                .unwrap();
        assert_eq!(
            configuration,
            (
                "custom-existing".to_string(),
                "custom-model".to_string(),
                "custom-whisper".to_string(),
            )
        );
        assert_eq!(ready_events.load(Ordering::SeqCst), 0);
        manager.pool().close().await;
    }

    #[tokio::test]
    async fn fresh_command_rejects_retained_legacy_secrets_without_writes_or_readiness() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        std::fs::create_dir(&app_data).unwrap();
        let retained = app_data.join("meeting_minutes.db");
        let active = app_data.join("meeting_minutes.sqlite");
        let legacy_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&retained)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations")
            .run(&legacy_pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, geminiApiKey) VALUES ('1', 'gemini', 'legacy-model', 'legacy-whisper', 'synthetic-retained-secret')")
            .execute(&legacy_pool)
            .await
            .unwrap();
        legacy_pool.close().await;
        let retained_before = std::fs::read(&retained).unwrap();
        let retained_inode = std::fs::metadata(&retained).unwrap().ino();

        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().identifier = app_data.to_string_lossy().into_owned();
        let app = tauri::test::mock_builder().build(context).unwrap();
        let ready_events = Arc::new(AtomicUsize::new(0));
        let observed_events = Arc::clone(&ready_events);
        app.listen("database-initialized", move |_| {
            observed_events.fetch_add(1, Ordering::SeqCst);
        });

        let result = initialize_fresh_database(app.handle().clone()).await;

        assert!(result.is_err());
        assert!(app.try_state::<AppState>().is_none());
        assert_eq!(ready_events.load(Ordering::SeqCst), 0);
        assert!(!active.exists());
        assert_eq!(std::fs::read(&retained).unwrap(), retained_before);
        assert_eq!(std::fs::metadata(&retained).unwrap().ino(), retained_inode);
    }
}
