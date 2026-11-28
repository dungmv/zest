use super::ResponseState;
use gpui_kit::Context;
use zest_core::grpc::GrpcCallSummary;
use zest_core::transport::{append_realtime_frame, TransportFrame};
use zest_core::ws::WebSocketStatus;

/// View-side cap for the live WS frame log (service keeps its own copy).
const MAX_VIEW_FRAMES: usize = 1000;

/// Realtime protocol (WebSocket / gRPC / SSE) session state mirrored from engines.
pub struct RealtimeResponseState {
    /// Live WebSocket connection status for the active request.
    pub ws_status: WebSocketStatus,
    /// Live frame log for the active request (mirrored from the WS/gRPC
    /// engines; shared by both realtime protocols).
    pub frames: Vec<TransportFrame>,
    /// Virtualized-list state backing the Messages tab (variable-height
    /// cards, tail-following while streaming).
    pub frames_list: gpui_kit::ListState,
    /// Handshake response headers of the active WS session.
    pub ws_handshake_headers: Vec<(String, String)>,
    /// Summary of the most recent gRPC call (status, headers, duration).
    pub grpc_last_call: Option<GrpcCallSummary>,
}

impl RealtimeResponseState {
    pub(super) fn new() -> Self {
        let frames_list =
            gpui_kit::ListState::new(0, gpui_kit::ListAlignment::Top, gpui_kit::px(600.));
        // Auto-scroll along the tail while streaming, as long as the user
        // is scrolled to the bottom.
        frames_list.set_follow_mode(gpui_kit::FollowMode::Tail);
        Self {
            ws_status: WebSocketStatus::default(),
            frames: Vec::new(),
            frames_list,
            ws_handshake_headers: Vec::new(),
            grpc_last_call: None,
        }
    }
    /// Clears the realtime protocol state (frame log, WS handshake headers,
    /// gRPC call summary) so a fresh send starts from a clean view. The WS
    /// status is deliberately left alone: the Runner sets `Connecting`
    /// around the same moment for WebSocket requests.
    pub(super) fn reset(&mut self) {
        self.frames.clear();
        self.frames_list.reset(0);
        self.ws_handshake_headers.clear();
        self.grpc_last_call = None;
    }

    fn push(&mut self, message: TransportFrame) {
        let old_count = self.frames.len();
        if append_realtime_frame(&mut self.frames, message, MAX_VIEW_FRAMES) > 0 {
            self.frames_list.reset(self.frames.len());
        } else {
            self.frames_list.splice(old_count..old_count, 1);
        }
    }

    fn restore(
        &mut self,
        status: WebSocketStatus,
        messages: Vec<TransportFrame>,
        handshake_headers: Vec<(String, String)>,
    ) {
        self.ws_status = status;
        self.frames = messages;
        self.frames_list.reset(self.frames.len());
        self.ws_handshake_headers = handshake_headers;
    }
}

impl ResponseState {
    pub fn set_ws_status(&mut self, status: WebSocketStatus, cx: &mut Context<Self>) {
        self.realtime.ws_status = status;
        cx.notify();
    }

    pub fn push_frame(&mut self, message: TransportFrame, cx: &mut Context<Self>) {
        self.realtime.push(message);
        cx.notify();
    }

    pub fn set_grpc_last_call(&mut self, summary: Option<GrpcCallSummary>, cx: &mut Context<Self>) {
        self.realtime.grpc_last_call = summary;
        cx.notify();
    }

    pub fn set_ws_handshake_headers(
        &mut self,
        headers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.realtime.ws_handshake_headers = headers;
        cx.notify();
    }

    /// Restores the realtime view when switching back to a request that has
    /// a live (or previously failed) connection / frame history in an
    /// engine. Does not touch `grpc_last_call`; callers restore it per
    /// protocol from the engine snapshot.
    pub fn restore_realtime_session(
        &mut self,
        status: WebSocketStatus,
        messages: Vec<TransportFrame>,
        handshake_headers: Vec<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        self.realtime.restore(status, messages, handshake_headers);
        self.view.reset_scroll();
        cx.notify();
    }
}
