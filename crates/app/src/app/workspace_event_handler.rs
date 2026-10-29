//! Unified event dispatcher for typed commands/events emitted by state entities
//! (`WorkspaceEvent`, `SessionEvent`, `ResponseEvent`, `EnvironmentEvent`).
//!
//! Centralizes cross-entity state synchronization and UI editor updates so
//! `cx.observe` subscriptions remain pure view-redraw triggers.

use crate::app::AppView;
use crate::state::{
    ComposerSyncScope, EnvironmentEvent, ResponseEvent, SessionEvent, WorkspaceEvent,
};
use gpui_kit::{Context, Window};

pub fn handle_workspace_event(
    this: &mut AppView,
    event: &WorkspaceEvent,
    window: &mut Window,
    cx: &mut Context<AppView>,
) {
    match event {
        WorkspaceEvent::WorkspaceOpened {
            path,
            workspace_name,
            store,
        } => {
            let ws_ent = this.state.workspace.clone();
            let env_ent = this.state.env.clone();
            let git_ent = this.state.git.clone();
            this.state.persistence.update(cx, |per, cx| {
                per.set_workspace(store.clone(), path.clone(), cx);
                per.start_watcher(ws_ent, env_ent, git_ent, cx);
            });

            let (active_env, next_id) = {
                let ws_ref = this.state.workspace.read(cx);
                let per_ref = this.state.persistence.read(cx);
                let id = per_ref
                    .local_state
                    .active_request
                    .as_ref()
                    .and_then(|act_id| {
                        ws_ref
                            .requests
                            .iter()
                            .find(|r| &r.id == act_id)
                            .map(|r| r.id.clone())
                    })
                    .or_else(|| ws_ref.requests.first().map(|r| r.id.clone()))
                    .unwrap_or_else(|| "scratch".to_string());
                (per_ref.local_state.active_environment.clone(), id)
            };

            this.state.env.update(cx, |env, cx| {
                env.set_workspace(store.clone(), workspace_name.clone(), active_env);
                cx.notify();
            });

            this.state.git.update(cx, |git, cx| {
                git.set_repo_path(path.clone(), cx);
            });

            this.state.session.update(cx, |ed, cx| {
                ed.active_request_id = None;
                ed.set_active_request_id(&next_id, cx);
            });
        }
        WorkspaceEvent::IdsRemapped(pairs) => {
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
            this.state.persistence.update(cx, |per, cx| {
                per.remap_request_ids(pairs, cx);
            });
        }
        WorkspaceEvent::RequestsReloaded { req_ids } => {
            let active_id = this.state.session.read(cx).active_request_id().to_string();
            let should_sync = match req_ids {
                None => true,
                Some(ids) => ids.iter().any(|id| id == &active_id),
            };
            if should_sync
                && this
                    .state
                    .workspace
                    .read(cx)
                    .find_request(&active_id)
                    .is_some()
            {
                this.composer_inputs.sync_inputs(
                    &active_id,
                    ComposerSyncScope::All,
                    &this.state.workspace,
                    &this.state.env,
                    window,
                    cx,
                );
            }
        }
        WorkspaceEvent::ExampleRenamed {
            old_name, new_name, ..
        } => {
            this.state.response.update(cx, |resp, cx| {
                resp.rename_example(old_name, new_name, cx);
            });
        }
        WorkspaceEvent::CrudFailed { title, message } => {
            this.state.toast.update(cx, |t, cx| {
                t.push_error(*title, message.clone(), cx);
            });
        }
        WorkspaceEvent::WorkspaceMetaModified => {
            let meta = this.state.workspace.read(cx).workspace_meta.clone();
            this.state.persistence.update(cx, |per, cx| {
                per.schedule_save_workspace_meta(meta, cx);
            });
        }
        WorkspaceEvent::FolderMetaModified(folder_rel) => {
            let fid = folder_rel.clone();
            let store = this.state.workspace.read(cx).store.clone();
            let meta_opt = this
                .state
                .workspace
                .read(cx)
                .folders
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
        WorkspaceEvent::PinsModified => {
            let pins = this.state.workspace.read(cx).pins.clone();
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
                &this.state.workspace,
                &this.state.env,
                window,
                cx,
            );

            let is_scratch = req_id == "scratch" || req_id.is_empty();
            let persisted_id = if is_scratch {
                None
            } else {
                Some(req_id.clone())
            };
            this.state.persistence.update(cx, |per, cx| {
                per.set_active_request(persisted_id, cx);
            });

            let ws_store = this.state.workspace.read(cx).store.clone();
            this.state.response.update(cx, |resp, cx| {
                resp.load_for_request(req_id, ws_store.as_ref(), cx);
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
                    &this.state.workspace,
                    &this.state.env,
                    window,
                    cx,
                );
            }

            let is_scratch = req_id == "scratch" || req_id.is_empty();
            if !is_scratch {
                let ws_ent = this.state.workspace.clone();
                let r_id = req_id.clone();
                this.state.persistence.update(cx, |per, cx| {
                    per.schedule_save_active_request(r_id, ws_ent, cx);
                });
            }
        }
        SessionEvent::SyncComposerInputs { scope } => {
            let active_id = this.state.session.read(cx).active_request_id().to_string();
            this.composer_inputs.sync_inputs(
                &active_id,
                *scope,
                &this.state.workspace,
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
    }
}
