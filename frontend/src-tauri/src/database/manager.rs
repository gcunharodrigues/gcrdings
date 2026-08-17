use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{
    migrate::MigrateDatabase, Connection, Result, Sqlite, SqliteConnection, SqlitePool, Transaction,
};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use tauri::Manager;

const IMPORT_STAGING_PREFIX: &str = ".gcrdings-import-";
const IMPORT_RESERVATION_FILE: &str = ".gcrdings-legacy-import-reservation";
static IMPORT_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct ImportReservation {
    digest: String,
    source_identity: Option<String>,
}

struct OpenedLegacySource {
    connection: SqliteConnection,
    binding: LegacySourceBinding,
}

struct LegacySourceBinding {
    identity: String,
    managed: bool,
}

fn remove_stale_import_snapshots(parent: &Path) -> Result<()> {
    for entry in fs::read_dir(parent).map_err(sqlx::Error::Io)? {
        let entry = entry.map_err(sqlx::Error::Io)?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(IMPORT_STAGING_PREFIX)
        {
            continue;
        }
        let file_type = entry.file_type().map_err(sqlx::Error::Io)?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path()).map_err(sqlx::Error::Io)?;
        } else {
            fs::remove_file(entry.path()).map_err(sqlx::Error::Io)?;
        }
    }
    Ok(())
}

fn filesystem_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(sqlx::Error::Io(error)),
    }
}

#[cfg(test)]
async fn copy_legacy_database(source: &Path, target: &Path) -> Result<()> {
    let _import_guard = IMPORT_GATE.lock().await;
    let parent = target.parent().ok_or_else(|| {
        sqlx::Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "legacy database target has no parent directory",
        ))
    })?;
    remove_stale_import_snapshots(parent)?;
    copy_legacy_database_unlocked(source, target, parent).await
}

async fn open_legacy_source(
    source: &Path,
    managed_target: Option<&Path>,
) -> Result<OpenedLegacySource> {
    let options = SqliteConnectOptions::new().filename(source).read_only(true);
    let mut connection = SqliteConnection::connect_with(&options).await?;
    let opened_path: String =
        sqlx::query_scalar("SELECT file FROM pragma_database_list WHERE name = 'main'")
            .fetch_one(&mut connection)
            .await?;
    if opened_path.is_empty() {
        return Err(sqlx::Error::Protocol(
            "legacy database source has no durable file identity".to_string(),
        ));
    }
    let opened_path = std::path::PathBuf::from(opened_path);
    let managed = managed_target
        .and_then(|target| fs::canonicalize(target).ok())
        .is_some_and(|target| target == opened_path);
    Ok(OpenedLegacySource {
        connection,
        binding: LegacySourceBinding {
            identity: source_identity(&opened_path)?,
            managed,
        },
    })
}

async fn create_legacy_snapshot_with_open_hook(
    source: &Path,
    parent: &Path,
    managed_target: Option<&Path>,
    after_open: impl FnOnce(),
) -> Result<(tempfile::TempDir, std::path::PathBuf, LegacySourceBinding)> {
    let staging = tempfile::Builder::new()
        .prefix(IMPORT_STAGING_PREFIX)
        .tempdir_in(parent)
        .map_err(sqlx::Error::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(staging.path(), fs::Permissions::from_mode(0o700))
            .map_err(sqlx::Error::Io)?;
    }
    let staged = staging.path().join("meeting_minutes.sqlite");
    let mut opened = open_legacy_source(source, managed_target).await?;
    after_open();
    sqlx::query("VACUUM INTO ?")
        .bind(staged.to_string_lossy().as_ref())
        .execute(&mut opened.connection)
        .await?;
    opened.connection.close().await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o600)).map_err(sqlx::Error::Io)?;
    }
    #[cfg(not(unix))]
    fs::set_permissions(
        &staged,
        fs::metadata(source).map_err(sqlx::Error::Io)?.permissions(),
    )
    .map_err(sqlx::Error::Io)?;
    fs::File::open(&staged)
        .and_then(|file| file.sync_all())
        .map_err(sqlx::Error::Io)?;

    Ok((staging, staged, opened.binding))
}

async fn create_legacy_snapshot(
    source: &Path,
    parent: &Path,
) -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let (staging, staged, _) =
        create_legacy_snapshot_with_open_hook(source, parent, None, || {}).await?;
    Ok((staging, staged))
}

async fn copy_legacy_database_unlocked(source: &Path, target: &Path, parent: &Path) -> Result<()> {
    let (_staging, staged) = create_legacy_snapshot(source, parent).await?;
    fs::rename(&staged, target).map_err(sqlx::Error::Io)?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(sqlx::Error::Io)?;

    Ok(())
}

async fn copy_bound_legacy_database_with_open_hook(
    source: &Path,
    target: &Path,
    parent: &Path,
    after_open: impl FnOnce(),
) -> Result<LegacySourceBinding> {
    let (_staging, staged, opened) =
        create_legacy_snapshot_with_open_hook(source, parent, Some(target), after_open).await?;
    fs::rename(&staged, target).map_err(sqlx::Error::Io)?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(sqlx::Error::Io)?;
    Ok(opened)
}

fn database_digest(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(sqlx::Error::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(sqlx::Error::Io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn source_identity(path: &Path) -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(path).map_err(sqlx::Error::Io)?;
        Ok(format!("unix-{}-{}", metadata.dev(), metadata.ino()))
    }
    #[cfg(not(unix))]
    {
        let canonical = fs::canonicalize(path).map_err(sqlx::Error::Io)?;
        return Ok(format!(
            "path-{:x}",
            Sha256::digest(canonical.to_string_lossy().as_bytes())
        ));
    }
}

fn selected_source_binding(source: &Path, managed_target: &Path) -> Result<LegacySourceBinding> {
    let source_path = fs::canonicalize(source).map_err(sqlx::Error::Io)?;
    let managed = fs::canonicalize(managed_target).is_ok_and(|target| target == source_path);
    Ok(LegacySourceBinding {
        identity: source_identity(&source_path)?,
        managed,
    })
}

fn read_import_reservation(path: &Path) -> Result<Option<ImportReservation>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(sqlx::Error::Io(error)),
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(sqlx::Error::Protocol(
            "legacy database import reservation is invalid".to_string(),
        ));
    }
    let value = fs::read_to_string(path)
        .map_err(sqlx::Error::Io)?
        .trim()
        .to_string();
    let mut fields = value.split(':');
    let source_kind = fields.next().ok_or_else(|| {
        sqlx::Error::Protocol("legacy database import reservation is invalid".to_string())
    })?;
    let identity = fields.next().ok_or_else(|| {
        sqlx::Error::Protocol("legacy database import reservation is invalid".to_string())
    })?;
    let digest = fields.next().ok_or_else(|| {
        sqlx::Error::Protocol("legacy database import reservation is invalid".to_string())
    })?;
    if fields.next().is_some() {
        return Err(sqlx::Error::Protocol(
            "legacy database import reservation is invalid".to_string(),
        ));
    }
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(sqlx::Error::Protocol(
            "legacy database import reservation is invalid".to_string(),
        ));
    }
    let source_identity = match source_kind {
        "managed" if identity.is_empty() => None,
        "external"
            if !identity.is_empty()
                && identity.len() <= 128
                && identity
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') =>
        {
            Some(identity.to_string())
        }
        _ => {
            return Err(sqlx::Error::Protocol(
                "legacy database import reservation is invalid".to_string(),
            ));
        }
    };
    Ok(Some(ImportReservation {
        digest: digest.to_string(),
        source_identity,
    }))
}

fn write_import_reservation(parent: &Path, reservation: &ImportReservation) -> Result<()> {
    let mut staging = tempfile::Builder::new()
        .prefix(IMPORT_STAGING_PREFIX)
        .tempfile_in(parent)
        .map_err(sqlx::Error::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(sqlx::Error::Io)?;
    }
    let (source_kind, identity) = match reservation.source_identity.as_deref() {
        Some(identity) => ("external", identity),
        None => ("managed", ""),
    };
    staging
        .write_all(format!("{source_kind}:{identity}:{}", reservation.digest).as_bytes())
        .and_then(|_| staging.as_file().sync_all())
        .map_err(sqlx::Error::Io)?;
    staging
        .persist(parent.join(IMPORT_RESERVATION_FILE))
        .map_err(|error| sqlx::Error::Io(error.error))?;
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(sqlx::Error::Io)?;
    Ok(())
}

fn remove_import_reservation(parent: &Path) -> Result<()> {
    let reservation = parent.join(IMPORT_RESERVATION_FILE);
    match fs::symlink_metadata(&reservation) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            return Err(sqlx::Error::Protocol(
                "legacy database import reservation is invalid".to_string(),
            ));
        }
        Ok(_) => fs::remove_file(&reservation).map_err(sqlx::Error::Io)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(sqlx::Error::Io(error)),
    }
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(sqlx::Error::Io)?;
    Ok(())
}

#[derive(Clone)]
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub async fn new(tauri_db_path: &str, backend_db_path: &str) -> Result<Self> {
        let _import_guard = IMPORT_GATE.lock().await;
        Self::new_paths_unlocked(Path::new(tauri_db_path), Path::new(backend_db_path)).await
    }

    async fn new_paths_unlocked(tauri_db_path: &Path, backend_db_path: &Path) -> Result<Self> {
        if let Some(parent_dir) = tauri_db_path.parent() {
            if !parent_dir.exists() {
                fs::create_dir_all(parent_dir).map_err(sqlx::Error::Io)?;
            }
        }

        let parent = tauri_db_path.parent().ok_or_else(|| {
            sqlx::Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "database target has no parent directory",
            ))
        })?;
        remove_stale_import_snapshots(parent)?;
        if !tauri_db_path.exists() {
            if backend_db_path.exists() {
                log::info!(
                    "Copying database from {} to {}",
                    backend_db_path.display(),
                    tauri_db_path.display()
                );
                copy_legacy_database_unlocked(backend_db_path, tauri_db_path, parent).await?;
            } else {
                log::info!("Creating database at {}", tauri_db_path.display());
                Sqlite::create_database(tauri_db_path.to_string_lossy().as_ref()).await?;
            }
        }

        Self::open_path_unlocked(tauri_db_path).await
    }

    async fn open_path_unlocked(database_path: &Path) -> Result<Self> {
        let pool = SqlitePool::connect(database_path.to_string_lossy().as_ref()).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(DatabaseManager { pool })
    }

    // NOTE: So for the first time users they needs to start the application
    // after they can just delete the existing .sqlite file and then copy the existing .db file to
    // the current app dir, So the system detects legacy db and copy it and starts with that data
    // (Newly created .sqlite with the copied content from .db)
    pub async fn new_from_app_handle(app_handle: &tauri::AppHandle) -> Result<Self> {
        // Resolve the app's data directory
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");
        if !app_data_dir.exists() {
            fs::create_dir_all(&app_data_dir).map_err(sqlx::Error::Io)?;
        }

        // Define database paths
        let tauri_db_path = app_data_dir
            .join("meeting_minutes.sqlite")
            .to_string_lossy()
            .to_string();
        // Legacy backend DB path (for auto-migration if exists)
        let backend_db_path = app_data_dir
            .join("meeting_minutes.db")
            .to_string_lossy()
            .to_string();

        // WAL file paths for defensive cleanup
        let wal_path = app_data_dir.join("meeting_minutes.sqlite-wal");
        let shm_path = app_data_dir.join("meeting_minutes.sqlite-shm");

        log::info!("Tauri DB path: {}", tauri_db_path);
        log::info!("Legacy backend DB path: {}", backend_db_path);

        // Try to open database with defensive WAL handling
        match Self::new(&tauri_db_path, &backend_db_path).await {
            Ok(db_manager) => {
                log::info!("Database opened successfully");
                Ok(db_manager)
            }
            Err(e) => {
                // Check if error is due to corrupted WAL file
                let error_msg = e.to_string();
                if error_msg.contains("malformed") || error_msg.contains("corrupt") {
                    log::warn!("Database appears corrupted, likely due to orphaned WAL file. Attempting recovery...");
                    log::warn!("Error details: {}", error_msg);

                    // Delete potentially corrupted WAL/SHM files
                    if wal_path.exists() {
                        match fs::remove_file(&wal_path) {
                            Ok(_) => log::info!("Removed orphaned WAL file: {:?}", wal_path),
                            Err(e) => log::warn!("Failed to remove WAL file: {}", e),
                        }
                    }
                    if shm_path.exists() {
                        match fs::remove_file(&shm_path) {
                            Ok(_) => log::info!("Removed orphaned SHM file: {:?}", shm_path),
                            Err(e) => log::warn!("Failed to remove SHM file: {}", e),
                        }
                    }

                    // Retry connection without WAL files
                    log::info!("Retrying database connection after WAL cleanup...");
                    match Self::new(&tauri_db_path, &backend_db_path).await {
                        Ok(db_manager) => {
                            log::info!("Database opened successfully after WAL recovery");
                            Ok(db_manager)
                        }
                        Err(retry_err) => {
                            log::error!(
                                "Database connection failed even after WAL cleanup: {}",
                                retry_err
                            );
                            Err(retry_err)
                        }
                    }
                } else {
                    // Not a WAL-related error, propagate original error
                    log::error!("Database connection failed: {}", error_msg);
                    Err(e)
                }
            }
        }
    }

    pub async fn create_fresh<R: tauri::Runtime>(app_handle: &tauri::AppHandle<R>) -> Result<Self> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");
        Self::create_fresh_paths(&app_data_dir).await
    }

    async fn create_fresh_paths(app_data_dir: &Path) -> Result<Self> {
        let _import_guard = IMPORT_GATE.lock().await;
        let active_path = app_data_dir.join("meeting_minutes.sqlite");
        let retained_path = app_data_dir.join("meeting_minutes.db");
        if filesystem_entry_exists(&active_path)?
            || filesystem_entry_exists(&retained_path)?
            || read_import_reservation(&app_data_dir.join(IMPORT_RESERVATION_FILE))?.is_some()
        {
            return Err(sqlx::Error::Protocol(
                "database initialization is already active or complete".to_string(),
            ));
        }

        fs::create_dir_all(app_data_dir).map_err(sqlx::Error::Io)?;
        remove_stale_import_snapshots(app_data_dir)?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&active_path)
            .map_err(sqlx::Error::Io)?;
        file.sync_all().map_err(sqlx::Error::Io)?;
        drop(file);
        Self::open_path_unlocked(&active_path).await
    }

    /// Check if this is the first launch (sqlite database doesn't exist yet)
    pub async fn is_first_launch(app_handle: &tauri::AppHandle) -> Result<bool> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");

        let tauri_db_path = app_data_dir.join("meeting_minutes.sqlite");

        Ok(!tauri_db_path.exists())
    }

    /// Import a legacy database from the specified path and initialize
    pub async fn import_legacy_database(
        app_handle: &tauri::AppHandle,
        legacy_db_path: &str,
    ) -> Result<Self> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");

        Self::import_legacy_database_paths(&app_data_dir, Path::new(legacy_db_path)).await
    }

    async fn import_legacy_database_paths(
        app_data_dir: &Path,
        legacy_db_path: &Path,
    ) -> Result<Self> {
        Self::import_legacy_database_paths_with_open_hook(app_data_dir, legacy_db_path, || {}).await
    }

    async fn import_legacy_database_paths_with_open_hook(
        app_data_dir: &Path,
        legacy_db_path: &Path,
        after_open: impl FnOnce(),
    ) -> Result<Self> {
        let _import_guard = IMPORT_GATE.lock().await;

        if !app_data_dir.exists() {
            fs::create_dir_all(app_data_dir).map_err(sqlx::Error::Io)?;
        }

        let active_path = app_data_dir.join("meeting_minutes.sqlite");
        let target_legacy_path = app_data_dir.join("meeting_minutes.db");
        let parent = active_path.parent().ok_or_else(|| {
            sqlx::Error::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "database target has no parent directory",
            ))
        })?;
        let reservation_path = parent.join(IMPORT_RESERVATION_FILE);
        if let Some(reservation) = read_import_reservation(&reservation_path)? {
            let selected = selected_source_binding(legacy_db_path, &target_legacy_path)?;
            let source_matches = match reservation.source_identity.as_deref() {
                Some(expected) => !selected.managed && selected.identity == expected,
                None => selected.managed,
            };
            if !source_matches {
                return Err(sqlx::Error::Protocol(
                    "another legacy database import is already pending".to_string(),
                ));
            }
            remove_stale_import_snapshots(parent)?;
            if active_path.exists() {
                let manager = Self::open_path_unlocked(&active_path).await?;
                let purge_pending: bool = sqlx::query_scalar(
                    "SELECT legacy_secret_purge_pending FROM provider_security_state WHERE id = 1",
                )
                .fetch_one(manager.pool())
                .await?;
                if !purge_pending {
                    manager.pool().close().await;
                    return Err(sqlx::Error::Protocol(
                        "database import is no longer available after initialization".to_string(),
                    ));
                }
                return Ok(manager);
            }

            if !target_legacy_path.exists()
                || database_digest(&target_legacy_path)? != reservation.digest
            {
                return Err(sqlx::Error::Protocol(
                    "another legacy database import is already pending".to_string(),
                ));
            }
            return Self::new_paths_unlocked(&active_path, &target_legacy_path).await;
        }

        if active_path.exists() {
            return Err(sqlx::Error::Protocol(
                "database import is no longer available after initialization".to_string(),
            ));
        }
        remove_stale_import_snapshots(parent)?;
        log::info!(
            "Copying legacy database into managed storage at {}",
            target_legacy_path.display()
        );
        let opened = copy_bound_legacy_database_with_open_hook(
            legacy_db_path,
            &target_legacy_path,
            parent,
            after_open,
        )
        .await?;
        write_import_reservation(
            parent,
            &ImportReservation {
                digest: database_digest(&target_legacy_path)?,
                source_identity: (!opened.managed).then_some(opened.identity),
            },
        )?;

        Self::new_paths_unlocked(&active_path, &target_legacy_path).await
    }

    pub async fn complete_legacy_import(app_handle: &tauri::AppHandle) -> Result<()> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");
        Self::complete_legacy_import_paths(&app_data_dir).await
    }

    async fn complete_legacy_import_paths(app_data_dir: &Path) -> Result<()> {
        let _import_guard = IMPORT_GATE.lock().await;
        let reservation = app_data_dir.join(IMPORT_RESERVATION_FILE);
        if read_import_reservation(&reservation)?.is_none() {
            return Ok(());
        }
        let active = app_data_dir.join("meeting_minutes.sqlite");
        let manager = Self::open_path_unlocked(&active).await?;
        let purge_pending: bool = sqlx::query_scalar(
            "SELECT legacy_secret_purge_pending FROM provider_security_state WHERE id = 1",
        )
        .fetch_one(manager.pool())
        .await?;
        manager.pool().close().await;
        if purge_pending {
            return Err(sqlx::Error::Protocol(
                "legacy database import credential migration is still pending".to_string(),
            ));
        }
        remove_import_reservation(app_data_dir)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn with_transaction<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Transaction<'_, Sqlite>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut tx = self.pool.begin().await?;
        let result = f(&mut tx).await;

        match result {
            Ok(val) => {
                tx.commit().await?;
                Ok(val)
            }
            Err(err) => {
                tx.rollback().await?;
                Err(err)
            }
        }
    }

    /// Cleanup database connection and checkpoint WAL
    /// This should be called on application shutdown to ensure:
    /// - All WAL changes are written to the main database file
    /// - The .wal and .shm files are deleted
    /// - Connection pool is gracefully closed
    pub async fn cleanup(&self) -> Result<()> {
        log::info!("Starting database cleanup...");

        // Force checkpoint of WAL to main database file and remove WAL file
        // TRUNCATE mode: checkpoints all pages AND deletes the WAL file
        match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
        {
            Ok(_) => log::info!("WAL checkpoint completed successfully"),
            Err(e) => log::warn!("WAL checkpoint failed (non-fatal): {}", e),
        }

        // Close the connection pool gracefully
        self.pool.close().await;
        log::info!("Database connection pool closed");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::repository::ProviderRepository;
    use crate::providers::{CredentialStore, ProviderError, SecretString};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryCredentials(Mutex<HashMap<String, Vec<u8>>>);

    struct UnavailableCredentials;

    impl CredentialStore for UnavailableCredentials {
        fn save(&self, _account: &str, _secret: &str) -> std::result::Result<(), ProviderError> {
            Err(ProviderError::Storage)
        }

        fn read(&self, _account: &str) -> std::result::Result<Option<SecretString>, ProviderError> {
            Ok(None)
        }

        fn remove(&self, _account: &str) -> std::result::Result<(), ProviderError> {
            Ok(())
        }
    }

    impl CredentialStore for MemoryCredentials {
        fn save(&self, account: &str, secret: &str) -> std::result::Result<(), ProviderError> {
            self.0
                .lock()
                .unwrap()
                .insert(account.to_string(), secret.as_bytes().to_vec());
            Ok(())
        }

        fn read(&self, account: &str) -> std::result::Result<Option<SecretString>, ProviderError> {
            Ok(self.0.lock().unwrap().get(account).map(SecretString::new))
        }

        fn remove(&self, account: &str) -> std::result::Result<(), ProviderError> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    async fn create_legacy_database(path: &Path, secret: &str) -> Vec<u8> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, geminiApiKey) VALUES ('1', 'gemini', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        fs::read(path).unwrap()
    }

    async fn migrate_copied_legacy_database(
        source: &Path,
        retained: &Path,
        active: &Path,
        secret: &str,
    ) {
        copy_legacy_database(source, retained).await.unwrap();
        let manager = DatabaseManager::new(active.to_str().unwrap(), retained.to_str().unwrap())
            .await
            .unwrap();
        let credentials = MemoryCredentials::default();
        ProviderRepository::migrate_legacy_credentials(manager.pool(), &credentials)
            .await
            .unwrap();
        assert!(credentials.read("summary:gemini").unwrap().is_some());
        manager.pool().close().await;

        for path in [active, retained] {
            let persisted = fs::read(path).unwrap();
            assert!(!persisted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    async fn stored_gemini_secret(path: &Path) -> Option<String> {
        let options = SqliteConnectOptions::new().filename(path).read_only(true);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        let secret = sqlx::query_scalar("SELECT geminiApiKey FROM settings WHERE id = '1'")
            .fetch_optional(&mut connection)
            .await
            .unwrap()
            .flatten();
        connection.close().await.unwrap();
        secret
    }

    #[tokio::test]
    async fn importing_default_legacy_database_preserves_source_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let legacy = directory.path().join("meeting_minutes.db");
        let secret = "synthetic default database secret";
        create_legacy_database(&legacy, secret).await;

        copy_legacy_database(&legacy, &legacy).await.unwrap();

        assert_eq!(stored_gemini_secret(&legacy).await.as_deref(), Some(secret));
    }

    #[tokio::test]
    async fn importing_hardlink_alias_preserves_source_bytes() {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("selected.db");
        let target = directory.path().join("meeting_minutes.db");
        let secret = "synthetic hardlink database secret";
        let original = create_legacy_database(&source, secret).await;
        fs::hard_link(&source, &target).unwrap();
        #[cfg(unix)]
        let source_inode = fs::metadata(&source).unwrap().ino();

        copy_legacy_database(&source, &target).await.unwrap();

        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(stored_gemini_secret(&target).await.as_deref(), Some(secret));
        #[cfg(unix)]
        {
            assert_eq!(fs::metadata(&source).unwrap().ino(), source_inode);
            assert_ne!(fs::metadata(&target).unwrap().ino(), source_inode);
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn importing_symlink_alias_preserves_source_bytes() {
        use std::os::unix::fs::{symlink, MetadataExt};

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("selected.db");
        let target = directory.path().join("meeting_minutes.db");
        let secret = "synthetic symlink database secret";
        let original = create_legacy_database(&source, secret).await;
        symlink(&source, &target).unwrap();
        let source_inode = fs::metadata(&source).unwrap().ino();

        copy_legacy_database(&source, &target).await.unwrap();

        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(stored_gemini_secret(&target).await.as_deref(), Some(secret));
        assert_eq!(fs::metadata(&source).unwrap().ino(), source_inode);
        assert_ne!(fs::metadata(&target).unwrap().ino(), source_inode);
    }

    #[tokio::test]
    async fn importing_external_database_copies_without_changing_source() {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("selected.db");
        let target = directory.path().join("meeting_minutes.db");
        let secret = "synthetic external database secret";
        let original = create_legacy_database(&source, secret).await;
        #[cfg(unix)]
        let source_inode = fs::metadata(&source).unwrap().ino();

        copy_legacy_database(&source, &target).await.unwrap();

        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(stored_gemini_secret(&target).await.as_deref(), Some(secret));
        #[cfg(unix)]
        {
            assert_eq!(fs::metadata(&source).unwrap().ino(), source_inode);
            assert_ne!(fs::metadata(&target).unwrap().ino(), source_inode);
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn lexical_symlink_target_is_replaced_without_touching_external_source() {
        use std::os::unix::fs::{symlink, MetadataExt};

        let directory = tempfile::tempdir().unwrap();
        let external = directory.path().join("external.db");
        let retained = directory.path().join("meeting_minutes.db");
        let secret = "synthetic lexical symlink secret";
        let original = create_legacy_database(&external, secret).await;
        symlink(&external, &retained).unwrap();
        let external_inode = fs::metadata(&external).unwrap().ino();

        copy_legacy_database(&retained, &retained).await.unwrap();

        assert_eq!(fs::read(&external).unwrap(), original);
        assert_eq!(fs::metadata(&external).unwrap().ino(), external_inode);
        assert!(!fs::symlink_metadata(&retained)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_ne!(fs::metadata(&retained).unwrap().ino(), external_inode);
        assert_eq!(
            stored_gemini_secret(&retained).await.as_deref(),
            Some(secret)
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn lexical_hardlink_target_is_replaced_without_touching_external_source() {
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let external = directory.path().join("external.db");
        let retained = directory.path().join("meeting_minutes.db");
        let secret = "synthetic lexical hardlink secret";
        let original = create_legacy_database(&external, secret).await;
        fs::hard_link(&external, &retained).unwrap();
        let external_inode = fs::metadata(&external).unwrap().ino();

        copy_legacy_database(&retained, &retained).await.unwrap();

        assert_eq!(fs::read(&external).unwrap(), original);
        assert_eq!(fs::metadata(&external).unwrap().ino(), external_inode);
        assert_ne!(fs::metadata(&retained).unwrap().ino(), external_inode);
        assert_eq!(
            stored_gemini_secret(&retained).await.as_deref(),
            Some(secret)
        );
    }

    #[tokio::test]
    async fn external_aliases_stay_unchanged_through_import_and_secret_migration() {
        #[cfg(unix)]
        use std::os::unix::fs::{symlink, MetadataExt};

        for alias in ["copy", "hardlink", "symlink"] {
            #[cfg(not(unix))]
            if alias == "symlink" {
                continue;
            }

            let directory = tempfile::tempdir().unwrap();
            let app_data = directory.path().join("app-data");
            fs::create_dir(&app_data).unwrap();
            let source = directory.path().join("selected.db");
            let retained = app_data.join("meeting_minutes.db");
            let active = app_data.join("meeting_minutes.sqlite");
            let secret = format!("synthetic-{alias}-migration-secret");
            let original = create_legacy_database(&source, &secret).await;
            #[cfg(unix)]
            let source_inode = fs::metadata(&source).unwrap().ino();

            match alias {
                "hardlink" => fs::hard_link(&source, &retained).unwrap(),
                #[cfg(unix)]
                "symlink" => symlink(&source, &retained).unwrap(),
                _ => {}
            }

            migrate_copied_legacy_database(&source, &retained, &active, &secret).await;

            assert_eq!(fs::read(&source).unwrap(), original);
            #[cfg(unix)]
            {
                assert_eq!(fs::metadata(&source).unwrap().ino(), source_inode);
                assert_ne!(fs::metadata(&retained).unwrap().ino(), source_inode);
            }
        }
    }

    #[tokio::test]
    async fn lexical_default_source_is_preserved_until_its_successful_secret_migration() {
        let directory = tempfile::tempdir().unwrap();
        let retained = directory.path().join("meeting_minutes.db");
        let active = directory.path().join("meeting_minutes.sqlite");
        let secret = "synthetic-default-migration-secret";
        create_legacy_database(&retained, secret).await;

        copy_legacy_database(&retained, &retained).await.unwrap();
        assert_eq!(
            stored_gemini_secret(&retained).await.as_deref(),
            Some(secret)
        );

        migrate_copied_legacy_database(&retained, &retained, &active, secret).await;
    }

    #[tokio::test]
    async fn import_snapshot_includes_committed_wal_credentials_and_data() {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let directory = tempfile::tempdir().unwrap();
        let external = directory.path().join("selected.db");
        let retained = directory.path().join("meeting_minutes.db");
        let active = directory.path().join("meeting_minutes.sqlite");
        let secret = "synthetic-wal-only-migration-secret";
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&external)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("CREATE TABLE wal_import_probe (value TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_autocheckpoint = 0")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel, geminiApiKey) VALUES ('1', 'gemini', 'fixture', 'fixture', ?)")
            .bind(secret)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO wal_import_probe (value) VALUES ('newest committed data')")
            .execute(&pool)
            .await
            .unwrap();
        assert!(external.with_extension("db-wal").exists());
        #[cfg(unix)]
        let external_inode = fs::metadata(&external).unwrap().ino();

        copy_legacy_database(&external, &retained).await.unwrap();
        let manager = DatabaseManager::new(active.to_str().unwrap(), retained.to_str().unwrap())
            .await
            .unwrap();
        let credentials = MemoryCredentials::default();
        ProviderRepository::migrate_legacy_credentials(manager.pool(), &credentials)
            .await
            .unwrap();

        assert!(credentials.read("summary:gemini").unwrap().is_some());
        let imported: String = sqlx::query_scalar("SELECT value FROM wal_import_probe LIMIT 1")
            .fetch_one(manager.pool())
            .await
            .unwrap();
        assert_eq!(imported, "newest committed data");
        #[cfg(unix)]
        assert_eq!(fs::metadata(&external).unwrap().ino(), external_inode);
        let source_secret: Option<String> =
            sqlx::query_scalar("SELECT geminiApiKey FROM settings WHERE id = '1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(source_secret.as_deref(), Some(secret));
        pool.close().await;
        manager.pool().close().await;
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn restart_removes_interrupted_private_snapshots_without_following_symlinks() {
        use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        fs::create_dir(&app_data).unwrap();
        let external = directory.path().join("selected.db");
        let retained = app_data.join("meeting_minutes.db");
        let active = app_data.join("meeting_minutes.sqlite");
        let secret = "synthetic-restart-migration-secret";
        create_legacy_database(&external, secret).await;

        let stale = app_data.join(".gcrdings-import-interrupted");
        fs::create_dir(&stale).unwrap();
        fs::write(
            stale.join("meeting_minutes.sqlite"),
            b"synthetic-interrupted-plaintext-secret",
        )
        .unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let outside_sentinel = outside.join("must-survive");
        fs::write(&outside_sentinel, b"outside").unwrap();
        let stale_symlink = app_data.join(".gcrdings-import-symlink");
        symlink(&outside, &stale_symlink).unwrap();
        let external_inode = fs::metadata(&external).unwrap().ino();

        migrate_copied_legacy_database(&external, &retained, &active, secret).await;

        assert!(fs::symlink_metadata(&stale).is_err());
        assert!(fs::symlink_metadata(&stale_symlink).is_err());
        assert!(outside_sentinel.exists());
        assert_eq!(fs::metadata(&external).unwrap().ino(), external_inode);
        assert_eq!(
            fs::metadata(&retained).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!fs::read(&active)
            .unwrap()
            .windows("synthetic-interrupted-plaintext-secret".len())
            .any(|window| window == b"synthetic-interrupted-plaintext-secret"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn ordinary_fresh_startup_removes_interrupted_snapshot_without_new_import() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let active = directory.path().join("meeting_minutes.sqlite");
        let retained = directory.path().join("meeting_minutes.db");
        let stale = directory.path().join(".gcrdings-import-interrupted");
        fs::create_dir(&stale).unwrap();
        fs::write(
            stale.join("meeting_minutes.sqlite"),
            b"synthetic-fresh-startup-plaintext-secret",
        )
        .unwrap();
        let outside = directory.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let sentinel = outside.join("must-survive");
        fs::write(&sentinel, b"outside").unwrap();
        let stale_symlink = directory.path().join(".gcrdings-import-symlink");
        symlink(&outside, &stale_symlink).unwrap();

        let manager = DatabaseManager::new(active.to_str().unwrap(), retained.to_str().unwrap())
            .await
            .unwrap();

        assert!(fs::symlink_metadata(&stale).is_err());
        assert!(fs::symlink_metadata(&stale_symlink).is_err());
        assert!(sentinel.exists());
        assert!(active.exists());
        assert!(!retained.exists());
        manager.pool().close().await;
    }

    #[tokio::test]
    async fn concurrent_imports_accept_only_one_source_and_keep_its_database_bound() {
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        let source_a = directory.path().join("source-a.db");
        let source_b = directory.path().join("source-b.db");
        let secret_a = "synthetic-concurrent-import-a";
        let secret_b = "synthetic-concurrent-import-b";
        create_legacy_database(&source_a, secret_a).await;
        create_legacy_database(&source_b, secret_b).await;

        let (result_a, result_b) = tokio::join!(
            DatabaseManager::import_legacy_database_paths(&app_data, &source_a),
            DatabaseManager::import_legacy_database_paths(&app_data, &source_b),
        );

        assert_ne!(result_a.is_ok(), result_b.is_ok());
        let (winner, expected_secret) = match (result_a, result_b) {
            (Ok(manager), Err(_)) => (manager, secret_a),
            (Err(_), Ok(manager)) => (manager, secret_b),
            _ => unreachable!(),
        };
        let active_secret: Option<String> =
            sqlx::query_scalar("SELECT geminiApiKey FROM settings WHERE id = '1'")
                .fetch_one(winner.pool())
                .await
                .unwrap();
        assert_eq!(active_secret.as_deref(), Some(expected_secret));
        let retained = app_data.join("meeting_minutes.db");
        assert_eq!(
            stored_gemini_secret(&retained).await.as_deref(),
            Some(expected_secret)
        );
        winner.pool().close().await;
    }

    #[tokio::test]
    async fn keychain_failure_allows_same_source_retry_but_rejects_another_source() {
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        let source = directory.path().join("source.db");
        let other = directory.path().join("other.db");
        let secret = "synthetic-keychain-retry-secret";
        create_legacy_database(&source, secret).await;
        create_legacy_database(&other, "synthetic-other-import-secret").await;
        let first = DatabaseManager::import_legacy_database_paths(&app_data, &source)
            .await
            .unwrap();

        let failed =
            ProviderRepository::migrate_legacy_credentials(first.pool(), &UnavailableCredentials)
                .await;
        assert_eq!(failed, Err(ProviderError::Storage));
        first.pool().close().await;
        assert!(DatabaseManager::complete_legacy_import_paths(&app_data)
            .await
            .is_err());
        assert!(DatabaseManager::create_fresh_paths(&app_data)
            .await
            .is_err());
        let active = app_data.join("meeting_minutes.sqlite");
        let retained = app_data.join("meeting_minutes.db");
        let reservation = app_data.join(IMPORT_RESERVATION_FILE);
        let before_rejected_import = [
            fs::read(&active).unwrap(),
            fs::read(&retained).unwrap(),
            fs::read(&reservation).unwrap(),
        ];
        assert!(
            DatabaseManager::import_legacy_database_paths(&app_data, &other)
                .await
                .is_err()
        );
        assert_eq!(
            before_rejected_import,
            [
                fs::read(&active).unwrap(),
                fs::read(&retained).unwrap(),
                fs::read(&reservation).unwrap(),
            ]
        );

        let retry = DatabaseManager::import_legacy_database_paths(&app_data, &source)
            .await
            .unwrap();
        let credentials = MemoryCredentials::default();
        ProviderRepository::migrate_legacy_credentials(retry.pool(), &credentials)
            .await
            .unwrap();
        DatabaseManager::complete_legacy_import_paths(&app_data)
            .await
            .unwrap();
        assert!(credentials.read("summary:gemini").unwrap().is_some());
        retry.pool().close().await;
        for path in [
            app_data.join("meeting_minutes.sqlite"),
            app_data.join("meeting_minutes.db"),
        ] {
            let persisted = fs::read(path).unwrap();
            assert!(!persisted
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()));
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn symlink_switch_after_open_cannot_substitute_the_reserved_source() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        fs::create_dir(&app_data).unwrap();
        let source_a = directory.path().join("source-a.db");
        let source_b = directory.path().join("source-b.db");
        let alias = directory.path().join("selected.db");
        let secret_a = "synthetic-open-source-a";
        let secret_b = "synthetic-open-source-b";
        create_legacy_database(&source_a, secret_a).await;
        create_legacy_database(&source_b, secret_b).await;
        symlink(&source_a, &alias).unwrap();

        let first =
            DatabaseManager::import_legacy_database_paths_with_open_hook(&app_data, &alias, || {
                fs::remove_file(&alias).unwrap();
                symlink(&source_b, &alias).unwrap();
            })
            .await
            .unwrap();

        assert_eq!(
            stored_gemini_secret(&app_data.join("meeting_minutes.db"))
                .await
                .as_deref(),
            Some(secret_a)
        );
        assert_ne!(
            stored_gemini_secret(&app_data.join("meeting_minutes.db"))
                .await
                .as_deref(),
            Some(secret_b)
        );
        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(first.pool(), &UnavailableCredentials,)
                .await,
            Err(ProviderError::Storage)
        );
        first.pool().close().await;
        assert!(
            DatabaseManager::import_legacy_database_paths(&app_data, &alias)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn symlink_to_managed_source_retries_after_keychain_failure() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        fs::create_dir(&app_data).unwrap();
        let retained = app_data.join("meeting_minutes.db");
        let alias = directory.path().join("selected.db");
        let secret = "synthetic-managed-symlink-retry";
        create_legacy_database(&retained, secret).await;
        symlink(&retained, &alias).unwrap();

        let first = DatabaseManager::import_legacy_database_paths(&app_data, &alias)
            .await
            .unwrap();
        assert_eq!(
            ProviderRepository::migrate_legacy_credentials(first.pool(), &UnavailableCredentials,)
                .await,
            Err(ProviderError::Storage)
        );
        first.pool().close().await;

        let retry = DatabaseManager::import_legacy_database_paths(&app_data, &alias)
            .await
            .unwrap();
        assert_eq!(
            stored_gemini_secret(&retained).await.as_deref(),
            Some(secret)
        );
        retry.pool().close().await;
    }

    #[tokio::test]
    async fn fresh_initialization_rejects_an_existing_database_without_writes() {
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        fs::create_dir(&app_data).unwrap();
        let active = app_data.join("meeting_minutes.sqlite");
        let retained = app_data.join("meeting_minutes.db");
        let manager = DatabaseManager::new(active.to_str().unwrap(), retained.to_str().unwrap())
            .await
            .unwrap();
        sqlx::query("INSERT INTO settings (id, provider, model, whisperModel) VALUES ('1', 'custom-existing', 'custom-model', 'custom-whisper')")
            .execute(manager.pool())
            .await
            .unwrap();
        manager.cleanup().await.unwrap();
        let before = fs::read(&active).unwrap();

        assert!(DatabaseManager::create_fresh_paths(&app_data)
            .await
            .is_err());
        assert_eq!(fs::read(&active).unwrap(), before);
        let options = SqliteConnectOptions::new()
            .filename(&active)
            .read_only(true);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        let configuration: (String, String, String) =
            sqlx::query_as("SELECT provider, model, whisperModel FROM settings WHERE id = '1'")
                .fetch_one(&mut connection)
                .await
                .unwrap();
        connection.close().await.unwrap();
        assert_eq!(
            configuration,
            (
                "custom-existing".to_string(),
                "custom-model".to_string(),
                "custom-whisper".to_string(),
            )
        );
    }

    #[tokio::test]
    async fn concurrent_fresh_initialization_has_exactly_one_winner() {
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");

        let (first, second) = tokio::join!(
            DatabaseManager::create_fresh_paths(&app_data),
            DatabaseManager::create_fresh_paths(&app_data),
        );

        assert_ne!(first.is_ok(), second.is_ok());
        if let Ok(manager) = first {
            manager.pool().close().await;
        }
        if let Ok(manager) = second {
            manager.pool().close().await;
        }
    }

    #[tokio::test]
    async fn fresh_initialization_rejects_a_broken_active_symlink_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        fs::create_dir(&app_data).unwrap();
        let outside = directory.path().join("outside.sqlite");
        let active = app_data.join("meeting_minutes.sqlite");
        symlink(&outside, &active).unwrap();

        assert!(DatabaseManager::create_fresh_paths(&app_data)
            .await
            .is_err());
        assert!(fs::symlink_metadata(&active)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!outside.exists());
    }
}
