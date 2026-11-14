use thiserror::Error;

/// Errors surfaced by the gRPC engine.
#[derive(Debug, Error)]
pub enum GrpcError {
    #[error("Invalid gRPC endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("Schema error: {0}")]
    Schema(String),
    #[error("Connection failed: {0}")]
    Channel(String),
    #[error("Invalid request message: {0}")]
    InvalidMessage(String),
    #[error("RPC failed: {message} (code {code})")]
    Status { code: i32, message: String },
    #[error("Timeout after {0} ms")]
    Timeout(u64),
    #[error("I/O error: {0}")]
    Io(String),
}

impl GrpcError {
    /// grpc status code carried by [`GrpcError::Status`], if any.
    pub fn status_code(&self) -> Option<i32> {
        match self {
            GrpcError::Status { code, .. } => Some(*code),
            _ => None,
        }
    }
}

impl From<std::io::Error> for GrpcError {
    fn from(e: std::io::Error) -> Self {
        GrpcError::Io(e.to_string())
    }
}
