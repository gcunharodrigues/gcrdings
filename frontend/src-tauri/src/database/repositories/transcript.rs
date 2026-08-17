use crate::api::{TranscriptSearchResult, TranscriptSegment};
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqlitePool};
use tracing::{error, info};
use uuid::Uuid;

pub struct TranscriptsRepository;

impl TranscriptsRepository {
    /// Saves a new meeting and its associated transcript segments.
    /// This function uses a transaction to ensure that either both the meeting
    /// and all its transcripts are saved, or none of them are.
    pub async fn save_transcript(
        pool: &SqlitePool,
        meeting_title: &str,
        transcripts: &[TranscriptSegment],
        folder_path: Option<String>,
    ) -> Result<String, SqlxError> {
        let meeting_id = format!("meeting-{}", Uuid::new_v4());

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        // 1. Create the new meeting
        let result = sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at, folder_path) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&meeting_id)
        .bind(meeting_title)
        .bind(now)
        .bind(now)
        .bind(&folder_path)
        .execute(&mut *transaction)
        .await;

        if let Err(e) = result {
            error!("Failed to create meeting '{}': {}", meeting_title, e);
            transaction.rollback().await?;
            return Err(e);
        }

        info!("Successfully created meeting with id: {}", meeting_id);

        // 2. Save each transcript segment with audio timing fields
        for segment in transcripts {
            let transcript_id = format!("transcript-{}", Uuid::new_v4());
            let result = sqlx::query(
                "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, speaker, source_origin, speaker_cluster_id, ambiguity_group_id, alignment_decision, audio_start_time, audio_end_time, duration)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&transcript_id)
            .bind(&meeting_id)
            .bind(&segment.text)
            .bind(&segment.timestamp)
            .bind(&segment.speaker)
            .bind(&segment.source_origin)
            .bind(&segment.speaker_cluster_id)
            .bind(&segment.ambiguity_group_id)
            .bind(&segment.alignment_decision)
            .bind(segment.audio_start_time)
            .bind(segment.audio_end_time)
            .bind(segment.duration)
            .execute(&mut *transaction)
            .await;

            if let Err(e) = result {
                error!(
                    "Failed to save transcript segment for meeting {}: {}",
                    meeting_id, e
                );
                transaction.rollback().await?;
                return Err(e);
            }
        }

        info!(
            "Successfully saved {} transcript segments for meeting {}",
            transcripts.len(),
            meeting_id
        );

        // Commit the transaction
        transaction.commit().await?;

        Ok(meeting_id)
    }

    /// Searches for a query string within the transcripts.
    /// It returns a list of matching transcripts with context.
    pub async fn search_transcripts(
        pool: &SqlitePool,
        query: &str,
    ) -> Result<Vec<TranscriptSearchResult>, SqlxError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let search_query = format!("%{}%", query.to_lowercase());

        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT m.id, m.title, t.transcript, t.timestamp
             FROM meetings m
             JOIN transcripts t ON m.id = t.meeting_id
             WHERE LOWER(t.transcript) LIKE ?",
        )
        .bind(&search_query)
        .fetch_all(pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|(id, title, transcript, timestamp)| {
                let match_context = Self::get_match_context(&transcript, query);
                TranscriptSearchResult {
                    id,
                    title,
                    match_context,
                    timestamp,
                }
            })
            .collect();

        Ok(results)
    }

    /// Helper function to extract a snippet of text around the first match of a query.
    fn get_match_context(transcript: &str, query: &str) -> String {
        let transcript_lower = transcript.to_lowercase();
        let query_lower = query.to_lowercase();

        match transcript_lower.find(&query_lower) {
            Some(match_index) => {
                let start_index = match_index.saturating_sub(100);
                let end_index = (match_index + query.len() + 100).min(transcript.len());

                let mut context = String::new();
                if start_index > 0 {
                    context.push_str("...");
                }
                context.push_str(&transcript[start_index..end_index]);
                if end_index < transcript.len() {
                    context.push_str("...");
                }
                context
            }
            None => transcript.chars().take(200).collect(), // Fallback to the start of the transcript
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn save_transcript_persists_speaker_label() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let segment = TranscriptSegment {
            id: "ignored-by-repository".to_string(),
            text: "Hello".to_string(),
            timestamp: "2026-08-05T00:00:00Z".to_string(),
            speaker: Some("Speaker 1".to_string()),
            source_origin: Some("imported".to_string()),
            speaker_cluster_id: Some("imported:Speaker 1".to_string()),
            ambiguity_group_id: None,
            alignment_decision: None,
            audio_start_time: Some(0.0),
            audio_end_time: Some(1.0),
            duration: Some(1.0),
        };

        let meeting_id =
            TranscriptsRepository::save_transcript(&pool, "Speaker persistence", &[segment], None)
                .await
                .unwrap();
        let saved: Option<String> =
            sqlx::query_scalar("SELECT speaker FROM transcripts WHERE meeting_id = ?")
                .bind(&meeting_id)
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(saved.as_deref(), Some("Speaker 1"));

        let origin: Option<String> =
            sqlx::query_scalar("SELECT source_origin FROM transcripts WHERE meeting_id = ?")
                .bind(&meeting_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(origin.as_deref(), Some("imported"));

        let cluster: Option<String> =
            sqlx::query_scalar("SELECT speaker_cluster_id FROM transcripts WHERE meeting_id = ?")
                .bind(&meeting_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(cluster.as_deref(), Some("imported:Speaker 1"));
    }

    #[tokio::test]
    async fn save_transcript_persists_captured_alignment_provenance() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let segment = TranscriptSegment {
            id: "block-1".to_string(),
            text: "Same words".to_string(),
            timestamp: "00:00:01".to_string(),
            speaker: Some("Guilherme".to_string()),
            source_origin: Some("microphone".to_string()),
            speaker_cluster_id: Some("microphone:local".to_string()),
            ambiguity_group_id: Some("ambiguity-fixed".to_string()),
            alignment_decision: Some("preserved-ambiguous-duplicate".to_string()),
            audio_start_time: Some(1.0),
            audio_end_time: Some(2.0),
            duration: Some(1.0),
        };

        let meeting_id =
            TranscriptsRepository::save_transcript(&pool, "Captured provenance", &[segment], None)
                .await
                .unwrap();
        let saved: (Option<String>, Option<String>, Option<String>, Option<String>) =
            sqlx::query_as(
                "SELECT source_origin, speaker_cluster_id, ambiguity_group_id, alignment_decision FROM transcripts WHERE meeting_id = ?",
            )
            .bind(meeting_id)
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(saved.0.as_deref(), Some("microphone"));
        assert_eq!(saved.1.as_deref(), Some("microphone:local"));
        assert_eq!(saved.2.as_deref(), Some("ambiguity-fixed"));
        assert_eq!(saved.3.as_deref(), Some("preserved-ambiguous-duplicate"));
    }
}
