use super::grpc_engine::{GrpcEngine, GrpcUiEvent};
use super::http_engine::{ExecEvent, HttpEngine};
use super::ws_service::{WsService, WsUiEvent};
use crate::state::{
    EnvironmentState, ModalState, RequestSessionState, ResponseState, ToastState,
    WorkspaceDataState,
};
use gpui_kit::{App, Entity};
use std::sync::Arc;
use tokio::sync::oneshot;
use zest_core::models::{EnvironmentVariable, Protocol};
use zest_core::storage::models::LocFile;
use zest_core::ws::WebSocketStatus;

/// Transport engines owned by the Runner, grouped to keep the constructor
/// within the argument budget.
#[derive(Clone)]
pub struct Engines {
    pub http: Arc<HttpEngine>,
    pub ws: Arc<WsService>,
    pub grpc: Arc<GrpcEngine>,
}

#[derive(Clone)]
pub struct Runner {
    pub session_state: Entity<RequestSessionState>,
    pub workspace_state: Entity<WorkspaceDataState>,
    pub env_state: Entity<EnvironmentState>,
    pub response_state: Entity<ResponseState>,
    pub toast_state: Entity<ToastState>,
    pub modal_state: Entity<ModalState>,
    pub engines: Engines,
}

impl Runner {
    pub fn new(
        session_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        response_state: Entity<ResponseState>,
        toast_state: Entity<ToastState>,
        modal_state: Entity<ModalState>,
        engines: Engines,
    ) -> Self {
        Self {
            session_state,
            workspace_state,
            env_state,
            response_state,
            toast_state,
            modal_state,
            engines,
        }
    }

    pub fn send_current_request(&self, cx: &mut App) {
        let ed = self.session_state.read(cx);
        let req_id = ed.active_request_id();
        let Some(mut req_item) = self.workspace_state.read(cx).find_request(req_id).cloned() else {
            return;
        };

        if ed.is_sending {
            return;
        }

        // Resolve Hierarchical Headers & Auth
        {
            let ws = self.workspace_state.read(cx);
            let (resolved_headers, resolved_auth) = ws.resolve_request_hierarchy(&req_item);
            req_item.headers = resolved_headers;
            req_item.auth = resolved_auth;
        }

        // Apply Global Network & Execution settings
        {
            let modal = self.modal_state.read(cx);
            req_item.set_timeout_secs(modal.network.request_timeout_secs);
            req_item.set_follow_redirects(modal.network.follow_redirects);
            req_item.set_validate_tls(modal.network.validate_tls);
        }

        let cached_env = self.env_state.read(cx).effective_environment();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.session_state.update(cx, |state, cx| {
            state.start_sending(Some(req_item.id.clone()), cancel_tx, cx);
        });

        let mut event_rx = self
            .engines
            .http
            .execute(req_item, Some(cached_env), cancel_rx);

        let session_state = self.session_state.clone();
        let workspace_state = self.workspace_state.clone();
        let env_state = self.env_state.clone();
        let response_state = self.response_state.clone();
        let toast_state = self.toast_state.clone();

        cx.spawn(async move |cx| {
            while let Some(event) = event_rx.recv().await {
                cx.update(|cx| {
                    match event {
                        ExecEvent::Started {
                            request_id,
                            initial_logs,
                            pre_script_error,
                        } => {
                            if let Some(err) = pre_script_error {
                                session_state.update(cx, |ed, cx| {
                                    ed.set_script_error(Some(err), cx);
                                });
                            } else {
                                session_state.update(cx, |ed, cx| {
                                    ed.set_script_error(None, cx);
                                });
                            }
                            response_state.update(cx, |resp, cx| {
                                resp.start_sending(Some(&request_id), cx);
                                resp.set_pre_request_logs(initial_logs, cx);
                            });
                        }
                        ExecEvent::Progress(text) => {
                            session_state.update(cx, |ed, cx| {
                                ed.update_progress(text, cx);
                            });
                        }
                        ExecEvent::Success {
                            record,
                            pre_output,
                            post_output,
                        } => {
                            let status = record.status_code;
                            let req_id = record.request_id.clone();

                            // Update workspace environment with mutations (Request chaining)
                            let all_mutations: Vec<_> = pre_output
                                .env_mutations
                                .iter()
                                .chain(post_output.env_mutations.iter())
                                .cloned()
                                .collect();
                            apply_env_mutations(&env_state, &all_mutations, cx);

                            if let Some(err) = &post_output.error {
                                session_state.update(cx, |ed, cx| {
                                    ed.set_script_error(Some(err.clone()), cx);
                                });
                            }

                            let loc_file = LocFile::from_record(&record);
                            workspace_state.update(cx, |ws, cx| {
                                if let Some(r) = ws.find_request_mut(&req_id) {
                                    r.last_status_code = Some(status);
                                }
                                ws.bump_tree_version();
                                let _ = ws.store.save_response_cache(&req_id, &loc_file);
                                ws.sync_cookies_from_executor();
                                cx.notify();
                            });

                            response_state.update(cx, |resp, cx| {
                                resp.set_test_results(
                                    post_output.test_results,
                                    record.console_logs.clone(),
                                    cx,
                                );
                                resp.set_response_for_request(req_id.clone(), record, cx);
                            });

                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                        }
                        ExecEvent::Cancelled => {
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                        }
                        ExecEvent::Error(err_msg) => {
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                            toast_state.update(cx, |ts, cx| {
                                ts.push_error("Send Failed", err_msg, cx);
                            });
                        }
                    }
                });
            }
        })
        .detach();
    }

    /// Starts the WebSocket handshake for the active request.
    pub fn connect_websocket(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        let Some(mut req_item) = self.workspace_state.read(cx).find_request(&req_id).cloned()
        else {
            return;
        };
        {
            let ed = self.session_state.read(cx);
            if ed.is_sending || req_item.protocol != Protocol::WebSocket {
                return;
            }
        }

        if self.engines.ws.is_connected(&req_id) {
            return;
        }

        // Resolve hierarchical headers & auth, apply global settings — same
        // preamble as the HTTP send path.
        {
            let ws = self.workspace_state.read(cx);
            let (resolved_headers, resolved_auth) = ws.resolve_request_hierarchy(&req_item);
            req_item.headers = resolved_headers;
            req_item.auth = resolved_auth;
        }
        {
            let modal = self.modal_state.read(cx);
            req_item.set_timeout_secs(modal.network.request_timeout_secs);
            req_item.set_validate_tls(modal.network.validate_tls);
        }

        let cached_env = self.env_state.read(cx).effective_environment();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.session_state.update(cx, |state, cx| {
            state.start_sending(Some(req_id.clone()), cancel_tx, cx);
        });
        self.response_state.update(cx, |resp, cx| {
            resp.set_ws_status(WebSocketStatus::Connecting, cx);
        });

        let mut event_rx = self
            .engines
            .ws
            .connect(req_item, Some(cached_env), cancel_rx);

        let session_state = self.session_state.clone();
        let response_state = self.response_state.clone();
        let env_state = self.env_state.clone();
        let toast_state = self.toast_state.clone();
        let ws_service = self.engines.ws.clone();

        cx.spawn(async move |cx| {
            while let Some(event) = event_rx.recv().await {
                cx.update(|cx| match event {
                    WsUiEvent::Started {
                        request_id,
                        initial_logs,
                        env_mutations,
                        pre_script_error,
                    } => {
                        if let Some(err) = pre_script_error {
                            session_state.update(cx, |ed, cx| {
                                ed.set_script_error(Some(err), cx);
                            });
                        } else {
                            session_state.update(cx, |ed, cx| {
                                ed.set_script_error(None, cx);
                            });
                        }
                        apply_env_mutations(&env_state, &env_mutations, cx);
                        response_state.update(cx, |resp, cx| {
                            resp.set_pre_request_logs(initial_logs, cx);
                        });
                        let _ = request_id;
                    }
                    WsUiEvent::Connected {
                        request_id,
                        outgoing,
                        response_headers,
                        duration_ms: _,
                    } => {
                        ws_service.register_session(
                            &request_id,
                            outgoing,
                            response_headers.clone(),
                        );
                        session_state.update(cx, |ed, cx| {
                            ed.stop_sending(cx);
                        });
                        response_state.update(cx, |resp, cx| {
                            resp.set_ws_handshake_headers(response_headers, cx);
                            resp.set_ws_status(WebSocketStatus::Connected, cx);
                        });
                    }
                    WsUiEvent::Message {
                        request_id,
                        message,
                    } => {
                        ws_service.append_inbound(&request_id, message.clone());
                        let active =
                            session_state.read(cx).active_request_id() == request_id.as_str();
                        if active {
                            response_state.update(cx, |resp, cx| {
                                resp.push_ws_message(message, cx);
                            });
                        }
                    }
                    WsUiEvent::Closed {
                        request_id,
                        code: _,
                        reason: _,
                    } => {
                        ws_service.mark_disconnected(&request_id);
                        let active =
                            session_state.read(cx).active_request_id() == request_id.as_str();
                        if active {
                            response_state.update(cx, |resp, cx| {
                                resp.set_ws_status(WebSocketStatus::Disconnected, cx);
                            });
                        }
                    }
                    WsUiEvent::Failed { request_id, error } => {
                        ws_service.mark_failed(&request_id, &error);
                        session_state.update(cx, |ed, cx| {
                            ed.stop_sending(cx);
                        });
                        let active =
                            session_state.read(cx).active_request_id() == request_id.as_str();
                        if active {
                            response_state.update(cx, |resp, cx| {
                                resp.set_ws_status(WebSocketStatus::Failed(error.clone()), cx);
                            });
                        }
                        toast_state.update(cx, |ts, cx| {
                            ts.push_error("WebSocket Failed", error, cx);
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Queues an outgoing text frame for the active request's connection.
    pub fn send_ws_message(&self, text: String, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        match self.engines.ws.send_text(&req_id, text) {
            Ok(message) => {
                self.response_state.update(cx, |resp, cx| {
                    resp.push_ws_message(message, cx);
                });
            }
            Err(err) => {
                self.toast_state.update(cx, |ts, cx| {
                    ts.push_error("Send Failed", err, cx);
                });
            }
        }
    }

    /// Sends a ping frame on the active request's connection.
    pub fn send_ws_ping(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        if let Err(err) = self.engines.ws.send_ping(&req_id) {
            self.toast_state.update(cx, |ts, cx| {
                ts.push_error("Ping Failed", err, cx);
            });
        }
    }

    /// Requests a graceful close of the active request's connection.
    pub fn disconnect_websocket(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        self.engines.ws.disconnect(&req_id);
        self.response_state.update(cx, |resp, cx| {
            resp.set_ws_status(WebSocketStatus::Disconnected, cx);
        });
    }

    /// Invokes the active gRPC request (unary or server-streaming).
    pub fn invoke_grpc(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        let Some(mut req_item) = self.workspace_state.read(cx).find_request(&req_id).cloned()
        else {
            return;
        };
        {
            let ed = self.session_state.read(cx);
            if ed.is_sending || req_item.protocol != Protocol::Grpc {
                return;
            }
        }

        // Resolve hierarchical headers & auth, apply global settings — same
        // preamble as the HTTP send path.
        {
            let ws = self.workspace_state.read(cx);
            let (resolved_headers, resolved_auth) = ws.resolve_request_hierarchy(&req_item);
            req_item.headers = resolved_headers;
            req_item.auth = resolved_auth;
        }
        {
            let modal = self.modal_state.read(cx);
            req_item.set_timeout_secs(modal.network.request_timeout_secs);
            req_item.set_validate_tls(modal.network.validate_tls);
        }

        let cached_env = self.env_state.read(cx).effective_environment();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.session_state.update(cx, |state, cx| {
            state.start_sending(Some(req_id.clone()), cancel_tx, cx);
        });

        let mut event_rx = self
            .engines
            .grpc
            .invoke(req_item, Some(cached_env), cancel_rx);

        let session_state = self.session_state.clone();
        let response_state = self.response_state.clone();
        let env_state = self.env_state.clone();
        let toast_state = self.toast_state.clone();

        cx.spawn(async move |cx| {
            while let Some(event) = event_rx.recv().await {
                cx.update(|cx| match event {
                    GrpcUiEvent::Started {
                        request_id: _,
                        initial_logs,
                        env_mutations,
                        pre_script_error,
                    } => {
                        if let Some(err) = pre_script_error {
                            session_state.update(cx, |ed, cx| {
                                ed.set_script_error(Some(err), cx);
                            });
                        } else {
                            session_state.update(cx, |ed, cx| {
                                ed.set_script_error(None, cx);
                            });
                        }
                        apply_env_mutations(&env_state, &env_mutations, cx);
                        response_state.update(cx, |resp, cx| {
                            resp.set_pre_request_logs(initial_logs, cx);
                        });
                    }
                    GrpcUiEvent::RequestMessage {
                        request_id,
                        message,
                    } => {
                        let active =
                            session_state.read(cx).active_request_id() == request_id.as_str();
                        if active {
                            response_state.update(cx, |resp, cx| {
                                resp.push_ws_message(message, cx);
                            });
                        }
                    }
                    GrpcUiEvent::ResponseMessage {
                        request_id,
                        message,
                    } => {
                        let active =
                            session_state.read(cx).active_request_id() == request_id.as_str();
                        if active {
                            response_state.update(cx, |resp, cx| {
                                resp.push_ws_message(message, cx);
                            });
                        }
                    }
                    GrpcUiEvent::CallCompleted {
                        request_id: _,
                        summary,
                    } => {
                        response_state.update(cx, |resp, cx| {
                            resp.set_grpc_last_call(Some(summary), cx);
                        });
                        session_state.update(cx, |ed, cx| {
                            ed.stop_sending(cx);
                        });
                    }
                    GrpcUiEvent::CallFailed {
                        request_id: _,
                        summary,
                    } => {
                        let status_message = summary.status_message.clone().unwrap_or_default();
                        response_state.update(cx, |resp, cx| {
                            resp.set_grpc_last_call(Some(summary), cx);
                        });
                        session_state.update(cx, |ed, cx| {
                            ed.stop_sending(cx);
                        });
                        toast_state.update(cx, |ts, cx| {
                            ts.push_error("gRPC Error", status_message, cx);
                        });
                    }
                    GrpcUiEvent::Failed {
                        request_id: _,
                        error,
                    } => {
                        session_state.update(cx, |ed, cx| {
                            ed.stop_sending(cx);
                        });
                        toast_state.update(cx, |ts, cx| {
                            ts.push_error("gRPC Failed", error, cx);
                        });
                    }
                });
            }
        })
        .detach();
    }

    /// Resolves the gRPC schema (reflection or .proto) for the picker UI.
    pub fn fetch_grpc_schema(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        let Some(req) = self.workspace_state.read(cx).find_request(&req_id).cloned() else {
            return;
        };
        if req.protocol != Protocol::Grpc {
            return;
        }
        let Some(payload) = req.grpc.clone() else {
            return;
        };

        let session_state = self.session_state.clone();
        let toast_state = self.toast_state.clone();

        let rx = self.engines.grpc.fetch_schema(
            req.url.clone(),
            payload.proto_path.clone(),
            payload.use_reflection,
        );
        cx.spawn(async move |cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Schema fetch task dropped".to_string()));
            cx.update(|cx| match result {
                Ok(summary) => {
                    session_state.update(cx, |ed, cx| {
                        ed.set_grpc_schema(Some(summary), cx);
                    });
                }
                Err(err) => {
                    toast_state.update(cx, |ts, cx| {
                        ts.push_error("Schema Fetch Failed", err, cx);
                    });
                }
            });
        })
        .detach();
    }
}

/// Applies `(key, Some(value))` / `(key, None)` environment mutations to the
/// active environment (or globals when none is active). Shared by the HTTP
/// send path and the WebSocket connect path.
pub(crate) fn apply_env_mutations(
    env_state: &Entity<EnvironmentState>,
    mutations: &[(String, Option<String>)],
    cx: &mut App,
) {
    if mutations.is_empty() {
        return;
    }
    env_state.update(cx, |env, cx| {
        if let Some(act_env) = env.active_environment_mut() {
            for (k, v_opt) in mutations {
                if let Some(val) = v_opt {
                    if let Some(var) = act_env.variables.iter_mut().find(|v| v.key == *k) {
                        var.current_value = val.clone();
                    } else {
                        act_env.variables.push(EnvironmentVariable {
                            key: k.clone(),
                            initial_value: "".to_string(),
                            current_value: val.clone(),
                            secret: false,
                        });
                    }
                } else {
                    act_env.variables.retain(|v| v.key != *k);
                }
            }
            let act_name = act_env.name.clone();
            env.invalidate_cache();
            cx.emit(crate::state::EnvironmentEvent::EnvironmentModified(
                act_name,
            ));
            cx.emit(crate::state::EnvironmentEvent::EnvironmentsReloaded);
            cx.notify();
        } else {
            for (k, v_opt) in mutations {
                if let Some(val) = v_opt {
                    if let Some(var) = env
                        .global_environment
                        .variables
                        .iter_mut()
                        .find(|v| v.key == *k)
                    {
                        var.current_value = val.clone();
                    } else {
                        env.global_environment.variables.push(EnvironmentVariable {
                            key: k.clone(),
                            initial_value: "".to_string(),
                            current_value: val.clone(),
                            secret: false,
                        });
                    }
                } else {
                    env.global_environment.variables.retain(|v| v.key != *k);
                }
            }
            env.invalidate_cache();
            cx.emit(crate::state::EnvironmentEvent::GlobalsModified);
            cx.emit(crate::state::EnvironmentEvent::EnvironmentsReloaded);
            cx.notify();
        }
    });
}
