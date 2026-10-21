use serde::Serialize;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct CoreError {
    pub code: String,
    pub message: String,
}

impl CoreError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("INVALID", message)
    }
    pub fn blocked(message: impl Into<String>) -> Self {
        Self::new("BLOCKED", message)
    }
    pub fn local(message: impl Into<String>) -> Self {
        Self::new("FAILED_LOCAL", message)
    }
}

impl From<std::io::Error> for CoreError {
    fn from(error: std::io::Error) -> Self {
        Self::local(error.to_string())
    }
}
impl From<serde_json::Error> for CoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::invalid(error.to_string())
    }
}
impl From<rusqlite::Error> for CoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::local(error.to_string())
    }
}
