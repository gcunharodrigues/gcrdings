//! Importing a folder of recordings instead of one file at a time.
//!
//! Imports are serialised by ImportGuard, so a batch holds the guard once and
//! runs its files in sequence rather than acquiring per file — otherwise the
//! second file would be refused by the first one still in flight.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_dialog::DialogExt;

use super::constants::AUDIO_EXTENSIONS;
use super::import::{validate_audio_file, ImportGuard};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCandidate {
    pub path: String,
    /// File stem, offered as the Session title before import.
    pub title: String,
    pub size_bytes: u64,
    /// Why this file cannot be imported, if it cannot.
    pub rejection: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchScan {
    pub folder: String,
    pub candidates: Vec<BatchCandidate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub completed: usize,
    pub total: usize,
    pub current_title: String,
    pub failed: Vec<String>,
}

fn is_importable(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn title_from(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Imported Session")
        .to_string()
}

/// One level only. Recursing would silently pull in whatever else lives under
/// a Downloads folder, and the person picking it cannot see what they agreed to.
fn scan_folder(folder: &Path) -> Vec<BatchCandidate> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };

    let mut candidates: Vec<BatchCandidate> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_importable(path))
        .map(|path| {
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            // Validation is what tells a silent video apart from usable audio,
            // so it runs before anything is queued rather than mid-batch.
            let rejection = validate_audio_file(&path)
                .err()
                .map(|error| error.to_string());
            BatchCandidate {
                path: path.to_string_lossy().to_string(),
                title: title_from(&path),
                size_bytes,
                rejection,
            }
        })
        .collect();

    candidates.sort_by_key(|candidate| candidate.title.to_lowercase());
    candidates
}

#[tauri::command]
pub async fn api_select_import_folder<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<BatchScan>, String> {
    let app_clone = app.clone();
    let selected =
        tokio::task::spawn_blocking(move || app_clone.dialog().file().blocking_pick_folder())
            .await
            .map_err(|_| "The folder picker could not be opened".to_string())?;

    let Some(folder) = selected else {
        return Ok(None);
    };
    let folder = PathBuf::from(folder.to_string());

    let scan_target = folder.clone();
    let candidates = tokio::task::spawn_blocking(move || scan_folder(&scan_target))
        .await
        .map_err(|_| "Scanning the folder stopped unexpectedly".to_string())?;

    Ok(Some(BatchScan {
        folder: folder.to_string_lossy().to_string(),
        candidates,
    }))
}

/// Imports the given files one after another under a single guard.
///
/// A file that fails does not abort the batch: with twenty recordings queued,
/// stopping at the third would leave seventeen unimported and no clear way to
/// resume. Failures are collected and reported at the end.
#[tauri::command]
pub async fn api_start_batch_import<R: Runtime>(
    app: AppHandle<R>,
    files: Vec<BatchCandidate>,
    language: Option<String>,
    model: Option<String>,
    provider: Option<String>,
) -> Result<usize, String> {
    let importable: Vec<BatchCandidate> = files
        .into_iter()
        .filter(|candidate| candidate.rejection.is_none())
        .collect();

    if importable.is_empty() {
        return Err("None of the selected files can be imported".to_string());
    }

    let guard = ImportGuard::acquire()?;
    let total = importable.len();

    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        let mut failed: Vec<String> = Vec::new();

        for (index, candidate) in importable.into_iter().enumerate() {
            let _ = app.emit(
                "batch-import-progress",
                BatchProgress {
                    completed: index,
                    total,
                    current_title: candidate.title.clone(),
                    failed: failed.clone(),
                },
            );

            // The batch already holds the single-import guard, so this runs
            // the import directly rather than trying to acquire it again.
            let outcome = super::import::start_import(
                app.clone(),
                candidate.path.clone(),
                candidate.title.clone(),
                language.clone(),
                model.clone(),
                provider.clone(),
            )
            .await;

            if outcome.is_err() {
                failed.push(candidate.title);
            }
        }

        let _ = app.emit(
            "batch-import-complete",
            BatchProgress {
                completed: total,
                total,
                current_title: String::new(),
                failed,
            },
        );
    });

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_media_extensions_are_importable() {
        assert!(is_importable(Path::new("/tmp/a.wav")));
        assert!(is_importable(Path::new("/tmp/A.MP3")));
        assert!(!is_importable(Path::new("/tmp/notes.txt")));
        assert!(!is_importable(Path::new("/tmp/no-extension")));
    }

    #[test]
    fn titles_come_from_the_file_stem() {
        assert_eq!(
            title_from(Path::new("/tmp/Weekly Review.m4a")),
            "Weekly Review"
        );
        // A trailing slash still has a final component, so the fallback only
        // applies to paths with none at all.
        assert_eq!(title_from(Path::new("/tmp/")), "tmp");
        assert_eq!(title_from(Path::new("/")), "Imported Session");
    }
}
