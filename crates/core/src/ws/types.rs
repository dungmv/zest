use crate::transport::TransportFrame;
use serde::{Deserialize, Serialize};

/// Lifecycle state of a WebSocket connection, as shown in the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WebSocketStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// Application-initiated action routed to the connection's writer task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsOutgoing {
    Text(String),
    Binary(Vec<u8>),
    Ping,
    Close,
}

/// Stream of connection lifecycle + frame events delivered to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    /// Handshake completed. Carries the handshake response headers.
    Connected {
        response_headers: Vec<(String, String)>,
        duration_ms: u64,
    },
    Message(TransportFrame),
    /// Peer closed the connection (or we closed it and got the close ack).
    Closed {
        code: Option<u16>,
        reason: String,
    },
    /// Terminal failure of the connection (network/protocol error).
    Error(String),
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
