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

/// Whether a frame travelled server→client or client→server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WsDirection {
    Inbound,
    Outbound,
}

/// Frame classification shown as a badge in the message log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WsMessageKind {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
}

/// One frame in the message log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WsMessage {
    /// Monotonic id within the session log.
    pub id: u64,
    /// Unix epoch milliseconds.
    pub timestamp_ms: i64,
    pub direction: WsDirection,
    pub kind: WsMessageKind,
    pub payload: Vec<u8>,
}

impl WsMessage {
    /// Payload decoded as UTF-8 when the frame is text-like, for previews.
    pub fn text_preview(&self, max_chars: usize) -> String {
        let text = String::from_utf8_lossy(&self.payload);
        let mut out: String = text.chars().take(max_chars).collect();
        if text.chars().count() > max_chars {
            out.push('…');
        }
        out
    }

    pub fn size_bytes(&self) -> usize {
        self.payload.len()
    }
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
    Message(WsMessage),
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
