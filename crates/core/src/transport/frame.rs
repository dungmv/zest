//! Unified realtime frame log shared by WebSocket and gRPC sessions.
//!
//! Frames are in-memory only — they are never persisted to YAML — so this
//! type may evolve without storage-compatibility concerns.

use serde::{Deserialize, Serialize};

/// Whether a frame travelled server→client or client→server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameDirection {
    Inbound,
    Outbound,
}

/// Frame classification shown as a badge in the message log. WebSocket uses
/// the full set; gRPC frames are emitted as [`FrameKind::Text`] (JSON payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FrameKind {
    Text,
    Binary,
    Ping,
    Pong,
    Close,
}

/// One frame in the message log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportFrame {
    /// Monotonic id within the session log.
    pub id: u64,
    /// Unix epoch milliseconds.
    pub timestamp_ms: i64,
    pub direction: FrameDirection,
    pub kind: FrameKind,
    pub payload: Vec<u8>,
}

impl TransportFrame {
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

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
