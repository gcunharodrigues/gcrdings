use crate::database::models::{Participant, Transcript};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashSet;

const MAX_PATCH_CHANGES: usize = 10_000;
const MAX_ID_BYTES: usize = 512;
const MAX_PARTICIPANT_NAME_BYTES: usize = 4_096;
const MAX_TRANSCRIPT_TEXT_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantChange {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptChange {
    pub id: String,
    pub text: Option<String>,
    pub participant_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavePrincipalRecord {
    pub meeting_id: String,
    pub expected_revision: i64,
    pub participant_changes: Vec<ParticipantChange>,
    pub transcript_changes: Vec<TranscriptChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrincipalRecordReceipt {
    #[serde(rename = "recordVersion")]
    pub principal_transcript_revision: i64,
}

#[derive(Debug, Serialize)]
pub struct PrincipalRecord {
    pub meeting_id: String,
    pub record_version: i64,
    pub participants: Vec<Participant>,
    pub transcripts: Vec<Transcript>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum PrincipalRecordError {
    #[error("principal record revision conflict")]
    RevisionConflict,
    #[error("invalid principal record change")]
    InvalidChange,
    #[error("session not found")]
    NotFound,
    #[error("principal record storage failure")]
    Storage,
}

pub struct PrincipalRecordRepository;

impl PrincipalRecordRepository {
    pub async fn get(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<PrincipalRecord, PrincipalRecordError> {
        if meeting_id.trim().is_empty() {
            return Err(PrincipalRecordError::InvalidChange);
        }

        let record_version =
            sqlx::query_scalar("SELECT principal_transcript_revision FROM meetings WHERE id = ?")
                .bind(meeting_id)
                .fetch_optional(pool)
                .await
                .map_err(|_| PrincipalRecordError::Storage)?
                .ok_or(PrincipalRecordError::NotFound)?;
        let mut participants = sqlx::query_as::<_, Participant>(
            "SELECT id, display_name, speaker_cluster_id FROM participants WHERE meeting_id = ? ORDER BY id",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .map_err(|_| PrincipalRecordError::Storage)?;
        let mut transcripts = sqlx::query_as::<_, Transcript>(
            "SELECT * FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time ASC, audio_end_time ASC, source_origin ASC, id ASC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .map_err(|_| PrincipalRecordError::Storage)?;

        let participant_ids: HashSet<_> = participants
            .iter()
            .map(|participant| participant.id.clone())
            .collect();
        if transcripts.iter().any(|transcript| {
            transcript
                .participant_id
                .as_ref()
                .is_none_or(|id| !participant_ids.contains(id))
        }) {
            let fallback_id = format!("{meeting_id}:participant:unknown");
            if !participant_ids.contains(&fallback_id) {
                participants.push(Participant {
                    id: fallback_id.clone(),
                    display_name: "Unknown participant".into(),
                    speaker_cluster_id: None,
                });
            }
            for transcript in &mut transcripts {
                if transcript
                    .participant_id
                    .as_ref()
                    .is_none_or(|id| !participant_ids.contains(id))
                {
                    transcript.participant_id = Some(fallback_id.clone());
                }
            }
        }

        Ok(PrincipalRecord {
            meeting_id: meeting_id.to_owned(),
            record_version,
            participants,
            transcripts,
        })
    }

    pub async fn save(
        pool: &SqlitePool,
        request: &SavePrincipalRecord,
    ) -> Result<PrincipalRecordReceipt, PrincipalRecordError> {
        if request.meeting_id.trim().is_empty()
            || request.meeting_id.len() > MAX_ID_BYTES
            || request.expected_revision < 0
            || (request.participant_changes.is_empty() && request.transcript_changes.is_empty())
            || request.participant_changes.len() > MAX_PATCH_CHANGES
            || request.transcript_changes.len() > MAX_PATCH_CHANGES
            || request.participant_changes.len() + request.transcript_changes.len()
                > MAX_PATCH_CHANGES
        {
            return Err(PrincipalRecordError::InvalidChange);
        }

        let participant_ids: HashSet<_> = request
            .participant_changes
            .iter()
            .map(|change| change.id.as_str())
            .collect();
        let transcript_ids: HashSet<_> = request
            .transcript_changes
            .iter()
            .map(|change| change.id.as_str())
            .collect();
        if participant_ids.len() != request.participant_changes.len()
            || transcript_ids.len() != request.transcript_changes.len()
            || request.participant_changes.iter().any(|change| {
                change.id.trim().is_empty()
                    || change.id.len() > MAX_ID_BYTES
                    || change.display_name.trim().is_empty()
                    || change.display_name.len() > MAX_PARTICIPANT_NAME_BYTES
            })
            || request.transcript_changes.iter().any(|change| {
                change.id.trim().is_empty()
                    || change.id.len() > MAX_ID_BYTES
                    || (change.text.is_none() && change.participant_id.is_none())
                    || change
                        .text
                        .as_ref()
                        .is_some_and(|text| text.len() > MAX_TRANSCRIPT_TEXT_BYTES)
                    || change
                        .participant_id
                        .as_ref()
                        .is_some_and(|id| id.trim().is_empty() || id.len() > MAX_ID_BYTES)
            })
        {
            return Err(PrincipalRecordError::InvalidChange);
        }

        let mut transaction = pool
            .begin()
            .await
            .map_err(|_| PrincipalRecordError::Storage)?;
        let result = async {
            let revision = sqlx::query(
                "UPDATE meetings SET principal_transcript_revision = principal_transcript_revision + 1 WHERE id = ? AND principal_transcript_revision = ?",
            )
            .bind(&request.meeting_id)
            .bind(request.expected_revision)
            .execute(&mut *transaction)
            .await
            .map_err(|_| PrincipalRecordError::Storage)?;
            if revision.rows_affected() != 1 {
                return Err(PrincipalRecordError::RevisionConflict);
            }

            let fallback_id = format!("{}:participant:unknown", request.meeting_id);
            let references_fallback = request
                .participant_changes
                .iter()
                .any(|change| change.id == fallback_id)
                || request.transcript_changes.iter().any(|change| {
                    change.participant_id.as_deref() == Some(fallback_id.as_str())
                });
            if references_fallback {
                sqlx::query(
                    "INSERT INTO participants (id, meeting_id, display_name, speaker_cluster_id)
                     SELECT ?, ?, 'Unknown participant', NULL
                     WHERE EXISTS (
                         SELECT 1 FROM transcripts t
                         WHERE t.meeting_id = ? AND (
                             t.participant_id IS NULL OR NOT EXISTS (
                                 SELECT 1 FROM participants p
                                 WHERE p.id = t.participant_id AND p.meeting_id = t.meeting_id
                             )
                         )
                     )
                     ON CONFLICT(id) DO NOTHING",
                )
                .bind(&fallback_id)
                .bind(&request.meeting_id)
                .bind(&request.meeting_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrincipalRecordError::Storage)?;
            }

            for change in &request.participant_changes {
                let updated = sqlx::query(
                    "UPDATE participants SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND meeting_id = ?",
                )
                .bind(change.display_name.trim())
                .bind(&change.id)
                .bind(&request.meeting_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| PrincipalRecordError::Storage)?;
                if updated.rows_affected() != 1 {
                    return Err(PrincipalRecordError::InvalidChange);
                }
            }

            for change in &request.transcript_changes {
                if let Some(participant_id) = &change.participant_id {
                    let owned: bool = sqlx::query_scalar(
                        "SELECT EXISTS(SELECT 1 FROM participants WHERE id = ? AND meeting_id = ?)",
                    )
                    .bind(participant_id)
                    .bind(&request.meeting_id)
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(|_| PrincipalRecordError::Storage)?;
                    if !owned {
                        return Err(PrincipalRecordError::InvalidChange);
                    }
                }

                let updated = match (&change.text, &change.participant_id) {
                    (Some(text), Some(participant_id)) => sqlx::query(
                        "UPDATE transcripts SET transcript = ?, participant_id = ? WHERE id = ? AND meeting_id = ?",
                    )
                    .bind(text)
                    .bind(participant_id)
                    .bind(&change.id)
                    .bind(&request.meeting_id)
                    .execute(&mut *transaction)
                    .await,
                    (Some(text), None) => sqlx::query(
                        "UPDATE transcripts SET transcript = ? WHERE id = ? AND meeting_id = ?",
                    )
                    .bind(text)
                    .bind(&change.id)
                    .bind(&request.meeting_id)
                    .execute(&mut *transaction)
                    .await,
                    (None, Some(participant_id)) => sqlx::query(
                        "UPDATE transcripts SET participant_id = ? WHERE id = ? AND meeting_id = ?",
                    )
                    .bind(participant_id)
                    .bind(&change.id)
                    .bind(&request.meeting_id)
                    .execute(&mut *transaction)
                    .await,
                    (None, None) => unreachable!(),
                }
                .map_err(|_| PrincipalRecordError::Storage)?;
                if updated.rows_affected() != 1 {
                    return Err(PrincipalRecordError::InvalidChange);
                }
            }

            Ok(PrincipalRecordReceipt {
                principal_transcript_revision: request.expected_revision + 1,
            })
        }
        .await;

        match result {
            Ok(receipt) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| PrincipalRecordError::Storage)?;
                Ok(receipt)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fixture() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('meeting-1', 'Review', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO participants (id, meeting_id, display_name, speaker_cluster_id) VALUES ('participant-a', 'meeting-1', 'Speaker 1', 'system:1'), ('participant-b', 'meeting-1', 'Speaker 2', 'system:2')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, speaker, participant_id, source_origin, speaker_cluster_id, ambiguity_group_id, alignment_decision, audio_start_time, audio_end_time, duration) VALUES ('block-1', 'meeting-1', 'Original', '00:00:01', 'Speaker 1', 'participant-a', 'system', 'system:1', 'overlap-1', 'preserved', 1.0, 2.0, 1.0), ('block-2', 'meeting-1', 'Unloaded', '00:00:03', 'Speaker 2', 'participant-b', 'microphone', 'system:2', NULL, NULL, 3.0, 4.0, 1.0)")
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    fn patch(expected_revision: i64) -> SavePrincipalRecord {
        SavePrincipalRecord {
            meeting_id: "meeting-1".into(),
            expected_revision,
            participant_changes: vec![ParticipantChange {
                id: "participant-a".into(),
                display_name: "Alice".into(),
            }],
            transcript_changes: vec![TranscriptChange {
                id: "block-1".into(),
                text: Some("Corrected".into()),
                participant_id: Some("participant-b".into()),
            }],
        }
    }

    #[tokio::test]
    async fn migration_seeds_stable_participants_without_changing_provenance() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, speaker TEXT, source_origin TEXT, speaker_cluster_id TEXT, ambiguity_group_id TEXT, alignment_decision TEXT, audio_start_time REAL, audio_end_time REAL, duration REAL)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO meetings VALUES ('meeting-1', 'Review', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transcripts VALUES ('block-1', 'meeting-1', 'Original', '00:00:01', 'Speaker 1', 'system', 'system:1', 'overlap-1', 'preserved', 1.0, 2.0, 1.0)")
            .execute(&pool).await.unwrap();

        sqlx::raw_sql(include_str!(
            "../../../migrations/20260813000000_add_principal_record.sql"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let row: (String, String, String, String, f64) = sqlx::query_as("SELECT p.display_name, t.source_origin, t.speaker_cluster_id, t.alignment_decision, t.audio_start_time FROM transcripts t JOIN participants p ON p.id = t.participant_id WHERE t.id = 'block-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "Speaker 1".into(),
                "system".into(),
                "system:1".into(),
                "preserved".into(),
                1.0
            )
        );
    }

    #[tokio::test]
    async fn patch_save_is_atomic_and_leaves_unloaded_blocks_unchanged() {
        let pool = fixture().await;
        let receipt = PrincipalRecordRepository::save(&pool, &patch(0))
            .await
            .unwrap();
        assert_eq!(receipt.principal_transcript_revision, 1);
        let rows: Vec<(String, String, String)> =
            sqlx::query_as("SELECT id, transcript, participant_id FROM transcripts ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                ("block-1".into(), "Corrected".into(), "participant-b".into()),
                ("block-2".into(), "Unloaded".into(), "participant-b".into())
            ]
        );
    }

    #[tokio::test]
    async fn malformed_legacy_rows_use_one_editable_persistent_fallback_participant() {
        let pool = fixture().await;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE transcripts SET participant_id = NULL WHERE id = 'block-1'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE transcripts SET participant_id = 'missing' WHERE id = 'block-2'")
            .execute(&pool)
            .await
            .unwrap();

        let record = PrincipalRecordRepository::get(&pool, "meeting-1")
            .await
            .unwrap();
        let fallback = record
            .participants
            .iter()
            .find(|participant| participant.display_name == "Unknown participant")
            .unwrap();

        assert_eq!(fallback.id, "meeting-1:participant:unknown");
        assert!(record
            .transcripts
            .iter()
            .all(|transcript| transcript.participant_id.as_deref() == Some(fallback.id.as_str())));
        assert!(record.transcripts.iter().all(|transcript| {
            transcript.source_origin.is_some()
                && transcript.speaker_cluster_id.is_some()
                && transcript.audio_start_time.is_some()
        }));

        PrincipalRecordRepository::save(
            &pool,
            &SavePrincipalRecord {
                meeting_id: "meeting-1".into(),
                expected_revision: 0,
                participant_changes: vec![ParticipantChange {
                    id: fallback.id.clone(),
                    display_name: "Recovered speaker".into(),
                }],
                transcript_changes: vec![TranscriptChange {
                    id: "block-2".into(),
                    text: None,
                    participant_id: Some(fallback.id.clone()),
                }],
            },
        )
        .await
        .unwrap();

        let reopened = PrincipalRecordRepository::get(&pool, "meeting-1")
            .await
            .unwrap();
        assert!(reopened.participants.iter().any(|participant| {
            participant.id == fallback.id && participant.display_name == "Recovered speaker"
        }));
        assert!(reopened
            .transcripts
            .iter()
            .all(|transcript| transcript.participant_id.as_deref() == Some(fallback.id.as_str())));
    }

    #[tokio::test]
    async fn stale_revision_changes_nothing() {
        let pool = fixture().await;
        PrincipalRecordRepository::save(&pool, &patch(0))
            .await
            .unwrap();
        assert_eq!(
            PrincipalRecordRepository::save(&pool, &patch(0)).await,
            Err(PrincipalRecordError::RevisionConflict)
        );
        let revision: i64 = sqlx::query_scalar(
            "SELECT principal_transcript_revision FROM meetings WHERE id = 'meeting-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(revision, 1);
    }

    #[tokio::test]
    async fn invalid_transcript_or_cross_session_participant_rolls_back_every_change() {
        let pool = fixture().await;
        let mut invalid = patch(0);
        invalid.transcript_changes.push(TranscriptChange {
            id: "missing".into(),
            text: Some("bad".into()),
            participant_id: None,
        });
        assert_eq!(
            PrincipalRecordRepository::save(&pool, &invalid).await,
            Err(PrincipalRecordError::InvalidChange)
        );
        let state: (String, String, i64) = sqlx::query_as("SELECT p.display_name, t.transcript, m.principal_transcript_revision FROM meetings m JOIN transcripts t ON t.meeting_id = m.id JOIN participants p ON p.id = 'participant-a' WHERE t.id = 'block-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(state, ("Speaker 1".into(), "Original".into(), 0));
    }

    #[tokio::test]
    async fn save_preserves_all_provenance_and_timing_fields() {
        let pool = fixture().await;
        PrincipalRecordRepository::save(&pool, &patch(0))
            .await
            .unwrap();
        let row: (String, String, Option<String>, Option<String>, f64, f64, f64) = sqlx::query_as("SELECT source_origin, speaker_cluster_id, ambiguity_group_id, alignment_decision, audio_start_time, audio_end_time, duration FROM transcripts WHERE id = 'block-1'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(
            row,
            (
                "system".into(),
                "system:1".into(),
                Some("overlap-1".into()),
                Some("preserved".into()),
                1.0,
                2.0,
                1.0
            )
        );
    }

    #[tokio::test]
    async fn oversized_patch_and_values_are_rejected_before_writes() {
        let pool = fixture().await;
        let mut too_many = patch(0);
        too_many.transcript_changes = (0..=MAX_PATCH_CHANGES)
            .map(|index| TranscriptChange {
                id: format!("block-{index}"),
                text: Some("x".into()),
                participant_id: None,
            })
            .collect();
        assert_eq!(
            PrincipalRecordRepository::save(&pool, &too_many).await,
            Err(PrincipalRecordError::InvalidChange)
        );

        let mut oversized_text = patch(0);
        oversized_text.transcript_changes[0].text = Some("x".repeat(MAX_TRANSCRIPT_TEXT_BYTES + 1));
        assert_eq!(
            PrincipalRecordRepository::save(&pool, &oversized_text).await,
            Err(PrincipalRecordError::InvalidChange)
        );

        let revision: i64 = sqlx::query_scalar(
            "SELECT principal_transcript_revision FROM meetings WHERE id = 'meeting-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(revision, 0);
    }
}
