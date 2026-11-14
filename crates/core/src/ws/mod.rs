//! WebSocket client engine built on `tokio-tungstenite`.
//!
//! A connection is a long-lived background task pair (reader + writer) that
//! streams [`WsEvent`]s to the UI through an unbounded channel, mirroring the
//! sketches in `docs/specs/03-protocols-graphql-websocket-grpc.md`.

mod client;
mod types;

pub use client::WebSocketClient;
pub use types::{
    WsDirection, WsEvent, WsMessage, WsMessageKind, WsOutgoing, WebSocketStatus,
};
