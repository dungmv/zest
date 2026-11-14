use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::Arc;

use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use zest_core::http::ResolvedRequest;
use zest_core::models::{ConsoleLogEntry, Environment, EnvironmentVariable, HttpRequestItem};
use zest_core::scripting::{
    resolve_fake_vars_in_request, PreRequestContext, PreRequestOutput, ScriptSandbox,
};
use zest_core::ws::{
    WebSocketClient, WsDirection, WsEvent, WsMessage, WsMessageKind, WsOutgoing, WebSocketStatus,
};

use super::global_tokio_runtime;

/// `(status, frame log, handshake headers)` view of one session.
pub type WsSnapshot = (WebSocketStatus, Vec<WsMessage>, Vec<(String, String)>);

/// Maximum frames retained per session log (oldest dropped).
const MAX_SESSION_MESSAGES: usize = 1000;

/// UI-delivered lifecycle events for a WebSocket connection, mirroring the
/// shape of [`super::http_engine::ExecEvent`]. Consumed exclusively on the
/// GPUI thread by the Runner pump.
pub enum WsUiEvent {
    /// Pre-request script finished; handshake about to start.
    Started {
        request_id: String,
        initial_logs: Vec<ConsoleLogEntry>,
        env_mutations: Vec<(String, Option<String>)>,
        pre_script_error: Option<String>,
    },
    /// Handshake succeeded. Carries the writer handle the service registers.
    Connected {
        request_id: String,
        outgoing: UnboundedSender<WsOutgoing>,
        response_headers: Vec<(String, String)>,
        duration_ms: u64,
    },
    Message {
        request_id: String,
        message: WsMessage,
    },
    Closed {
        request_id: String,
        code: Option<u16>,
        reason: String,
    },
    Failed {
        request_id: String,
        error: String,
    },
}

/// Authoritative per-request connection record. The Runner pump and UI
/// actions all run on the GPUI thread; the `Mutex` only exists so the
/// service handle itself is `Send + Sync`.
pub struct WsSession {
    pub outgoing: UnboundedSender<WsOutgoing>,
    pub status: WebSocketStatus,
    /// Bounded frame log; survives request switches within the app session.
    pub messages: Vec<WsMessage>,
    pub handshake_headers: Vec<(String, String)>,
    next_message_id: u64,
}

impl WsSession {
    fn push_message(&mut self, mut message: WsMessage) {
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
        mut req_item: HttpRequestItem,
        env: Option<Arc<Environment>>,
        cancel_rx: oneshot::Receiver<()>,
    ) -> UnboundedReceiver<WsUiEvent> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        self.tokio_rt.spawn(async move {
            // 1. Resolve dynamic fake variables + run the pre-request script
            resolve_fake_vars_in_request(&mut req_item);
            let pre_script_code = req_item
                .script
                .as_ref()
                .and_then(|s| s.pre_request.clone());
            let pre_output = match pre_script_code {
                Some(code) => {
                    let ctx = PreRequestContext::from_request_and_env(&req_item, env.as_deref());
                    ScriptSandbox::run_pre_request(&code, ctx)
                }
                None => PreRequestOutput::default(),
            };

            let _ = event_tx.send(WsUiEvent::Started {
                request_id: req_item.id.clone(),
                initial_logs: pre_output.console_entries.clone(),
                env_mutations: pre_output.env_mutations.clone(),
                pre_script_error: pre_output.error.clone(),
            });

            // 2. Merge pre-script env mutations into the effective environment
            let effective_env = merge_env_mutations(env, &pre_output.env_mutations);

            // 3. Resolve the wire request (URL, headers, auth expansion)
            let resolved = match ResolvedRequest::from_item(&req_item, effective_env.as_ref()) {
                Ok(r) => r,
                Err(e) => {
                    let _ = event_tx.send(WsUiEvent::Failed {
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
                        let _ = event_tx.send(WsUiEvent::Failed {
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
    pub fn append_inbound(&self, request_id: &str, message: WsMessage) {
        if let Some(session) = self.lock_sessions().get_mut(request_id) {
            session.push_message(message);
        }
    }

    /// Queues a text frame and appends the outbound entry to the log.
    pub fn send_text(&self, request_id: &str, text: String) -> Result<WsMessage, String> {
        let mut sessions = self.lock_sessions();
        let session = sessions
            .get_mut(request_id)
            .ok_or_else(|| "Not connected".to_string())?;
        if session.status != WebSocketStatus::Connected {
            return Err("Not connected".to_string());
        }
        let message = WsMessage {
            id: 0, // assigned by push_message
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            direction: WsDirection::Outbound,
            kind: WsMessageKind::Text,
            payload: text.clone().into_bytes(),
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

fn merge_env_mutations(
    env: Option<Arc<Environment>>,
    mutations: &[(String, Option<String>)],
) -> Option<Environment> {
    if mutations.is_empty() {
        return env.as_deref().cloned();
    }
    let mut temp = env
        .as_ref()
        .map(|e| (**e).clone())
        .unwrap_or_else(|| Environment::new("temp", "Temp"));
    for (k, v_opt) in mutations {
        if let Some(val) = v_opt {
            if let Some(v_item) = temp.variables.iter_mut().find(|v| v.key == *k) {
                v_item.current_value = val.clone();
            } else {
                temp.variables.push(EnvironmentVariable {
                    key: k.clone(),
                    initial_value: String::new(),
                    current_value: val.clone(),
                    secret: false,
                });
            }
        } else {
            temp.variables.retain(|v| v.key != *k);
        }
    }
    Some(temp)
}

/// Forwards connection events to the UI channel until the connection ends.
fn pump_connection(
    request_id: &str,
    mut events: UnboundedReceiver<WsEvent>,
    outgoing: UnboundedSender<WsOutgoing>,
    ui_tx: UnboundedSender<WsUiEvent>,
) {
    let request_id = request_id.to_string();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let ui_event = match event {
                WsEvent::Connected {
                    response_headers,
                    duration_ms,
                } => WsUiEvent::Connected {
                    request_id: request_id.clone(),
                    outgoing: outgoing.clone(),
                    response_headers,
                    duration_ms,
                },
                WsEvent::Message(message) => WsUiEvent::Message {
                    request_id: request_id.clone(),
                    message,
                },
                WsEvent::Closed { code, reason } => WsUiEvent::Closed {
                    request_id: request_id.clone(),
                    code,
                    reason,
                },
                WsEvent::Error(error) => WsUiEvent::Failed {
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
