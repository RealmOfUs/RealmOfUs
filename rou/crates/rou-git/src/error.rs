//! Stable host failure categories while retaining the full anyhow context chain.
//!
//! Domain errors preserve `rou_core::Error::code`. Host precondition failures that
//! have no narrower category use `host_error`; display messages are not an API.
use std::fmt;

/// A host precondition failure with an explicit stable code and human diagnostics.
#[derive(Debug)]
pub struct HostError {
    /// Stable category; callers must not parse the diagnostic message.
    pub code: &'static str,
    /// Context for a person, including relevant paths or required recovery steps.
    pub message: String,
}
impl HostError {
    /// Construct a categorized host failure without inspecting message text.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for HostError {}

/// Return a typed host failure if a precondition is false.
#[macro_export]
macro_rules! ensure {
    ($code:literal, $condition:expr, $($message:tt)*) => {
        if !$condition {
            return Err($crate::error::HostError::new($code, format!($($message)*)).into());
        }
    };
}

/// Return a typed host failure with an explicit machine-readable category.
#[macro_export]
macro_rules! bail {
    ($code:literal, $($message:tt)*) => {
        return Err($crate::error::HostError::new($code, format!($($message)*)).into())
    };
}

/// A failed native Git subprocess, including its original diagnostics.
#[derive(Debug)]
pub struct GitFailure {
    /// Native exit status, absent when the process was terminated by a signal.
    pub exit_code: Option<i32>,
    /// Combined stderr/stdout diagnostics, intended for people rather than parsing.
    pub message: String,
}
impl fmt::Display for GitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Git failed ({:?}): {}",
            self.exit_code, self.message
        )
    }
}
impl std::error::Error for GitFailure {}

/// Return a stable category without parsing human-readable diagnostics.
///
/// Explicit host preconditions retain their [`HostError::code`], such as
/// `snapshot_changed`, `unresolved_conflict`, `stale_runtime` or `lock_busy`.
/// Other categories are domain codes, `git_failed`, `missing_payload`, `io_error`,
/// `invalid_json`, and `host_error` for unclassified external failures.
/// Context added with `anyhow::Context` does not erase the underlying category.
pub fn code(error: &anyhow::Error) -> &str {
    if let Some(domain) = error.downcast_ref::<rou_core::Error>() {
        &domain.code
    } else if let Some(host) = error.downcast_ref::<HostError>() {
        host.code
    } else if error.is::<GitFailure>() {
        "git_failed"
    } else if error.is::<crate::payload::MissingPayload>() {
        "missing_payload"
    } else if error.is::<std::io::Error>() {
        "io_error"
    } else if error.is::<serde_json::Error>() {
        "invalid_json"
    } else {
        "host_error"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_preserves_machine_codes() {
        let error = anyhow::Error::new(rou_core::Error::new("invalid_source", "bad path"))
            .context("while building");
        assert_eq!(code(&error), "invalid_source");
        let error = anyhow::Error::new(GitFailure {
            exit_code: Some(128),
            message: "diagnostic".into(),
        })
        .context("while switching");
        assert_eq!(code(&error), "git_failed");
        let error = anyhow::Error::new(HostError::new("stale_runtime", "select the model"))
            .context("while building");
        assert_eq!(code(&error), "stale_runtime");
    }
}
