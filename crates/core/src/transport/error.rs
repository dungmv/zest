//! Transport error type shared by all protocol executors.

use thiserror::Error;

/// Errors surfaced while preparing or executing a transport-level request.
#[derive(Error, Debug)]
pub enum RequestError {
    #[error("Request cancelled")]
    Cancelled,
    #[error("Network error: {0}")]
    Network(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Invalid Header: {0}")]
    InvalidHeader(String),
    #[error("Timeout after {0} seconds")]
    Timeout(u64),
    #[error("Pre-request script error: {0}")]
    ScriptError(String),
    #[error("Response exceeds the {0}-byte limit")]
    ResponseTooLarge(usize),
    #[error("Execution error: {0}")]
    Other(String),
}
