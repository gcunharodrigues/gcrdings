use crate::database::models::Participant;
use crate::state::AppState;
use crate::verifiable_record::models::{
    GeneratedRecord, GenerationStatus, RecordBlock, RecordType, VerifiableRecord,
};
use crate::verifiable_record::repository::VerifiableRecordRepository;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHandoffFormat {
    Json,
    Markdown,
}

impl AgentHandoffFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHandoffAction {
    Save,
    Share,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveOutcome {
    Cancelled,
    Saved,
    SharePresented,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AgentHandoffError {
    #[error("Agent Handoff record is unavailable")]
    SnapshotUnavailable,
    #[error("Agent Handoff serialization failed")]
    Serialization,
    #[error("Agent Handoff destination is unsafe")]
    UnsafeDestination,
    #[error("Agent Handoff could not be written")]
    WriteFailed,
    #[error("Agent Handoff sharing is unavailable")]
    ShareUnavailable,
    #[error("Agent Handoff could not be shared")]
    ShareFailed,
}

#[derive(Serialize)]
struct AgentHandoffSnapshot<'a> {
    schema_version: u32,
    session: SessionSnapshot<'a>,
    participants: &'a [Participant],
    generation: GenerationSnapshot<'a>,
    transcript: &'a [RecordBlock],
}

#[derive(Serialize)]
struct SessionSnapshot<'a> {
    id: &'a str,
    title: &'a str,
    record_type: RecordType,
    principal_transcript_revision: i64,
}

#[derive(Serialize)]
struct GenerationSnapshot<'a> {
    status: &'a GenerationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<&'a GeneratedRecord>,
}

impl<'a> From<&'a VerifiableRecord> for AgentHandoffSnapshot<'a> {
    fn from(record: &'a VerifiableRecord) -> Self {
        Self {
            schema_version: record.schema_version,
            session: SessionSnapshot {
                id: &record.meeting_id,
                title: &record.title,
                record_type: record.record_type,
                principal_transcript_revision: record.principal_transcript_revision,
            },
            participants: &record.participants,
            generation: GenerationSnapshot {
                status: &record.generation_status,
                record: record.generated.as_ref(),
            },
            transcript: &record.transcript,
        }
    }
}

pub fn serialize_json(record: &VerifiableRecord) -> Result<String, AgentHandoffError> {
    serde_json::to_string_pretty(&AgentHandoffSnapshot::from(record))
        .map_err(|_| AgentHandoffError::Serialization)
}

pub fn serialize_markdown(record: &VerifiableRecord) -> Result<String, AgentHandoffError> {
    let json = serialize_json(record)?;
    Ok(format!(
        "# Agent Handoff: {}\n\nType: {}  \nTranscript revision: {}\n\n```agent-handoff+json\n{}\n```\n",
        record.title,
        record.record_type.as_str(),
        record.principal_transcript_revision,
        json
    ))
}

fn save_selected(
    destination: Option<&Path>,
    content: &[u8],
) -> Result<SaveOutcome, AgentHandoffError> {
    let Some(destination) = destination else {
        return Ok(SaveOutcome::Cancelled);
    };
    write_atomic(destination, content)?;
    Ok(SaveOutcome::Saved)
}

fn write_atomic(destination: &Path, content: &[u8]) -> Result<(), AgentHandoffError> {
    if destination.is_dir() || destination.file_name().is_none() {
        return Err(AgentHandoffError::UnsafeDestination);
    }
    let parent = destination
        .parent()
        .ok_or(AgentHandoffError::UnsafeDestination)?;
    if !parent.is_dir() {
        return Err(AgentHandoffError::WriteFailed);
    }
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(AgentHandoffError::UnsafeDestination)?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| AgentHandoffError::WriteFailed)?;
        file.write_all(content)
            .and_then(|_| file.sync_all())
            .map_err(|_| AgentHandoffError::WriteFailed)?;
        fs::rename(&temporary, destination).map_err(|_| AgentHandoffError::WriteFailed)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) async fn save_after_destination_selection(
    pool: SqlitePool,
    meeting_id: String,
    format: AgentHandoffFormat,
    selection: tokio::sync::oneshot::Receiver<Option<PathBuf>>,
) -> Result<SaveOutcome, AgentHandoffError> {
    let destination = selection
        .await
        .map_err(|_| AgentHandoffError::WriteFailed)?;
    let Some(destination) = destination else {
        return Ok(SaveOutcome::Cancelled);
    };
    let record = VerifiableRecordRepository::get(&pool, &meeting_id)
        .await
        .map_err(|_| AgentHandoffError::SnapshotUnavailable)?;
    let content = match format {
        AgentHandoffFormat::Json => serialize_json(&record)?,
        AgentHandoffFormat::Markdown => serialize_markdown(&record)?,
    };
    tauri::async_runtime::spawn_blocking(move || {
        save_selected(Some(&destination), content.as_bytes())
    })
    .await
    .map_err(|_| AgentHandoffError::WriteFailed)?
}

#[tauri::command]
pub async fn api_export_agent_handoff<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    meeting_id: String,
    format: AgentHandoffFormat,
    action: AgentHandoffAction,
) -> Result<SaveOutcome, AgentHandoffError> {
    match action {
        AgentHandoffAction::Save => {
            let (send, receive) = tokio::sync::oneshot::channel();
            app.dialog()
                .file()
                .add_filter("Agent Handoff", &[format.extension()])
                .set_file_name(format!("agent-handoff.{}", format.extension()))
                .save_file(move |selection| {
                    let _ = send.send(selection.and_then(|file| file.into_path().ok()));
                });
            save_after_destination_selection(
                state.db_manager.pool().clone(),
                meeting_id,
                format,
                receive,
            )
            .await
        }
        AgentHandoffAction::Share => {
            let record = VerifiableRecordRepository::get(state.db_manager.pool(), &meeting_id)
                .await
                .map_err(|_| AgentHandoffError::SnapshotUnavailable)?;
            let content = match format {
                AgentHandoffFormat::Json => serialize_json(&record)?,
                AgentHandoffFormat::Markdown => serialize_markdown(&record)?,
            };
            share_content(&app, content).await
        }
    }
}

#[cfg(target_os = "macos")]
async fn share_content<R: Runtime>(
    app: &AppHandle<R>,
    content: String,
) -> Result<SaveOutcome, AgentHandoffError> {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl, Encode, Encoding};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Point {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Size {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Rect {
        origin: Point,
        size: Size,
    }

    unsafe impl Encode for Rect {
        fn encode() -> Encoding {
            unsafe { Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}") }
        }
    }

    let view = app
        .get_webview_window("main")
        .ok_or(AgentHandoffError::ShareUnavailable)?
        .ns_view()
        .map_err(|_| AgentHandoffError::ShareUnavailable)? as usize;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || unsafe {
        let bytes = content.as_bytes();
        let string: *mut Object = msg_send![class!(NSString), alloc];
        let string: *mut Object = msg_send![string,
            initWithBytes: bytes.as_ptr()
            length: bytes.len()
            encoding: 4usize
        ];
        if string.is_null() {
            let _ = send.send(Err(AgentHandoffError::ShareFailed));
            return;
        }
        let items: *mut Object = msg_send![class!(NSArray), arrayWithObject: string];
        let picker: *mut Object = msg_send![class!(NSSharingServicePicker), alloc];
        let picker: *mut Object = msg_send![picker, initWithItems: items];
        if picker.is_null() {
            let _: () = msg_send![string, release];
            let _ = send.send(Err(AgentHandoffError::ShareFailed));
            return;
        }
        let view = view as *mut Object;
        let bounds: Rect = msg_send![view, bounds];
        let _: () = msg_send![picker,
            showRelativeToRect: bounds
            ofView: view
            preferredEdge: 3usize
        ];
        let _: () = msg_send![picker, release];
        let _: () = msg_send![string, release];
        let _ = send.send(Ok(SaveOutcome::SharePresented));
    })
    .map_err(|_| AgentHandoffError::ShareFailed)?;
    receive.await.map_err(|_| AgentHandoffError::ShareFailed)?
}

#[cfg(not(target_os = "macos"))]
async fn share_content<R: Runtime>(
    _app: &AppHandle<R>,
    _content: String,
) -> Result<SaveOutcome, AgentHandoffError> {
    Err(AgentHandoffError::ShareUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::Participant;
    use crate::verifiable_record::models::{
        EvidenceReference, Finding, GeneratedRecord, GenerationStatus, RecordBlock, RecordType,
    };
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::fs;

    async fn snapshot_fixture() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at, principal_transcript_revision) VALUES ('session-1', 'Session', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 7)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, display_name) VALUES ('participant-1', 'session-1', 'Alice')").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, participant_id, audio_start_time, audio_end_time) VALUES ('block-1', 'session-1', 'Old passage', '00:00:01', 'participant-1', 1.0, 2.0)").execute(&pool).await.unwrap();
        pool
    }

    fn generated(record_type: RecordType) -> GeneratedRecord {
        let finding = Finding {
            label: "Decision".into(),
            detail: "Use the corrected principal transcript".into(),
            evidence: vec![EvidenceReference {
                block_id: "block-2".into(),
                timestamp_ms: 2250,
            }],
        };
        match record_type {
            RecordType::Meeting => GeneratedRecord::Meeting {
                summary: "Meeting summary".into(),
                decisions: vec![finding],
                action_items: vec![],
                key_points: vec![],
            },
            RecordType::Interview => GeneratedRecord::Interview {
                summary: "Interview summary".into(),
                answers: vec![finding],
                themes: vec![],
                follow_ups: vec![],
            },
            RecordType::Content => GeneratedRecord::Content {
                summary: "Content summary".into(),
                claims: vec![finding],
                outline: vec![],
                source_notes: vec![],
            },
        }
    }

    fn record(record_type: RecordType) -> VerifiableRecord {
        VerifiableRecord {
            schema_version: 1,
            meeting_id: "session-1".into(),
            title: "Corrected Session".into(),
            record_type,
            principal_transcript_revision: 7,
            participants: vec![Participant {
                id: "participant-1".into(),
                display_name: "Alice".into(),
                speaker_cluster_id: Some("cluster-1".into()),
            }],
            transcript: vec![
                RecordBlock {
                    id: "block-1".into(),
                    text: "First corrected passage.".into(),
                    participant_id: "participant-1".into(),
                    start_ms: 1000,
                    end_ms: 1750,
                },
                RecordBlock {
                    id: "block-2".into(),
                    text: "Second corrected passage.".into(),
                    participant_id: "participant-1".into(),
                    start_ms: 2000,
                    end_ms: 3000,
                },
            ],
            generation_status: GenerationStatus::Completed,
            generated: Some(generated(record_type)),
            error_code: None,
        }
    }

    fn markdown_facts(markdown: &str) -> Value {
        let payload = markdown
            .split_once("```agent-handoff+json\n")
            .and_then(|(_, tail)| tail.split_once("\n```").map(|(json, _)| json))
            .expect("Markdown must carry one canonical facts block");
        serde_json::from_str(payload).unwrap()
    }

    #[test]
    fn json_and_markdown_are_semantically_equivalent_for_every_record_type() {
        for record_type in [
            RecordType::Meeting,
            RecordType::Interview,
            RecordType::Content,
        ] {
            let snapshot = record(record_type);
            let json: Value = serde_json::from_str(&serialize_json(&snapshot).unwrap()).unwrap();
            let markdown = serialize_markdown(&snapshot).unwrap();

            assert_eq!(markdown_facts(&markdown), json);
            assert_eq!(json["session"]["principal_transcript_revision"], 7);
            assert_eq!(json["transcript"].as_array().unwrap().len(), 2);
            assert_eq!(json["transcript"][0]["text"], "First corrected passage.");
            assert_eq!(json["transcript"][1]["text"], "Second corrected passage.");
        }
    }

    #[test]
    fn canonical_json_has_only_declared_fields_and_omits_private_inputs() {
        let value: Value =
            serde_json::from_str(&serialize_json(&record(RecordType::Meeting)).unwrap()).unwrap();
        let mut keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            [
                "generation",
                "participants",
                "schema_version",
                "session",
                "transcript"
            ]
        );
        assert_eq!(
            value["session"],
            json!({
                "id": "session-1",
                "title": "Corrected Session",
                "record_type": "meeting",
                "principal_transcript_revision": 7
            })
        );
        let serialized = value.to_string();
        for forbidden in [
            "audio",
            "video",
            "folder_path",
            "source_path",
            "credential",
            "keychain",
            "prompt",
            "temporary_path",
            "error_code",
        ] {
            assert!(!serialized.to_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn cancelled_destination_writes_nothing() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            save_selected(None, b"private record"),
            Ok(SaveOutcome::Cancelled)
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn atomic_write_overwrites_stale_output_and_leaves_no_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("handoff.json");
        fs::write(&destination, b"stale prior output").unwrap();

        write_atomic(&destination, b"current revision 7").unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"current revision 7");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn unsafe_or_failed_destination_leaves_no_incomplete_temporary_artifact() {
        let directory = tempfile::tempdir().unwrap();
        let unsafe_destination = directory.path().join("destination-is-a-directory");
        fs::create_dir(&unsafe_destination).unwrap();
        assert_eq!(
            write_atomic(&unsafe_destination, b"private record"),
            Err(AgentHandoffError::UnsafeDestination)
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);

        let missing_parent = directory.path().join("missing").join("handoff.md");
        assert_eq!(
            write_atomic(&missing_parent, b"private record"),
            Err(AgentHandoffError::WriteFailed)
        );
        assert!(!missing_parent.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn save_uses_the_current_revision_after_destination_selection() {
        let pool = snapshot_fixture().await;
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("handoff.json");
        fs::write(&destination, b"stale prior export").unwrap();
        let (selected, selection) = tokio::sync::oneshot::channel();
        let save = tokio::spawn(save_after_destination_selection(
            pool.clone(),
            "session-1".into(),
            AgentHandoffFormat::Json,
            selection,
        ));
        tokio::task::yield_now().await;

        sqlx::query("UPDATE meetings SET principal_transcript_revision = 8 WHERE id = 'session-1'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE transcripts SET transcript = 'Current passage' WHERE id = 'block-1'")
            .execute(&pool)
            .await
            .unwrap();
        selected.send(Some(destination.clone())).unwrap();

        assert_eq!(save.await.unwrap(), Ok(SaveOutcome::Saved));
        let exported: Value = serde_json::from_slice(&fs::read(&destination).unwrap()).unwrap();
        assert_eq!(exported["session"]["principal_transcript_revision"], 8);
        assert_eq!(exported["transcript"][0]["text"], "Current passage");
        assert!(!fs::read_to_string(&destination)
            .unwrap()
            .contains("stale prior export"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn cancelled_destination_does_not_read_or_write_a_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let (selected, selection) = tokio::sync::oneshot::channel();
        selected.send(None).unwrap();

        assert_eq!(
            save_after_destination_selection(
                pool,
                "missing-session".into(),
                AgentHandoffFormat::Json,
                selection,
            )
            .await,
            Ok(SaveOutcome::Cancelled)
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
