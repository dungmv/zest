#[path = "runner_schema.rs"]
mod schema;

use super::grpc_engine::GrpcEngine;
use super::http_engine::HttpEngine;
use super::send_event::SendEvent;
use super::ws_service::WsService;
use crate::state::{
    CollectionState, EnvironmentState, ModalState, RequestSessionState, ResponseState,
    WorkspaceState,
};
use gpui_kit::{App, Entity};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use zest_core::http::ReqwestExecutor;
use zest_core::models::{Environment, EnvironmentVariable, Protocol, RequestItem};
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

impl Engines {
    pub fn new(executor: Arc<ReqwestExecutor>) -> Self {
        Self {
            http: Arc::new(HttpEngine::new(executor, super::global_tokio_runtime())),
            ws: Arc::new(WsService::new()),
            grpc: Arc::new(GrpcEngine::new()),
        }
    }
}

#[derive(Clone)]
pub struct Runner {
    pub session_state: Entity<RequestSessionState>,
    pub workspace_state: Entity<WorkspaceState>,
    pub collection_state: Entity<CollectionState>,
    pub env_state: Entity<EnvironmentState>,
    pub response_state: Entity<ResponseState>,
    pub modal_state: Entity<ModalState>,
    pub engines: Engines,
}

/// Fully prepared send context gathered on the GPUI thread before any engine
/// starts.
struct SendContext {
    req_id: String,
    req_item: RequestItem,
    env: Option<Arc<Environment>>,
}

/// The begun send: context plus the cancel receiver handed to the engine.
struct PreparedSend {
    ctx: SendContext,
    cancel_rx: oneshot::Receiver<()>,
}

impl Runner {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceState>,
        collection_state: Entity<CollectionState>,
        env_state: Entity<EnvironmentState>,
        response_state: Entity<ResponseState>,
        modal_state: Entity<ModalState>,
        engines: Engines,
    ) -> Self {
        Self {
            session_state,
            workspace_state,
            collection_state,
            env_state,
            response_state,
            modal_state,
            engines,
        }
    }

    /// Primary send action for the active request, whatever its protocol:
    /// HTTP executes a request/response exchange, WebSocket opens a realtime
    /// connection, gRPC invokes a unary or server-streaming call, GraphQL is
    /// converted to its HTTP POST equivalent and SSE streams the response.
    /// All of them share one preamble ([`Runner::gather_send_context`] +
    /// [`Runner::begin_send`]) and one event pump ([`Runner::spawn_send_pump`]).
    pub fn send_current_request(&self, cx: &mut App) {
        let mut ctx = match self.gather_send_context(cx) {
            Some(ctx) => ctx,
            None => return,
        };
        let protocol = ctx.req_item.protocol;
        if protocol == Protocol::WebSocket && self.engines.ws.is_connected(&ctx.req_id) {
            return;
        }
        // GraphQL validation happens before the sending state flashes so a
        // bad query/variables never starts the pipeline.
        if protocol == Protocol::GraphQL {
            match ctx.req_item.to_graphql_http_item() {
                Ok(http_item) => ctx.req_item = http_item,
                Err(err) => {
                    crate::notify::error("GraphQL Failed", err, cx);
                    return;
                }
            }
        }
        if protocol == Protocol::Sse {
            self.engines.http.clear_sse_frames(&ctx.req_id);
        }

        let PreparedSend { ctx, cancel_rx } = self.begin_send(cx, ctx);
        if protocol == Protocol::WebSocket {
            self.response_state.update(cx, |resp, cx| {
                resp.set_ws_status(WebSocketStatus::Connecting, cx);
            });
        }

        let SendContext { req_item, env, .. } = ctx;
        let event_rx = match protocol {
            Protocol::Http | Protocol::GraphQL => {
                self.engines.http.execute(req_item, env, cancel_rx)
            }
            Protocol::WebSocket => self.engines.ws.connect(req_item, env, cancel_rx),
            Protocol::Grpc => self.engines.grpc.invoke(req_item, env, cancel_rx),
            Protocol::Sse => self.engines.http.execute(req_item, env, cancel_rx),
        };
        self.spawn_send_pump(cx, protocol, event_rx);
    }

    /// Shared preamble for every send path: fetch the active request, guard
    /// against re-entrant sends, merge hierarchy headers/auth, apply global
    /// network settings and snapshot the effective environment. Pure reads —
    /// no state is mutated until [`Runner::begin_send`].
    fn gather_send_context(&self, cx: &App) -> Option<SendContext> {
        let ed = self.session_state.read(cx);
        if ed.is_sending || self.collection_state.read(cx).is_loading_workspace() {
            return None;
        }
        let req_id = ed.active_request_id().to_string();
        let mut req_item = self
            .collection_state
            .read(cx)
            .find_request(&req_id)
            .cloned()?;

        // Resolve hierarchical headers & auth (Workspace -> Folders -> Request)
        {
            let col = self.collection_state.read(cx);
            let (resolved_headers, resolved_auth) = col.resolve_request_hierarchy(&req_item);
            req_item.headers = resolved_headers;
            req_item.auth = resolved_auth;
        }

        // Apply global network & execution settings
        {
            let modal = self.modal_state.read(cx);
            req_item.set_timeout_secs(modal.network.request_timeout_secs);
            req_item.set_follow_redirects(modal.network.follow_redirects);
            req_item.set_validate_tls(modal.network.validate_tls);
        }

        let env = Some(self.env_state.read(cx).effective_environment());
        Some(SendContext {
            req_id,
            req_item,
            env,
        })
    }

    /// Flips the session into the sending state and creates the cancel
    /// channel. Split from [`Runner::gather_send_context`] so protocol-specific
    /// guards (e.g. an already-open WebSocket) can run between the two steps
    /// without flashing a sending state.
    fn begin_send(&self, cx: &mut App, ctx: SendContext) -> PreparedSend {
        let (cancel_tx, cancel_rx) = oneshot::channel();
        let req_id = ctx.req_id.clone();
        self.session_state.update(cx, |state, cx| {
            state.start_sending(Some(req_id), cancel_tx, cx);
        });
        PreparedSend { ctx, cancel_rx }
    }

    /// Pumps one unified send-event stream to completion on the GPUI thread.
    /// The pump lives as long as the invocation: until the HTTP exchange
    /// settles, the WebSocket closes, or the gRPC call completes.
    fn spawn_send_pump(
        &self,
        cx: &mut App,
        protocol: Protocol,
        mut event_rx: mpsc::Receiver<SendEvent>,
    ) {
        let session_state = self.session_state.clone();
        let workspace_state = self.workspace_state.clone();
        let origin = workspace_state.read(cx).store.clone();
        let collection_state = self.collection_state.clone();
        let env_state = self.env_state.clone();
        let response_state = self.response_state.clone();
        let ws_service = self.engines.ws.clone();
        let grpc_engine = self.engines.grpc.clone();
        let http_engine = self.engines.http.clone();

        cx.spawn(async move |cx| {
            while let Some(event) = event_rx.recv().await {
                let current = cx.update(|cx| {
                    if !Arc::ptr_eq(&origin, &workspace_state.read(cx).store) {
                        return false;
                    }
                    match event {
                        SendEvent::Started {
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
                                resp.start_sending(Some(&request_id), cx);
                                resp.set_pre_request_logs(initial_logs, cx);
                            });
                        }
                        SendEvent::Progress(text) => {
                            session_state.update(cx, |ed, cx| {
                                ed.update_progress(text, cx);
                            });
                        }
                        SendEvent::Frame { request_id, frame } => match protocol {
                            Protocol::WebSocket => {
                                ws_service.append_inbound(&request_id, frame.clone());
                                let active = session_state.read(cx).active_request_id()
                                    == request_id.as_str();
                                if active {
                                    response_state.update(cx, |resp, cx| {
                                        resp.push_frame(frame, cx);
                                    });
                                }
                            }
                            Protocol::Grpc => {
                                // The engine log is authoritative: register the
                                // frame even when another request is active so
                                // the history survives request switches.
                                grpc_engine.register_frame(&request_id, frame.clone());
                                let active = session_state.read(cx).active_request_id()
                                    == request_id.as_str();
                                if active {
                                    response_state.update(cx, |resp, cx| {
                                        resp.push_frame(frame, cx);
                                    });
                                }
                            }
                            Protocol::Sse => {
                                http_engine.register_sse_frame(&request_id, frame.clone());
                                let active = session_state.read(cx).active_request_id()
                                    == request_id.as_str();
                                if active {
                                    response_state.update(cx, |resp, cx| {
                                        resp.push_frame(frame, cx);
                                    });
                                }
                            }
                            Protocol::Http | Protocol::GraphQL => {}
                        },
                        SendEvent::HttpSuccess {
                            record,
                            post_output,
                        } => {
                            let status = record.status_code;
                            let req_id = record.request_id.clone();

                            // Update workspace environment with post-script
                            // mutations (pre-script ones arrived on Started)
                            apply_env_mutations(&env_state, &post_output.env_mutations, cx);

                            if let Some(err) = &post_output.error {
                                session_state.update(cx, |ed, cx| {
                                    ed.set_script_error(Some(err.clone()), cx);
                                });
                            }

                            let record_for_disk = record.clone();
                            let cookies =
                                workspace_state.update(cx, |ws, _cx| ws.sync_cookies_in_memory());
                            collection_state.update(cx, |col, cx| {
                                if let Some(r) = col.find_request_mut(&req_id) {
                                    r.last_status_code = Some(status);
                                }
                                col.bump_tree_version();
                                // Disk writes (response cache + cookies) serialize
                                // and hash sizable payloads; run them on the
                                // background executor in one sequential task so
                                // response-arrival frames never hitch on I/O.
                                let store = origin.clone();
                                let req_id_for_disk = req_id.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        let loc_file = LocFile::from_record(&record_for_disk);
                                        let _ =
                                            store.save_response_cache(&req_id_for_disk, &loc_file);
                                        if let Ok(val) = serde_json::to_value(&cookies) {
                                            let _ = store.save_cookies(&val);
                                        }
                                    })
                                    .detach();
                                cx.notify();
                            });

                            response_state.update(cx, |resp, cx| {
                                resp.set_response_for_request(req_id.clone(), record, cx);
                            });

                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                        }
                        SendEvent::HandshakeConnected {
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
                            let active =
                                session_state.read(cx).active_request_id() == request_id.as_str();
                            if active {
                                response_state.update(cx, |resp, cx| {
                                    resp.set_ws_handshake_headers(response_headers, cx);
                                    resp.set_ws_status(WebSocketStatus::Connected, cx);
                                });
                            }
                        }
                        SendEvent::HandshakeClosed {
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
                        SendEvent::CallCompleted {
                            request_id,
                            summary,
                        } => {
                            grpc_engine.register_last_call(&request_id, summary.clone());
                            let active =
                                session_state.read(cx).active_request_id() == request_id.as_str();
                            if active {
                                response_state.update(cx, |resp, cx| {
                                    resp.set_grpc_last_call(Some(summary), cx);
                                });
                            }
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                        }
                        SendEvent::CallFailed {
                            request_id,
                            summary,
                        } => {
                            let status_message = summary.status_message.clone().unwrap_or_default();
                            grpc_engine.register_last_call(&request_id, summary.clone());
                            let active =
                                session_state.read(cx).active_request_id() == request_id.as_str();
                            if active {
                                response_state.update(cx, |resp, cx| {
                                    resp.set_grpc_last_call(Some(summary), cx);
                                });
                            }
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                            crate::notify::error("gRPC Error", status_message, cx);
                        }
                        SendEvent::SchemaReady {
                            request_id,
                            summary,
                            auto_service,
                            auto_method,
                        } => {
                            session_state.update(cx, |ed, cx| {
                                ed.set_grpc_schema(Some(std::sync::Arc::new(summary)), cx);
                            });
                            if let (Some(svc), Some(mth)) = (auto_service, auto_method) {
                                collection_state.update(cx, |col, _| {
                                    col.update_active_request(&request_id, |r| {
                                        let payload = r.grpc_mut();
                                        payload.service = svc;
                                        payload.method = mth;
                                    });
                                });
                                session_state.update(cx, |ed, cx| {
                                    ed.mark_dirty(cx);
                                });
                            }
                        }
                        SendEvent::Cancelled => {
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                        }
                        SendEvent::Failed { request_id, error } => {
                            session_state.update(cx, |ed, cx| {
                                ed.stop_sending(cx);
                            });
                            match protocol {
                                Protocol::WebSocket => {
                                    ws_service.mark_failed(&request_id, &error);
                                    let active = session_state.read(cx).active_request_id()
                                        == request_id.as_str();
                                    if active {
                                        response_state.update(cx, |resp, cx| {
                                            resp.set_ws_status(
                                                WebSocketStatus::Failed(error.clone()),
                                                cx,
                                            );
                                        });
                                    }
                                    crate::notify::error("WebSocket Failed", error, cx);
                                }
                                Protocol::Grpc => {
                                    crate::notify::error("gRPC Failed", error, cx);
                                }
                                Protocol::Http | Protocol::GraphQL | Protocol::Sse => {
                                    crate::notify::error("Send Failed", error, cx);
                                }
                            }
                        }
                    }
                    true
                });
                if !current {
                    break;
                }
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
                    resp.push_frame(message, cx);
                });
            }
            Err(err) => {
                crate::notify::error("Send Failed", err, cx);
            }
        }
    }

    /// Sends a ping frame on the active request's connection.
    pub fn send_ws_ping(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        if let Err(err) = self.engines.ws.send_ping(&req_id) {
            crate::notify::error("Ping Failed", err, cx);
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
}

/// Applies `(key, Some(value))` / `(key, None)` environment mutations to the
/// active environment (or globals when none is active). Used by the send
/// pump for every protocol's pre/post script mutations.
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

#[cfg(test)]
#[path = "runner_tests.rs"]
mod tests;
