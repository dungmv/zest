/// One resolved RPC method from the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcMethodInfo {
    pub name: String,
    pub is_client_streaming: bool,
    pub is_server_streaming: bool,
}

impl GrpcMethodInfo {
    /// Human label used in the service/method picker, e.g. "SayHello()".
    pub fn label(&self) -> String {
        if self.is_client_streaming || self.is_server_streaming {
            format!(
                "{}({}stream)",
                self.name,
                match (self.is_client_streaming, self.is_server_streaming) {
                    (true, true) => "bi",
                    (true, false) => "client-",
                    (false, true) => "server-",
                    (false, false) => "",
                }
            )
        } else {
            format!("{}()", self.name)
        }
    }
}

/// Result of one unary call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcUnaryResult {
    /// Canonical JSON encoding of the response message.
    pub response_json: String,
    /// Response headers (trailers-only responses merge into this set).
    pub headers: Vec<(String, String)>,
    pub duration_ms: u64,
}

/// Summary of the most recent gRPC call (unary or streaming), shown in the
/// response status bar and Headers tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcCallSummary {
    /// grpc-status code (0 = OK).
    pub status_code: i32,
    pub status_message: Option<String>,
    pub headers: Vec<(String, String)>,
    pub duration_ms: u64,
}

impl GrpcCallSummary {
    pub fn is_ok(&self) -> bool {
        self.status_code == 0
    }
}

/// Stream of events for a server-streaming call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrpcStreamEvent {
    /// One response message from the stream.
    Message { index: u64, json: String },
    /// Stream finished cleanly (grpc-status 0).
    Completed { duration_ms: u64 },
    /// Stream finished with a non-OK status.
    Failed {
        code: i32,
        message: String,
        duration_ms: u64,
    },
    /// Transport-level failure mid-stream.
    TransportError(String),
}

/// Maps the request URL onto a tonic endpoint URI and its TLS mode.
/// `grpc://` → plaintext HTTP/2, `grpcs://`/`https://` → TLS, anything else
/// defaults to plaintext.
pub(crate) fn map_endpoint(url: &str) -> Result<(String, bool), crate::grpc::error::GrpcError> {
    let (scheme, rest) = if let Some(rest) = url.strip_prefix("grpcs://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("grpc://") {
        ("http", rest)
    } else if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("http", url)
    };
    if rest.is_empty() {
        return Err(crate::grpc::error::GrpcError::InvalidEndpoint(
            url.to_string(),
        ));
    }
    Ok((format!("{scheme}://{rest}"), scheme == "https"))
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
