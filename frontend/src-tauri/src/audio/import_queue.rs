//! A queue for media imports, so importing is not a modal that owns the app.
//!
//! Imports are serialised — one ffmpeg extraction and one transcription at a
//! time — but that is a scheduling constraint, not a reason to block the
//! window. Files queue, the worker drains them in order, and the person keeps
//! using the app while it happens.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

use super::import::ImportGuard;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedImport {
    pub id: String,
    pub path: String,
    pub title: String,
    pub language: Option<String>,
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportQueueState {
    /// The item being imported right now, if any.
    pub active: Option<QueuedImport>,
    pub pending: Vec<QueuedImport>,
    /// Titles that failed since the queue last went idle.
    pub failed: Vec<String>,
    pub completed: usize,
}

static QUEUE: Mutex<VecDeque<QueuedImport>> = Mutex::new(VecDeque::new());
static ACTIVE: Mutex<Option<QueuedImport>> = Mutex::new(None);
static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static COMPLETED: Mutex<usize> = Mutex::new(0);
static WORKER_RUNNING: AtomicBool = AtomicBool::new(false);

fn snapshot() -> ImportQueueState {
    ImportQueueState {
        active: ACTIVE.lock().ok().and_then(|guard| guard.clone()),
        pending: QUEUE
            .lock()
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default(),
        failed: FAILED.lock().map(|f| f.clone()).unwrap_or_default(),
        completed: COMPLETED.lock().map(|c| *c).unwrap_or(0),
    }
}

fn broadcast<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("import-queue-changed", snapshot());
}

/// Drains the queue until it is empty, holding the single-import guard for the
/// whole run. Acquiring per item would have the second item refused by the
/// first still in flight.
fn spawn_worker<R: Runtime>(app: AppHandle<R>) {
    // compare_exchange rather than a load-then-store: two enqueues arriving
    // together would both see "not running" and start two workers.
    if WORKER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let guard = match ImportGuard::acquire() {
            Ok(guard) => guard,
            Err(_) => {
                // A one-off import outside the queue is still running. It will
                // finish and the next enqueue restarts the worker.
                WORKER_RUNNING.store(false, Ordering::SeqCst);
                return;
            }
        };
        let _guard = guard;

        loop {
            let next = QUEUE.lock().ok().and_then(|mut queue| queue.pop_front());
            let Some(item) = next else { break };

            if let Ok(mut active) = ACTIVE.lock() {
                *active = Some(item.clone());
            }
            broadcast(&app);

            let outcome = super::import::start_import(
                app.clone(),
                item.path.clone(),
                item.title.clone(),
                item.language.clone(),
                item.model.clone(),
                item.provider.clone(),
            )
            .await;

            if outcome.is_err() {
                if let Ok(mut failed) = FAILED.lock() {
                    failed.push(item.title.clone());
                }
            } else if let Ok(mut completed) = COMPLETED.lock() {
                *completed += 1;
            }

            if let Ok(mut active) = ACTIVE.lock() {
                *active = None;
            }
            broadcast(&app);
        }

        WORKER_RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit("import-queue-drained", snapshot());
        // Counters reset only once the queue is empty, so a person who queued
        // twenty files sees one running total rather than it restarting at
        // every gap between items.
        if let Ok(mut failed) = FAILED.lock() {
            failed.clear();
        }
        if let Ok(mut completed) = COMPLETED.lock() {
            *completed = 0;
        }
    });
}

#[tauri::command]
pub async fn api_enqueue_imports<R: Runtime>(
    app: AppHandle<R>,
    items: Vec<QueuedImport>,
) -> Result<ImportQueueState, String> {
    if items.is_empty() {
        return Err("Nothing to import".to_string());
    }

    {
        let mut queue = QUEUE
            .lock()
            .map_err(|_| "The import queue is unavailable")?;
        for mut item in items {
            if item.id.is_empty() {
                item.id = uuid::Uuid::new_v4().to_string();
            }
            queue.push_back(item);
        }
    }

    broadcast(&app);
    spawn_worker(app.clone());
    Ok(snapshot())
}

#[tauri::command]
pub async fn api_get_import_queue<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<ImportQueueState, String> {
    Ok(snapshot())
}

/// Removes an item that has not started. The active import is cancelled through
/// cancel_import_command, which is a different operation with a different cost.
#[tauri::command]
pub async fn api_remove_queued_import<R: Runtime>(
    app: AppHandle<R>,
    item_id: String,
) -> Result<ImportQueueState, String> {
    {
        let mut queue = QUEUE
            .lock()
            .map_err(|_| "The import queue is unavailable")?;
        queue.retain(|item| item.id != item_id);
    }
    broadcast(&app);
    Ok(snapshot())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The queue is process-global, so tests that touch it must not interleave.
    /// Without this they race and each sees the other's items.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn item(title: &str) -> QueuedImport {
        QueuedImport {
            id: title.to_string(),
            path: format!("/tmp/{title}.wav"),
            title: title.to_string(),
            language: None,
            model: None,
            provider: None,
        }
    }

    #[test]
    fn snapshot_reports_pending_in_order() {
        let _serialised = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        QUEUE.lock().unwrap().clear();
        {
            let mut queue = QUEUE.lock().unwrap();
            queue.push_back(item("first"));
            queue.push_back(item("second"));
        }

        let state = snapshot();
        assert_eq!(
            state
                .pending
                .iter()
                .map(|i| i.title.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert!(state.active.is_none());
        QUEUE.lock().unwrap().clear();
    }

    #[test]
    fn removing_a_queued_item_leaves_the_rest_in_order() {
        let _serialised = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        QUEUE.lock().unwrap().clear();
        {
            let mut queue = QUEUE.lock().unwrap();
            queue.push_back(item("a"));
            queue.push_back(item("b"));
            queue.push_back(item("c"));
        }

        QUEUE.lock().unwrap().retain(|i| i.id != "b");

        assert_eq!(
            snapshot()
                .pending
                .iter()
                .map(|i| i.title.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        QUEUE.lock().unwrap().clear();
    }
}
