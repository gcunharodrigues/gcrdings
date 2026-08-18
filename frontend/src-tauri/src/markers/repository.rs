use sqlx::{Pool, Sqlite};

use super::models::{MarkerError, PendingMarker, SessionMarker};

pub struct MarkerRepository;

impl MarkerRepository {
    /// Markers of one Session, in playback order.
    pub async fn list(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
    ) -> Result<Vec<SessionMarker>, MarkerError> {
        sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
            "SELECT id, meeting_id, offset_ms, label, created_at \
             FROM session_markers WHERE meeting_id = ?1 ORDER BY offset_ms ASC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .map_err(|_| MarkerError::Storage)
        .map(|rows| {
            rows.into_iter()
                .map(
                    |(id, meeting_id, offset_ms, label, created_at)| SessionMarker {
                        id,
                        meeting_id,
                        offset_ms,
                        label,
                        created_at,
                    },
                )
                .collect()
        })
    }

    pub async fn insert(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        marker: &PendingMarker,
    ) -> Result<SessionMarker, MarkerError> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO session_markers (id, meeting_id, offset_ms, label) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(meeting_id)
        .bind(marker.offset_ms)
        .bind(&marker.label)
        .execute(pool)
        .await
        .map_err(|_| MarkerError::Storage)?;

        sqlx::query_as::<_, (String,)>("SELECT created_at FROM session_markers WHERE id = ?1")
            .bind(&id)
            .fetch_one(pool)
            .await
            .map_err(|_| MarkerError::Storage)
            .map(|(created_at,)| SessionMarker {
                id: id.clone(),
                meeting_id: meeting_id.to_string(),
                offset_ms: marker.offset_ms,
                label: marker.label.clone(),
                created_at,
            })
    }

    /// Attaches everything buffered during recording to the Session that was
    /// just created. One transaction: a partial flush would leave the recording
    /// with some of its markers and no way to tell which are missing.
    pub async fn insert_many(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        markers: &[PendingMarker],
    ) -> Result<u64, MarkerError> {
        if markers.is_empty() {
            return Ok(0);
        }

        let mut transaction = pool.begin().await.map_err(|_| MarkerError::Storage)?;

        for marker in markers {
            sqlx::query(
                "INSERT INTO session_markers (id, meeting_id, offset_ms, label) \
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(meeting_id)
            .bind(marker.offset_ms)
            .bind(&marker.label)
            .execute(&mut *transaction)
            .await
            .map_err(|_| MarkerError::Storage)?;
        }

        transaction
            .commit()
            .await
            .map_err(|_| MarkerError::Storage)?;
        Ok(markers.len() as u64)
    }

    pub async fn update_label(
        pool: &Pool<Sqlite>,
        marker_id: &str,
        label: Option<&str>,
    ) -> Result<(), MarkerError> {
        let result = sqlx::query("UPDATE session_markers SET label = ?2 WHERE id = ?1")
            .bind(marker_id)
            .bind(label)
            .execute(pool)
            .await
            .map_err(|_| MarkerError::Storage)?;

        if result.rows_affected() == 0 {
            return Err(MarkerError::NotFound);
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, marker_id: &str) -> Result<(), MarkerError> {
        let result = sqlx::query("DELETE FROM session_markers WHERE id = ?1")
            .bind(marker_id)
            .execute(pool)
            .await
            .map_err(|_| MarkerError::Storage)?;

        if result.rows_affected() == 0 {
            return Err(MarkerError::NotFound);
        }
        Ok(())
    }
}
