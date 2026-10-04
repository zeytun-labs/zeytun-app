use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum CommandError {
    #[error("Core error: {0}")]
    Core(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("State error: {0}")]
    State(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        CommandError::Internal(s)
    }
}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        CommandError::Internal(s.to_string())
    }
}

impl From<crate::core::storage::StorageError> for CommandError {
    fn from(e: crate::core::storage::StorageError) -> Self {
        CommandError::Internal(e.to_string())
    }
}
