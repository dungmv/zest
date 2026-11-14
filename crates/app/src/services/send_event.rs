//! Unified send event channel shared by the HTTP, WebSocket and gRPC engines.
//!
//! Every engine emits the same [`SendEvent`] shape over an unbounded tokio
//! channel; the Runner pump running on the GPUI thread is the single
//! consumer.

use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use zest_core::grpc::GrpcCallSummary;
use zest_core::models::{ConsoleLogEntry, HttpResponseRecord};
use zest_core::scripting::PostResponseOutput;
use zest_core::transport::TransportFrame;
use zest_core::ws::WsOutgoing;

use super::grpc_engine::GrpcSchemaSummary;

/// Events for one send invocation. Common lifecycle variants (Started /
/// Progress / Failed / Cancelled) are shared by all transports; the
/// transport-specific outcomes keep their own variants so per-protocol
/// handling in the pump stays explicit.
pub enum SendEvent {
    /// Fake vars + pre-request script finished; the transport call is about
    /// to start. Pre-script env mutations ride along so the pump can chain
    /// them uniformly for every protocol.
    Started {
        request_id: String,
        initial_logs: Vec<ConsoleLogEntry>,
        env_mutations: Vec<(String, Option<String>)>,
        pre_script_error: Option<String>,
    },
    /// Human-readable progress text (HTTP transport only).
    Progress(String),
    /// One realtime frame for the unified message log: WS inbound frames,
    /// gRPC outbound request echo and inbound response messages.
    Frame {
        request_id: String,
        frame: TransportFrame,
    },
    /// HTTP exchange finished. Pre-script mutations were already delivered
    /// via [`SendEvent::Started`]; only the post-response output rides here.
    HttpSuccess {
        record: Arc<HttpResponseRecord>,
        post_output: Box<PostResponseOutput>,
    },
    /// WebSocket handshake succeeded; carries the writer handle the WS
    /// service registers for outbound sends.
    HandshakeConnected {
        request_id: String,
        outgoing: UnboundedSender<WsOutgoing>,
        response_headers: Vec<(String, String)>,
        duration_ms: u64,
    },
    /// WebSocket closed (peer closed, or the close ack was received).
    HandshakeClosed {
        request_id: String,
        code: Option<u16>,
        reason: String,
    },
    /// gRPC call finished (unary OK, or stream completed).
    CallCompleted {
        request_id: String,
        summary: GrpcCallSummary,
    },
    /// gRPC server-stream finished with a non-OK status mid-stream.
    CallFailed {
        request_id: String,
        summary: GrpcCallSummary,
    },
    /// Schema resolved during an invocation with a missing service/method
    /// selection. When the server exposes exactly one service+method the
    /// engine continues with them and the UI should persist the selection.
    SchemaReady {
        request_id: String,
        summary: GrpcSchemaSummary,
        auto_service: Option<String>,
        auto_method: Option<String>,
    },
    /// The invocation finished without a result (user cancelled).
    Cancelled,
    /// Terminal setup/transport failure before any result.
    Failed { request_id: String, error: String },
}
