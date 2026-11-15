//! Server-Sent Events (SSE) parsing per the WHATWG stream update algorithm.
//!
//! The parser is a hand-rolled incremental byte feeder so `ReqwestExecutor`
//! can drive it straight off `bytes_stream()` without an extra dependency.
//! Every parsed item becomes a [`TransportFrame`] in the unified message log:
//! named/unnamed events map to `FrameKind::Text` (event name in the frame
//! label) and server keep-alive comments map to `FrameKind::Ping`.

mod parser;

pub use parser::{SseEvent, SseOutput, SseParser};
