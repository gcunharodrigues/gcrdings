use sqlx::{Pool, Sqlite};

use super::models::{Clip, ClipError, ClipStatus};

type Row = (
    String,
    String,
    Option<String>,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
    String,
);

fn to_clip(row: Row) -> Clip {
    let (id, meeting_id, title, start_ms, end_ms, status, file_path, error_code, created_at) = row;
    Clip {
        id,
        meeting_id,
        title,
        start_ms,
        end_ms,
        status: ClipStatus::from_column(&status),
        file_path,
        error_code,
        created_at,
    }
}

const COLUMNS: &str =
    "id, meeting_id, title, start_ms, end_ms, status, file_path, error_code, created_at";

pub struct ClipRepository;

impl ClipRepository {
    pub async fn list(pool: &Pool<Sqlite>, meeting_id: &str) -> Result<Vec<Clip>, ClipError> {
        sqlx::query_as::<_, Row>(&format!(
            "SELECT {COLUMNS} FROM clips WHERE meeting_id = ?1 ORDER BY start_ms ASC"
        ))
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .map_err(|_| ClipError::Storage)
        .map(|rows| rows.into_iter().map(to_clip).collect())
    }

    pub async fn get(pool: &Pool<Sqlite>, clip_id: &str) -> Result<Clip, ClipError> {
        sqlx::query_as::<_, Row>(&format!("SELECT {COLUMNS} FROM clips WHERE id = ?1"))
            .bind(clip_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ClipError::Storage)?
            .map(to_clip)
            .ok_or(ClipError::NotFound)
    }

    pub async fn create(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        title: Option<&str>,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Clip, ClipError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO clips (id, meeting_id, title, start_ms, end_ms, status) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
        )
        .bind(&id)
        .bind(meeting_id)
        .bind(title)
        .bind(start_ms)
        .bind(end_ms)
        .execute(pool)
        .await
        .map_err(|_| ClipError::Storage)?;

        Self::get(pool, &id).await
    }

    pub async fn set_status(
        pool: &Pool<Sqlite>,
        clip_id: &str,
        status: ClipStatus,
        file_path: Option<&str>,
        error_code: Option<&str>,
    ) -> Result<(), ClipError> {
        sqlx::query("UPDATE clips SET status = ?2, file_path = ?3, error_code = ?4 WHERE id = ?1")
            .bind(clip_id)
            .bind(status.as_str())
            .bind(file_path)
            .bind(error_code)
            .execute(pool)
            .await
            .map_err(|_| ClipError::Storage)?;
        Ok(())
    }

    pub async fn rename(
        pool: &Pool<Sqlite>,
        clip_id: &str,
        title: Option<&str>,
    ) -> Result<(), ClipError> {
        let result = sqlx::query("UPDATE clips SET title = ?2 WHERE id = ?1")
            .bind(clip_id)
            .bind(title)
            .execute(pool)
            .await
            .map_err(|_| ClipError::Storage)?;
        if result.rows_affected() == 0 {
            return Err(ClipError::NotFound);
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, clip_id: &str) -> Result<Option<String>, ClipError> {
        let clip = Self::get(pool, clip_id).await?;
        sqlx::query("DELETE FROM clips WHERE id = ?1")
            .bind(clip_id)
            .execute(pool)
            .await
            .map_err(|_| ClipError::Storage)?;
        Ok(clip.file_path)
    }

    pub async fn meeting_folder(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
    ) -> Result<String, ClipError> {
        sqlx::query_as::<_, (Option<String>,)>("SELECT folder_path FROM meetings WHERE id = ?1")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await
            .map_err(|_| ClipError::Storage)?
            .and_then(|(folder,)| folder)
            .ok_or(ClipError::SourceUnavailable)
    }
}
