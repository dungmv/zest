use super::http_engine::{ExecEvent, HttpEngine};
use crate::state::{
    EnvironmentState, ModalState, RequestSessionState, ResponseState, ToastState,
    WorkspaceDataState,
};
use gpui_kit::{App, Entity};
use std::sync::Arc;
use tokio::sync::oneshot;
use zest_core::models::EnvironmentVariable;
use zest_core::storage::models::LocFile;

#[derive(Clone)]
pub struct Runner {
    pub session_state: Entity<RequestSessionState>,
    pub workspace_state: Entity<WorkspaceDataState>,
    pub env_state: Entity<EnvironmentState>,
    pub response_state: Entity<ResponseState>,
    pub toast_state: Entity<ToastState>,
    pub modal_state: Entity<ModalState>,
    pub engine: Arc<HttpEngine>,
}

impl Runner {
    pub fn new(
        session_state: Entity<RequestSessionState>,
        workspace_state: Entity<WorkspaceDataState>,
        env_state: Entity<EnvironmentState>,
        response_state: Entity<ResponseState>,
        toast_state: Entity<ToastState>,
        modal_state: Entity<ModalState>,
        engine: Arc<HttpEngine>,
    ) -> Self {
        Self {
            session_state,
            workspace_state,
            env_state,
            response_state,
            toast_state,
            modal_state,
            engine,
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
            req_item.set_timeout_secs(modal.request_timeout_secs);
            req_item.set_follow_redirects(modal.follow_redirects);
            req_item.set_validate_tls(modal.validate_tls);
        }

        let cached_env = self.env_state.read(cx).effective_environment();
        let (cancel_tx, cancel_rx) = oneshot::channel();

        self.session_state.update(cx, |state, cx| {
            state.start_sending(Some(req_item.id.clone()), cancel_tx, cx);
        });

        let mut event_rx = self.engine.execute(req_item, Some(cached_env), cancel_rx);

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

                            if !all_mutations.is_empty() {
                                env_state.update(cx, |env, cx| {
                                    if let Some(act_env) = env.active_environment_mut() {
                                        for (k, v_opt) in &all_mutations {
                                            if let Some(val) = v_opt {
                                                if let Some(var) = act_env
                                                    .variables
                                                    .iter_mut()
                                                    .find(|v| v.key == *k)
                                                {
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
                                        cx.emit(
                                            crate::state::EnvironmentEvent::EnvironmentModified(
                                                act_name,
                                            ),
                                        );
                                        cx.notify();
                                    } else {
                                        for (k, v_opt) in &all_mutations {
                                            if let Some(val) = v_opt {
                                                if let Some(var) = env
                                                    .global_environment
                                                    .variables
                                                    .iter_mut()
                                                    .find(|v| v.key == *k)
                                                {
                                                    var.current_value = val.clone();
                                                } else {
                                                    env.global_environment.variables.push(
                                                        EnvironmentVariable {
                                                            key: k.clone(),
                                                            initial_value: "".to_string(),
                                                            current_value: val.clone(),
                                                            secret: false,
                                                        },
                                                    );
                                                }
                                            } else {
                                                env.global_environment
                                                    .variables
                                                    .retain(|v| v.key != *k);
                                            }
                                        }
                                        env.invalidate_cache();
                                        cx.emit(crate::state::EnvironmentEvent::GlobalsModified);
                                        cx.notify();
                                    }
                                });
                            }

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
}
