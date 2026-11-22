use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use zest_core::models::{Environment, RequestItem};
use zest_core::transport::{FrameDirection, FrameKind, ResolvedRequest, TransportFrame};
use zest_core::ws::{WebSocketClient, WebSocketStatus, WsEvent, WsOutgoing};

use super::global_tokio_runtime;
use super::pipeline::{merge_env_mutations, run_pre_request_stage};
use super::send_event::SendEvent;

/// `(status, frame log, handshake headers)` view of one session.
pub type WsSnapshot = (WebSocketStatus, Vec<TransportFrame>, Vec<(String, String)>);

/// Maximum frames retained per session log (oldest dropped).
const MAX_SESSION_MESSAGES: usize = 1000;

/// Authoritative per-request connection record. The Runner pump and UI
/// actions all run on the GPUI thread; the `Mutex` only exists so the
/// service handle itself is `Send + Sync`.
pub struct WsSession {
    pub outgoing: UnboundedSender<WsOutgoing>,
    pub status: WebSocketStatus,
    /// Bounded frame log; survives request switches within the app session.
    pub messages: Vec<TransportFrame>,
    pub handshake_headers: Vec<(String, String)>,
    next_message_id: u64,
}

impl WsSession {
    fn push_message(&mut self, mut message: TransportFrame) {
        message.id = self.next_message_id;
        self.next_message_id = self.next_message_id.wrapping_add(1);
        self.messages.push(message);
        if self.messages.len() > MAX_SESSION_MESSAGES {
            let overflow = self.messages.len() - MAX_SESSION_MESSAGES;
            self.messages.drain(..overflow);
        }
    }
}

/// Long-lived WebSocket connection manager: owns the tokio-side pipeline and
/// the per-request session records (status + frame log).
pub struct WsService {
    tokio_rt: Arc<tokio::runtime::Runtime>,
    sessions: Mutex<HashMap<String, WsSession>>,
}

impl Default for WsService {
    fn default() -> Self {
        Self::new()
    }
}

impl WsService {
    pub fn new() -> Self {
        Self {
            tokio_rt: global_tokio_runtime(),
            sessions: Mutex::new(HashMap::new()),
        }
    }

    fn lock_sessions(&self) -> std::sync::MutexGuard<'_, HashMap<String, WsSession>> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts the connect pipeline on the tokio runtime and returns the event
    /// receiver the Runner must pump. Any existing session for the request is
    /// replaced once the new handshake succeeds.
    pub fn connect(
        &self,
        mut req_item: RequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> UnboundedReceiver<SendEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        self.tokio_rt.spawn(async move {
            // 1. Resolve dynamic fake variables + run the pre-request script
            let pre_output = run_pre_request_stage(&mut req_item, env.as_deref(), None);

            let _ = event_tx.send(SendEvent::Started {
                request_id: req_item.id.clone(),
                initial_logs: pre_output.console_entries.clone(),
                env_mutations: pre_output.env_mutations.clone(),
                pre_script_error: pre_output.error.clone(),
            });

            // 2. Merge pre-script env mutations into the effective environment
            let effective_env = merge_env_mutations(&env, &pre_output.env_mutations);

            // 3. Resolve the wire request (URL, headers, auth expansion)
            let resolved = match ResolvedRequest::from_item(&req_item, effective_env.as_deref()) {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_tx.send(SendEvent::Failed {
                        request_id: req_item.id.clone(),
                        error: e.to_string(),
                    });
                    return;
                }
            };

            // 4. Handshake, racing the cancel channel
            tokio::select! {
                _ = cancel_rx => {}
                res = WebSocketClient::connect(&resolved) => match res {
                    Ok(handle) => {
                        pump_connection(&req_item.id, handle.events, handle.outgoing, event_tx);
                    }
                    Err(e) => {
                        let _ = event_tx.send(SendEvent::Failed {
                            request_id: req_item.id.clone(),
                            error: e.to_string(),
                        });
                    }
                },
            }
        });

        event_rx
    }

    pub fn is_connected(&self, request_id: &str) -> bool {
        self.status_of(request_id) == WebSocketStatus::Connected
    }

    pub fn status_of(&self, request_id: &str) -> WebSocketStatus {
        self.lock_sessions()
            .get(request_id)
            .map(|s| s.status.clone())
            .unwrap_or(WebSocketStatus::Disconnected)
    }

    /// Copy of the session state for view restoration on request switch.
    pub fn snapshot(&self, request_id: &str) -> Option<WsSnapshot> {
        self.lock_sessions().get(request_id).map(|s| {
            (
                s.status.clone(),
                s.messages.clone(),
                s.handshake_headers.clone(),
            )
        })
    }

    /// Registers the live session once the handshake succeeded (Runner pump).
    pub fn register_session(
        &self,
        request_id: &str,
        outgoing: UnboundedSender<WsOutgoing>,
        handshake_headers: Vec<(String, String)>,
    ) {
        self.lock_sessions().insert(
            request_id.to_string(),
            WsSession {
                outgoing,
                status: WebSocketStatus::Connected,
                messages: Vec::new(),
                handshake_headers,
                next_message_id: 0,
            },
        );
    }

    /// Marks a registered session closed without sending another Close frame
    /// (used when the peer already closed the connection).
    pub fn mark_disconnected(&self, request_id: &str) {
        if let Some(session) = self.lock_sessions().get_mut(request_id) {
            session.status = WebSocketStatus::Disconnected;
        }
    }

    /// Marks a registered session failed (terminal connection error).
    pub fn mark_failed(&self, request_id: &str, error: &str) {
        if let Some(session) = self.lock_sessions().get_mut(request_id) {
            session.status = WebSocketStatus::Failed(error.to_string());
        }
    }

    /// Appends an inbound frame to the session log (Runner pump).
    pub fn append_inbound(&self, request_id: &str, message: TransportFrame) {
        if let Some(session) = self.lock_sessions().get_mut(request_id) {
            session.push_message(message);
        }
    }

    /// Queues a text frame and appends the outbound entry to the log.
    pub fn send_text(&self, request_id: &str, text: String) -> Result<TransportFrame, String> {
        let mut sessions = self.lock_sessions();
        let session = sessions
            .get_mut(request_id)
            .ok_or_else(|| "Not connected".to_string())?;
        if session.status != WebSocketStatus::Connected {
            return Err("Not connected".to_string());
        }
        let message = TransportFrame {
            id: 0, // assigned by push_message
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            direction: FrameDirection::Outbound,
            kind: FrameKind::Text,
            payload: text.clone().into_bytes(),
            label: None,
        };
        session
            .outgoing
            .send(WsOutgoing::Text(text))
            .map_err(|_| "Connection closed".to_string())?;
        session.push_message(message.clone());
        Ok(message)
    }

    pub fn send_ping(&self, request_id: &str) -> Result<(), String> {
        let mut sessions = self.lock_sessions();
        let session = sessions
            .get_mut(request_id)
            .ok_or_else(|| "Not connected".to_string())?;
        if session.status != WebSocketStatus::Connected {
            return Err("Not connected".to_string());
        }
        session
            .outgoing
            .send(WsOutgoing::Ping)
            .map_err(|_| "Connection closed".to_string())
    }

    /// Requests a graceful close; status flips immediately so the UI reacts,
    /// the eventual `Closed` event is idempotent.
    pub fn disconnect(&self, request_id: &str) {
        let mut sessions = self.lock_sessions();
        if let Some(session) = sessions.get_mut(request_id) {
            let _ = session.outgoing.send(WsOutgoing::Close);
            session.status = WebSocketStatus::Disconnected;
        }
    }

    /// Forgets a session entirely (e.g. its request was deleted).
    pub fn drop_session(&self, request_id: &str) {
        if let Some(session) = self.lock_sessions().remove(request_id) {
            let _ = session.outgoing.send(WsOutgoing::Close);
        }
    }
}

/// Forwards connection events to the UI channel until the connection ends.
fn pump_connection(
    request_id: &str,
    mut events: UnboundedReceiver<WsEvent>,
    outgoing: UnboundedSender<WsOutgoing>,
    ui_tx: UnboundedSender<SendEvent>,
) {
    let request_id = request_id.to_string();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let ui_event = match event {
                WsEvent::Connected {
                    response_headers,
                    duration_ms,
                } => SendEvent::HandshakeConnected {
                    request_id: request_id.clone(),
                    outgoing: outgoing.clone(),
                    response_headers,
                    duration_ms,
                },
                WsEvent::Message(frame) => SendEvent::Frame {
                    request_id: request_id.clone(),
                    frame,
                },
                WsEvent::Closed { code, reason } => SendEvent::HandshakeClosed {
                    request_id: request_id.clone(),
                    code,
                    reason,
                },
                WsEvent::Error(error) => SendEvent::Failed {
                    request_id: request_id.clone(),
                    error,
                },
            };
            if ui_tx.send(ui_event).is_err() {
                break;
            }
        }
    });
}
