use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A place a Session lives. Exactly one per Session, or none for the root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    /// Sessions filed directly in this folder, excluding its children.
    pub session_count: i64,
}

/// A subject a Session is about. Many per Session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub session_count: i64,
}

#[derive(Debug, Error)]
pub enum OrganisationError {
    #[error("not found")]
    NotFound,
    #[error("a folder or tag with that name already exists here")]
    DuplicateName,
    #[error("a folder cannot be moved inside itself")]
    CircularParent,
    #[error("the name cannot be blank")]
    BlankName,
    #[error("storage failed")]
    Storage,
}

impl serde::Serialize for OrganisationError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self {
            OrganisationError::NotFound => "not_found",
            OrganisationError::DuplicateName => "duplicate_name",
            OrganisationError::CircularParent => "circular_parent",
            OrganisationError::BlankName => "blank_name",
            OrganisationError::Storage => "storage",
        };
        let mut state = serializer.serialize_struct("OrganisationError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

/// Trims a user-supplied name and rejects blanks, so the tree can never grow a
/// row nobody can see or click.
pub fn validate_name(name: &str) -> Result<String, OrganisationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(OrganisationError::BlankName);
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_names_are_rejected() {
        assert!(matches!(
            validate_name(""),
            Err(OrganisationError::BlankName)
        ));
        assert!(matches!(
            validate_name("   \n "),
            Err(OrganisationError::BlankName)
        ));
    }

    #[test]
    fn names_are_trimmed() {
        assert_eq!(validate_name("  Clientes  ").unwrap(), "Clientes");
    }
}
