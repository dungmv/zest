//! Unified event dispatcher for typed commands/events emitted by state entities
//! (`WorkspaceEvent`, `CollectionEvent`, `SessionEvent`, `ResponseEvent`,
//! `EnvironmentEvent`).
//!
//! Centralizes cross-entity state synchronization and UI editor updates so
//! `cx.observe` subscriptions remain pure view-redraw triggers.

use crate::app::AppView;
use crate::state::{
    CollectionEvent, ComposerSyncScope, EnvironmentEvent, RequestTabKind, ResponseEvent,
    ResponseTabKind, SessionEvent, WorkspaceEvent,
};
use gpui_kit::{Context, Window};
use std::sync::Arc;
use zest_core::models::Protocol;
use zest_core::storage::WorkspaceStore;

pub fn handle_workspace_event(
    this: &mut AppView,
    event: &WorkspaceEvent,
    cx: &mut Context<AppView>,
) {
    match event {
        WorkspaceEvent::WorkspaceSwitchRequested {
            path,
            store,
            folder_name,
        } => {
            let path = path.clone();
            let store = store.clone();
            let folder_name = folder_name.clone();
            this.workspace_switch_task = Some(cx.spawn(async move |this, cx| loop {
                let Ok(edits) = this.update(cx, |this, cx| {
                    this.state.collection.read(cx).pending_disk_edits()
                }) else {
                    break;
                };
                if let Some(mut edits) = edits {
                    let _ = edits.changed().await;
                    continue;
                }
                let waiting = this.update(cx, |this, cx| {
                    let waiting = this
                        .state
                        .persistence
                        .update(cx, |per, cx| per.flush_before_switch(cx));
                    if waiting.is_none() {
                        activate_workspace(this, &path, &store, &folder_name, cx);
                    }
                    waiting
                });
                let Ok(Some(waiting)) = waiting else { break };
                match waiting.await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        let _ = this.update(cx, |_, cx| {
                            crate::notify::error("Could Not Switch Workspace", error, cx);
                        });
                        break;
                    }
                    Err(_) => break,
                }
            }));
        }
        WorkspaceEvent::WorkspaceMetaModified => {
            let (meta, name) = {
                let ws = this.state.workspace.read(cx);
                (ws.workspace_meta().clone(), ws.workspace_name().to_string())
            };
            let headers = meta.headers.clone();
            let auth = meta.auth.clone();
            this.state.collection.update(cx, |col, cx| {
                col.set_workspace_scope(name, headers, auth, cx);
            });
            this.state.persistence.update(cx, |per, cx| {
                per.schedule_save_workspace_meta(meta, cx);
            });
        }
    }
}

fn activate_workspace(
    this: &mut AppView,
    path: &std::path::Path,
    store: &Arc<dyn WorkspaceStore>,
    folder_name: &str,
    cx: &mut Context<AppView>,
) {
    this.state.workspace.update(cx, |ws, cx| {
        ws.activate_workspace(path.to_path_buf(), store.clone(), cx);
    });
    this.runner.engines.ws.disconnect_all();
    this.runner.engines =
        crate::services::Engines::new(this.state.workspace.read(cx).executor.clone());
    this.state
        .session
        .update(cx, |session, cx| session.reset_workspace(cx));
    this.state
        .response
        .update(cx, |response, cx| response.reset_workspace(cx));
    this.state.persistence.update(cx, |per, cx| {
        per.set_workspace(store.clone(), path.to_path_buf(), cx);
    });
    this.state
        .git
        .update(cx, |git, cx| git.set_repo_path(path.to_path_buf(), cx));
    this.state.env.update(cx, |env, cx| {
        *env = crate::state::EnvironmentState::empty(store.clone(), folder_name.to_string());
        cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        cx.notify();
    });
    this.state.collection.update(cx, |collection, cx| {
        collection.switch_workspace(
            store.clone(),
            path.to_path_buf(),
            folder_name.to_string(),
            cx,
        );
    });
    this.state.session.update(cx, |session, cx| {
        session.set_active_request_id("scratch", cx);
    });
}

pub fn handle_collection_event(
    this: &mut AppView,
    event: &CollectionEvent,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    match event {
        CollectionEvent::WorkspaceScanLoaded {
            workspace_meta,
            local_state,
            environment,
            cookies,
            store,
            announce_opened,
            ..
        } => {
            if !Arc::ptr_eq(store, &this.state.workspace.read(cx).store) {
                return;
            }
            if *announce_opened || !this.initial_restore_done {
                let collection = this.state.collection.clone();
                this.state.sidebar.update(cx, |sidebar, cx| {
                    sidebar.collapse_workspace_folders(&collection, cx);
                });
            }
            this.state.workspace.update(cx, |ws, cx| {
                ws.apply_scan_result(workspace_meta.clone(), cookies.clone(), store, cx);
            });
            let workspace_name = this.state.workspace.read(cx).workspace_name().to_string();
            let col_ent = this.state.collection.clone();
            let env_ent = this.state.env.clone();
            let git_ent = this.state.git.clone();
            this.state.persistence.update(cx, |per, cx| {
                per.local_state = local_state.as_ref().clone();
                per.start_watcher(col_ent, env_ent, git_ent, cx);
            });
            this.state.env.update(cx, |env, cx| {
                env.workspace_name = workspace_name;
                env.active_environment_id = local_state.active_environment.clone();
                env.apply_snapshot(environment.as_ref().clone());
                cx.emit(EnvironmentEvent::EnvironmentsReloaded);
                cx.notify();
            });

            if *announce_opened {
                let col = this.state.collection.read(cx);
                let next = local_state
                    .active_request
                    .as_ref()
                    .and_then(|id| col.find_request(id))
                    .or_else(|| col.requests().first())
                    .map(|request| request.id.clone())
                    .unwrap_or_else(|| "scratch".to_string());
                this.state
                    .session
                    .update(cx, |session, cx| session.set_active_request_id(&next, cx));
            }
        }
        CollectionEvent::WorkspaceMetaReloaded { meta, store } => {
            let persistence = this.state.persistence.read(cx);
            if !persistence.can_apply_workspace_meta(store) {
                return;
            }
            let name = this.state.workspace.update(cx, |workspace, cx| {
                workspace.apply_workspace_meta(meta.as_ref().clone(), cx);
                workspace.workspace_name().to_string()
            });
            this.state.env.update(cx, |env, cx| {
                env.workspace_name = name.clone();
                env.invalidate_cache();
                cx.notify();
            });
            this.state.collection.update(cx, |collection, cx| {
                collection.set_workspace_scope(name, meta.headers.clone(), meta.auth.clone(), cx);
            });
        }
        CollectionEvent::IdsRemapped(pairs) => {
            if pairs.is_empty() {
                return;
            }

            // 1. Rename requests in response state / cache
            this.state.response.update(cx, |resp, _cx| {
                for (old_id, new_id) in pairs {
                    resp.rename_request(old_id, new_id);
                }
            });

            // 2. Follow active request pointer if active request was moved/renamed
            let new_active = {
                let ed_ref = this.state.session.read(cx);
                pairs.iter().find_map(|(old, new)| {
                    (ed_ref.active_request_id.as_deref() == Some(old.as_str())).then(|| new.clone())
                })
            };
            if let Some(new_id) = new_active {
                this.state.session.update(cx, |ed, cx| {
                    ed.active_request_id = Some(new_id);
                    cx.notify();
                });
            }

            // 3. Remap persistence state unsaved requests / active request
            let collection = this.state.collection.clone();
            let pins = collection.read(cx).pins().to_vec();
            this.state.persistence.update(cx, |per, cx| {
                let pending: Vec<_> = pairs
                    .iter()
                    .filter_map(|(old, new)| per.is_dirty(old).then_some(new))
                    .collect();
                per.remap_request_ids(pairs, cx);
                per.schedule_save_pins(pins, cx);
                for id in pending {
                    per.schedule_save_active_request(id.clone(), collection.clone(), cx);
                }
            });
        }
        CollectionEvent::RescanRequested => {
            let persistence = this.state.persistence.clone();
            let dirty = persistence.read(cx).dirty_request_ids();
            this.state.collection.update(cx, |collection, cx| {
                collection.reload_tree_preserving_async(persistence.downgrade(), dirty, cx);
            });
        }
        CollectionEvent::RequestsReloaded { req_ids } => {
            // Startup restore replay: the workspace scans asynchronously, so
            // the first `RequestsReloaded` after AppView::new carries the
            // initial data.
            if !this.initial_restore_done {
                this.initial_restore_done = true;
                let next_id = {
                    let col_ref = this.state.collection.read(cx);
                    let per_ref = this.state.persistence.read(cx);
                    per_ref
                        .local_state
                        .active_request
                        .as_ref()
                        .and_then(|act_id| {
                            col_ref
                                .requests()
                                .iter()
                                .find(|r| &r.id == act_id)
                                .map(|r| r.id.clone())
                        })
                        .or_else(|| col_ref.requests().first().map(|r| r.id.clone()))
                };
                if let Some(next_id) = next_id {
                    this.state.session.update(cx, |ed, cx| {
                        ed.set_active_request_id(&next_id, cx);
                    });
                }
            }

            let active_id = this.state.session.read(cx).active_request_id().to_string();
            if req_ids.is_none()
                && this
                    .state
                    .collection
                    .read(cx)
                    .find_request(&active_id)
                    .is_none()
            {
                let next = this
                    .state
                    .collection
                    .read(cx)
                    .requests()
                    .first()
                    .map(|request| request.id.clone())
                    .unwrap_or_else(|| "scratch".into());
                this.state
                    .session
                    .update(cx, |session, cx| session.set_active_request_id(&next, cx));
                return;
            }
            let should_sync = match req_ids {
                None => !this.state.persistence.read(cx).is_dirty(&active_id),
                Some(ids) => ids.iter().any(|id| id == &active_id),
            };
            if should_sync
                && this
                    .state
                    .collection
                    .read(cx)
                    .find_request(&active_id)
                    .is_some()
            {
                this.composer_inputs.sync_inputs(
                    &active_id,
                    ComposerSyncScope::All,
                    &this.state.collection,
                    &this.state.env,
                    window,
                    cx,
                );
            }
        }
        CollectionEvent::ImportApplied(environments) => {
            this.state
                .persistence
                .read(cx)
                .reconcile_import(environments.clone());
        }
        CollectionEvent::RequestDeleted(id) => {
            this.state
                .persistence
                .update(cx, |persistence, cx| persistence.cancel_request(id, cx));
            this.runner.engines.ws.drop_session(id);
            if this.state.session.read(cx).active_request_id() == id {
                let next = this
                    .state
                    .collection
                    .read(cx)
                    .requests()
                    .first()
                    .map(|request| request.id.clone())
                    .unwrap_or_else(|| "scratch".into());
                this.state
                    .session
                    .update(cx, |session, cx| session.set_active_request_id(&next, cx));
            }
        }
        CollectionEvent::RequestContentModified(id) => {
            let collection = this.state.collection.clone();
            this.state.persistence.update(cx, |persistence, cx| {
                persistence.schedule_save_active_request(id.clone(), collection, cx);
            });
        }
        CollectionEvent::ExampleRenamed {
            req_id,
            old_name,
            new_name,
        } => {
            if this.state.session.read(cx).active_request_id() == req_id {
                this.state.response.update(cx, |resp, cx| {
                    resp.rename_example(old_name, new_name, cx);
                });
            }
        }
        CollectionEvent::CrudFailed { title, message } => {
            crate::notify::error_in(*title, message.clone(), window, cx);
        }
        CollectionEvent::FolderMetaModified(folder_rel) => {
            let fid = folder_rel.clone();
            let store = this.state.collection.read(cx).store.clone();
            let meta_opt = this
                .state
                .collection
                .read(cx)
                .folders()
                .iter()
                .find(|f| f.id == fid)
                .map(|f| {
                    let mut meta = store.load_folder_meta(&fid).unwrap_or_else(|_| {
                        zest_core::storage::models::FolderFile {
                            name: f.name.clone(),
                            order: Vec::new(),
                            headers: Vec::new(),
                            auth: zest_core::models::Auth::Inherit,
                        }
                    });
                    meta.headers = f.headers.clone();
                    meta.auth = f.auth.clone();
                    meta
                });
            if let Some(meta) = meta_opt {
                this.state.persistence.update(cx, |per, cx| {
                    per.schedule_save_folder_meta(fid, meta, cx);
                });
            }
        }
        CollectionEvent::PinsModified => {
            let pins = this.state.collection.read(cx).pins().to_vec();
            this.state.persistence.update(cx, |per, cx| {
                per.schedule_save_pins(pins, cx);
            });
        }
    }
}

pub fn handle_session_event(
    this: &mut AppView,
    event: &SessionEvent,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    match event {
        SessionEvent::ActiveRequestChanged { req_id } => {
            this.composer_inputs.sync_inputs(
                req_id,
                ComposerSyncScope::All,
                &this.state.collection,
                &this.state.env,
                window,
                cx,
            );

            // Keep the composer tab strip valid for this request's protocol
            // (e.g. an HTTP "Body" tab does not exist on a WebSocket request).
            let (protocol, binary_path) = this
                .state
                .collection
                .read(cx)
                .find_request(req_id)
                .map(|r| {
                    let bin = match &r.body {
                        zest_core::models::RequestBody::Binary { file_path } => file_path.clone(),
                        _ => String::new(),
                    };
                    (r.protocol, bin)
                })
                .unwrap_or((Protocol::Http, String::new()));
            this.state.session.update(cx, |ed, cx| {
                let clamped = RequestTabKind::clamp_for(ed.active_tab_kind, protocol);
                if clamped != ed.active_tab_kind {
                    ed.set_active_tab_kind(clamped, cx);
                }
                ed.refresh_binary_file_size(binary_path, cx);
            });

            // Restore the realtime session view (WS status + shared frame
            // log + gRPC call summary + SSE event log) for the incoming
            // request, or an empty disconnected view.
            let (realtime_status, frames, realtime_headers, grpc_last_call) = match protocol {
                Protocol::WebSocket => {
                    let (status, frames, headers) =
                        this.runner.engines.ws.snapshot(req_id).unwrap_or((
                            zest_core::ws::WebSocketStatus::Disconnected,
                            Vec::new(),
                            Vec::new(),
                        ));
                    (status, frames, headers, None)
                }
                Protocol::Grpc => {
                    let engine = &this.runner.engines.grpc;
                    (
                        zest_core::ws::WebSocketStatus::Disconnected,
                        engine.snapshot_frames(req_id).unwrap_or_default(),
                        Vec::new(),
                        engine.snapshot_last_call(req_id),
                    )
                }
                Protocol::Sse => {
                    let engine = &this.runner.engines.http;
                    (
                        zest_core::ws::WebSocketStatus::Disconnected,
                        engine.snapshot_sse_frames(req_id).unwrap_or_default(),
                        Vec::new(),
                        None,
                    )
                }
                _ => (
                    zest_core::ws::WebSocketStatus::Disconnected,
                    Vec::new(),
                    Vec::new(),
                    None,
                ),
            };
            this.state.response.update(cx, |resp, cx| {
                let clamped_tab = ResponseTabKind::clamp_for(resp.view().active_tab, protocol);
                if clamped_tab != resp.view().active_tab {
                    resp.set_active_tab(clamped_tab, cx);
                }
                resp.restore_realtime_session(realtime_status, frames, realtime_headers, cx);
                resp.set_grpc_last_call(grpc_last_call, cx);
            });

            // Follow the restored editor content with fresh validation marks.
            this.refresh_grpc_message_diagnostics(cx);

            if this.state.collection.read(cx).is_loading_workspace() {
                return;
            }

            let is_scratch = req_id == "scratch" || req_id.is_empty();
            let persisted_id = if is_scratch {
                None
            } else {
                Some(req_id.clone())
            };
            this.state.persistence.update(cx, |per, cx| {
                per.set_active_request(persisted_id, cx);
            });

            let col_store = this.state.collection.read(cx).store.clone();
            this.state.response.update(cx, |resp, cx| {
                resp.load_for_request(req_id, col_store, cx);
            });
        }
        SessionEvent::RequestModified {
            req_id,
            sync_inputs,
        } => {
            if let Some(scope) = sync_inputs {
                this.composer_inputs.sync_inputs(
                    req_id,
                    *scope,
                    &this.state.collection,
                    &this.state.env,
                    window,
                    cx,
                );
            }

            let is_scratch = req_id == "scratch" || req_id.is_empty();
            if !is_scratch {
                let col_ent = this.state.collection.clone();
                let r_id = req_id.clone();
                this.state.persistence.update(cx, |per, cx| {
                    per.schedule_save_active_request(r_id, col_ent, cx);
                });
            }
        }
        SessionEvent::SyncComposerInputs { scope } => {
            let active_id = this.state.session.read(cx).active_request_id().to_string();
            this.composer_inputs.sync_inputs(
                &active_id,
                *scope,
                &this.state.collection,
                &this.state.env,
                window,
                cx,
            );
        }
    }
}

pub fn handle_response_event(
    this: &mut AppView,
    event: &ResponseEvent,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    match event {
        ResponseEvent::ContentChanged { sync_sent_preview } => {
            this.inspector_displays
                .sync_response_editor(&this.state.response, window, cx);
            if *sync_sent_preview {
                this.inspector_displays
                    .sync_sent_request_preview(&this.state.response, window, cx);
            }
        }
    }
}

pub fn handle_environment_event(
    this: &mut AppView,
    event: &EnvironmentEvent,
    cx: &mut Context<AppView>,
) {
    match event {
        EnvironmentEvent::ActiveEnvironmentChanged(env_id) => {
            let next_env = env_id.clone();
            this.state.persistence.update(cx, |per, cx| {
                per.set_active_environment(next_env, cx);
            });
        }
        EnvironmentEvent::EnvironmentModified(name) => {
            let env_opt = this
                .state
                .env
                .read(cx)
                .environments
                .iter()
                .find(|e| &e.name == name || &e.id == name)
                .cloned();
            if let Some(env) = env_opt {
                let env_name = env.name.clone();
                this.state.persistence.update(cx, |per, cx| {
                    per.schedule_save_environment(env_name, env, cx);
                });
            }
        }
        EnvironmentEvent::GlobalsModified => {
            let env = this.state.env.read(cx).global_environment.clone();
            this.state.persistence.update(cx, |per, cx| {
                per.schedule_save_globals(env, cx);
            });
        }
        EnvironmentEvent::EnvironmentsReloaded => {}
        EnvironmentEvent::RenameRequested { id, name } => {
            let environment = this.state.env.clone();
            this.state.persistence.update(cx, |persistence, cx| {
                persistence.queue_environment_operation(environment, id, Some(name.clone()), cx);
            });
            return;
        }
        EnvironmentEvent::DeleteRequested(id) => {
            let environment = this.state.env.clone();
            this.state.persistence.update(cx, |persistence, cx| {
                persistence.queue_environment_operation(environment, id, None, cx);
            });
            return;
        }
        EnvironmentEvent::EnvironmentRenamed { old_id, new_id } => {
            this.state.modal.update(cx, |modal, cx| {
                if modal.env_modal.selected_id.as_ref() == Some(old_id) {
                    modal.select_modal_environment(Some(new_id.clone()), cx);
                }
            });
        }
    }

    this.composer_inputs
        .refresh_all_env_decorations(&this.state.env, cx);
}
