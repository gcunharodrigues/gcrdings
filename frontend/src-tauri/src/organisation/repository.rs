use sqlx::{Pool, Sqlite};

use super::models::{Folder, OrganisationError, Tag};

fn storage_error(error: sqlx::Error) -> OrganisationError {
    // SQLite reports a violated UNIQUE index as a constraint error. Surfacing
    // that as "storage failed" would send the user to a log file for something
    // they can fix by picking another name.
    if let sqlx::Error::Database(db_error) = &error {
        if db_error.message().contains("UNIQUE constraint failed") {
            return OrganisationError::DuplicateName;
        }
    }
    OrganisationError::Storage
}

pub struct FolderRepository;

impl FolderRepository {
    pub async fn list(pool: &Pool<Sqlite>) -> Result<Vec<Folder>, OrganisationError> {
        sqlx::query_as::<_, (String, String, Option<String>, String, i64)>(
            "SELECT f.id, f.name, f.parent_id, f.created_at, \
                    (SELECT COUNT(*) FROM meetings m WHERE m.folder_id = f.id) \
             FROM folders f ORDER BY f.name COLLATE NOCASE ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(storage_error)
        .map(|rows| {
            rows.into_iter()
                .map(|(id, name, parent_id, created_at, session_count)| Folder {
                    id,
                    name,
                    parent_id,
                    created_at,
                    session_count,
                })
                .collect()
        })
    }

    pub async fn create(
        pool: &Pool<Sqlite>,
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<String, OrganisationError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO folders (id, name, parent_id) VALUES (?1, ?2, ?3)")
            .bind(&id)
            .bind(name)
            .bind(parent_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        Ok(id)
    }

    pub async fn rename(
        pool: &Pool<Sqlite>,
        folder_id: &str,
        name: &str,
    ) -> Result<(), OrganisationError> {
        let result = sqlx::query("UPDATE folders SET name = ?2 WHERE id = ?1")
            .bind(folder_id)
            .bind(name)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }

    /// Rejects a move that would detach a subtree from the root by making a
    /// folder its own ancestor. Walking upwards is cheap: folder trees people
    /// actually build are shallow.
    pub async fn would_create_cycle(
        pool: &Pool<Sqlite>,
        folder_id: &str,
        new_parent: &str,
    ) -> Result<bool, OrganisationError> {
        if folder_id == new_parent {
            return Ok(true);
        }

        let mut cursor = Some(new_parent.to_string());
        // A corrupt tree must not spin forever; the bound is the folder count.
        let mut hops = 0;
        while let Some(current) = cursor {
            if current == folder_id {
                return Ok(true);
            }
            hops += 1;
            if hops > 1_000 {
                return Err(OrganisationError::Storage);
            }
            cursor = sqlx::query_as::<_, (Option<String>,)>(
                "SELECT parent_id FROM folders WHERE id = ?1",
            )
            .bind(&current)
            .fetch_optional(pool)
            .await
            .map_err(storage_error)?
            .and_then(|(parent,)| parent);
        }
        Ok(false)
    }

    pub async fn move_to(
        pool: &Pool<Sqlite>,
        folder_id: &str,
        parent_id: Option<&str>,
    ) -> Result<(), OrganisationError> {
        if let Some(parent) = parent_id {
            if Self::would_create_cycle(pool, folder_id, parent).await? {
                return Err(OrganisationError::CircularParent);
            }
        }

        let result = sqlx::query("UPDATE folders SET parent_id = ?2 WHERE id = ?1")
            .bind(folder_id)
            .bind(parent_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }

    /// Sessions are never deleted with their folder — the schema sets their
    /// folder_id to NULL, returning them to the root.
    pub async fn delete(pool: &Pool<Sqlite>, folder_id: &str) -> Result<(), OrganisationError> {
        let result = sqlx::query("DELETE FROM folders WHERE id = ?1")
            .bind(folder_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }

    pub async fn assign_session(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        folder_id: Option<&str>,
    ) -> Result<(), OrganisationError> {
        let result = sqlx::query("UPDATE meetings SET folder_id = ?2 WHERE id = ?1")
            .bind(meeting_id)
            .bind(folder_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }
}

pub struct TagRepository;

impl TagRepository {
    pub async fn list(pool: &Pool<Sqlite>) -> Result<Vec<Tag>, OrganisationError> {
        sqlx::query_as::<_, (String, String, Option<String>, String, i64)>(
            "SELECT t.id, t.name, t.color, t.created_at, \
                    (SELECT COUNT(*) FROM meeting_tags mt WHERE mt.tag_id = t.id) \
             FROM tags t ORDER BY t.name COLLATE NOCASE ASC",
        )
        .fetch_all(pool)
        .await
        .map_err(storage_error)
        .map(|rows| {
            rows.into_iter()
                .map(|(id, name, color, created_at, session_count)| Tag {
                    id,
                    name,
                    color,
                    created_at,
                    session_count,
                })
                .collect()
        })
    }

    pub async fn create(
        pool: &Pool<Sqlite>,
        name: &str,
        color: Option<&str>,
    ) -> Result<String, OrganisationError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO tags (id, name, color) VALUES (?1, ?2, ?3)")
            .bind(&id)
            .bind(name)
            .bind(color)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        Ok(id)
    }

    pub async fn rename(
        pool: &Pool<Sqlite>,
        tag_id: &str,
        name: &str,
        color: Option<&str>,
    ) -> Result<(), OrganisationError> {
        let result = sqlx::query("UPDATE tags SET name = ?2, color = ?3 WHERE id = ?1")
            .bind(tag_id)
            .bind(name)
            .bind(color)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }

    pub async fn delete(pool: &Pool<Sqlite>, tag_id: &str) -> Result<(), OrganisationError> {
        let result = sqlx::query("DELETE FROM tags WHERE id = ?1")
            .bind(tag_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        if result.rows_affected() == 0 {
            return Err(OrganisationError::NotFound);
        }
        Ok(())
    }

    pub async fn tags_for_session(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
    ) -> Result<Vec<Tag>, OrganisationError> {
        sqlx::query_as::<_, (String, String, Option<String>, String)>(
            "SELECT t.id, t.name, t.color, t.created_at \
             FROM tags t JOIN meeting_tags mt ON mt.tag_id = t.id \
             WHERE mt.meeting_id = ?1 ORDER BY t.name COLLATE NOCASE ASC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .map_err(storage_error)
        .map(|rows| {
            rows.into_iter()
                .map(|(id, name, color, created_at)| Tag {
                    id,
                    name,
                    color,
                    created_at,
                    session_count: 0,
                })
                .collect()
        })
    }

    /// Idempotent: tagging something already tagged is a no-op rather than an
    /// error, because the UI treats it as a toggle.
    pub async fn attach(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        tag_id: &str,
    ) -> Result<(), OrganisationError> {
        sqlx::query("INSERT OR IGNORE INTO meeting_tags (meeting_id, tag_id) VALUES (?1, ?2)")
            .bind(meeting_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    pub async fn detach(
        pool: &Pool<Sqlite>,
        meeting_id: &str,
        tag_id: &str,
    ) -> Result<(), OrganisationError> {
        sqlx::query("DELETE FROM meeting_tags WHERE meeting_id = ?1 AND tag_id = ?2")
            .bind(meeting_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}
