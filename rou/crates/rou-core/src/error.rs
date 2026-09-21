//! Host-independent domain errors, without filesystem or host exception dependencies.
use serde::Serialize;

/// The result of a core operation, carrying a serializable [`Error`] on failure.
pub type Result<T> = std::result::Result<T, Error>;

/// A domain error with a stable machine-readable code.
///
/// `Display` emits only [`message`](Self::message). Programs should branch on
/// [`code`](Self::code), not parse display text that may contain paths or other context.
/// Serde JSON errors are uniformly converted to `invalid_json`.
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct Error {
    /// Failure category for host logic, such as `invalid_json` or `outside_world`.
    pub code: String,
    /// Human-readable reason and context; not a stable machine protocol.
    pub message: String,
}

impl Error {
    /// Create an error from its code and description without checking membership in a predefined code set.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new("invalid_json", error.to_string())
    }
}

pub(crate) fn require(condition: bool, code: &str, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::new(code, message))
    }
}
